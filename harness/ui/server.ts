#!/usr/bin/env node
// Peach Ice Tea — local web UI (D-092). `make ui`, then open the printed URL.
//
// Starts tasks through harness/run-task (the same path as `make run`), streams its output and
// the run's live telemetry, and browses evidence bundles and benchmark reports.
//
// Safety:
// - Listens on 127.0.0.1 only. Every request must name a loopback Host, and every POST must
//   come from this page's own Origin, so another website cannot start a run (CSRF, DNS rebinding).
// - Keys come only from this process's environment. The browser learns whether a key exists,
//   never its value, and a run receives only the key of the profile it uses (as AI_API_KEY,
//   which run-task hands to that profile's variable).
// - Files are served only from inside the evidence and report directories.
// - One run at a time: the free tiers this harness targets cannot afford parallel runs anyway.

import * as http from "http";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { spawn, type ChildProcess } from "child_process";
import { fileURLToPath } from "url";

import { PROVIDER_KEY_VARS, autoProfile, keyFor, listProfiles, splitTelemetry, summarizeBundle, within } from "./lib.ts";

const HARNESS = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const PORT = Number(process.env.UI_PORT ?? 4173);
const HOST = "127.0.0.1";
const FIXTURES = path.join(HARNESS, "benchmarks", "hackathon", "fixtures");
const EVIDENCE_ROOTS: Record<string, string> = {
  evidence: path.join(HARNESS, "evidence"),
  documentation: path.join(HARNESS, "documentation", "evidence"),
};
const REPORT_ROOTS: Record<string, string> = {
  ab: path.join(HARNESS, "benchmarks", "reports", "ab"),
  models: path.join(HARNESS, "benchmarks", "reports", "models"),
  tools: path.join(HARNESS, "benchmarks", "reports", "tools"),
};

function binary(): string | null {
  for (const candidate of ["release", "debug"]) {
    const bin = path.join(HARNESS, "target", candidate, "peach");
    if (fs.existsSync(bin)) return bin;
  }
  return null;
}

// ---------------------------------------------------------------------------------------------
// The current run

interface Run {
  id: string;
  child: ChildProcess;
  evidenceDir: string;
  startedAt: number;
  profile: string;
  model: string | null;
  repo: string;
  log: string[];
  events: any[];
  exitCode: number | null;
  offset: number;
  partial: string;
  timer: NodeJS.Timeout;
}

let current: Run | null = null;
const listeners = new Set<http.ServerResponse>();

function broadcast(kind: string, data: unknown) {
  const frame = `event: ${kind}\ndata: ${JSON.stringify(data)}\n\n`;
  for (const res of listeners) res.write(frame);
}

function runSummary(run: Run) {
  return {
    id: run.id,
    evidenceId: `evidence/${path.basename(run.evidenceDir)}`,
    startedAt: run.startedAt,
    profile: run.profile,
    model: run.model,
    repo: run.repo,
    exitCode: run.exitCode,
    running: run.exitCode === null,
  };
}

function pollTelemetry(run: Run) {
  const file = path.join(run.evidenceDir, "telemetry.jsonl");
  let size: number;
  try {
    size = fs.statSync(file).size;
  } catch {
    return;
  }
  if (size <= run.offset) return;
  const fd = fs.openSync(file, "r");
  const buffer = Buffer.alloc(size - run.offset);
  fs.readSync(fd, buffer, 0, buffer.length, run.offset);
  fs.closeSync(fd);
  run.offset = size;
  const { events, rest } = splitTelemetry(run.partial + buffer.toString("utf8"));
  run.partial = rest;
  for (const event of events) {
    run.events.push(event);
    broadcast("telemetry", event);
  }
}

function appendLog(run: Run, text: string) {
  for (const line of text.split("\n")) {
    if (!line) continue;
    run.log.push(line);
    broadcast("log", line);
  }
}

interface StartRequest {
  repo: string;
  prompt: string;
  profile: string;
  model?: string;
  testCommand?: string;
  maxDurationSecs?: number;
}

function startRun(body: StartRequest): { status: number; error?: string; run?: ReturnType<typeof runSummary> } {
  if (current && current.exitCode === null) return { status: 409, error: "a run is already in progress" };
  const bin = binary();
  if (!bin) return { status: 400, error: "no harness binary: run `make setup` first" };
  const repo = path.resolve(String(body.repo ?? "").replace(/^~(?=$|\/)/, os.homedir()));
  if (!body.repo || !fs.existsSync(repo) || !fs.statSync(repo).isDirectory()) {
    return { status: 400, error: `repository not found: ${body.repo || "(empty)"}` };
  }
  if (repo === HARNESS) return { status: 400, error: "pick the repository to fix, not the harness itself" };
  const prompt = String(body.prompt ?? "").trim();
  if (!prompt) return { status: 400, error: "the task is empty" };

  const profiles = listProfiles(HARNESS, process.env);
  const profileName = body.profile === "auto" ? autoProfile(process.env) : body.profile;
  const profile = profiles.find((p) => p.name === profileName);
  if (!profile) return { status: 400, error: `unknown profile: ${profileName}` };
  const key = keyFor(profile, process.env);
  if (!key) return { status: 400, error: `no key for ${profile.name}: export ${profile.keyVar} (or AI_API_KEY) before make ui` };

  const env: NodeJS.ProcessEnv = { ...process.env };
  for (const name of PROVIDER_KEY_VARS) delete env[name];
  const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\.\d+Z$/, "Z");
  const evidenceDir = path.join(EVIDENCE_ROOTS.evidence, stamp);
  Object.assign(env, {
    AI_API_KEY: key,
    PROFILE: profile.name,
    PROMPT: prompt,
    REPO: repo,
    EVIDENCE_DIR: evidenceDir,
    PEACH_ICE_TEA_BIN: bin,
  });
  const model = body.model?.trim() || null;
  if (model) env.MODEL = model;
  else delete env.MODEL;
  if (body.testCommand?.trim()) env.TEST_COMMAND = body.testCommand.trim();
  else delete env.TEST_COMMAND;
  if (body.maxDurationSecs && body.maxDurationSecs > 0) env.MAX_DURATION_SECS = String(Math.floor(body.maxDurationSecs));
  else delete env.MAX_DURATION_SECS;

  // Its own process group, so Stop can signal peach (not just the shell) and get exit 5 with
  // a complete evidence bundle, as a Ctrl-C would.
  const child = spawn("bash", [path.join(HARNESS, "harness", "run-task")], { env, detached: true, stdio: ["ignore", "pipe", "pipe"] });
  const run: Run = {
    id: stamp,
    child,
    evidenceDir,
    startedAt: Date.now(),
    profile: profile.name,
    model: model ?? profile.defaultModel,
    repo,
    log: [],
    events: [],
    exitCode: null,
    offset: 0,
    partial: "",
    timer: setInterval(() => current && pollTelemetry(current), 400),
  };
  current = run;
  child.stdout!.on("data", (chunk) => appendLog(run, chunk.toString()));
  child.stderr!.on("data", (chunk) => appendLog(run, chunk.toString()));
  child.on("close", (code) => {
    clearInterval(run.timer);
    pollTelemetry(run);
    run.exitCode = code ?? 1;
    broadcast("end", runSummary(run));
  });
  broadcast("start", runSummary(run));
  return { status: 200, run: runSummary(run) };
}

function stopRun(): boolean {
  if (!current || current.exitCode !== null || !current.child.pid) return false;
  try {
    process.kill(-current.child.pid, "SIGINT");
  } catch {
    current.child.kill("SIGINT");
  }
  return true;
}

// ---------------------------------------------------------------------------------------------
// Evidence, reports and fixtures

function listBundles() {
  return Object.entries(EVIDENCE_ROOTS)
    .flatMap(([rootId, root]) => {
      if (!fs.existsSync(root)) return [];
      return fs
        .readdirSync(root, { withFileTypes: true })
        .filter((d) => d.isDirectory())
        .map((d) => summarizeBundle(rootId, path.join(root, d.name)));
    })
    .filter((b) => b !== null)
    .sort((a, b) => b!.mtimeMs - a!.mtimeMs);
}

function readIf(file: string): string | null {
  try {
    return fs.readFileSync(file, "utf8");
  } catch {
    return null;
  }
}

function bundleDetail(rootId: string, name: string) {
  const root = EVIDENCE_ROOTS[rootId];
  if (!root || !within(root, name) || name.includes("/")) return null;
  const dir = path.join(root, name);
  if (!fs.existsSync(dir)) return null;
  const telemetry = splitTelemetry((readIf(path.join(dir, "telemetry.jsonl")) ?? "") + "\n").events;
  return {
    summary: summarizeBundle(rootId, dir),
    reportMd: readIf(path.join(dir, "report.md")),
    prompt: readIf(path.join(dir, "prompt.txt")),
    diff: readIf(path.join(dir, "diff.patch")),
    telemetry,
  };
}

function listReports() {
  return Object.entries(REPORT_ROOTS).flatMap(([kind, root]) => {
    if (!fs.existsSync(root)) return [];
    return fs
      .readdirSync(root)
      .filter((f) => f.endsWith(".md"))
      .map((f) => ({ kind, name: f, mtimeMs: fs.statSync(path.join(root, f)).mtimeMs }));
  }).sort((a, b) => b.mtimeMs - a.mtimeMs);
}

function listFixtures() {
  if (!fs.existsSync(FIXTURES)) return [];
  return fs.readdirSync(FIXTURES).filter((f) => fs.existsSync(path.join(FIXTURES, f, "issue.md")));
}

/** Copies a fixture's repository to a scratch directory, so a run never edits the fixture itself. */
function prepareFixture(name: string) {
  if (!listFixtures().includes(name)) return null;
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), `peach-ice-tea-${name}-`));
  fs.cpSync(path.join(FIXTURES, name, "repo"), dir, { recursive: true });
  const meta = JSON.parse(readIf(path.join(FIXTURES, name, "meta.json")) ?? "{}");
  return { repo: dir, prompt: readIf(path.join(FIXTURES, name, "issue.md")) ?? "", testCommand: meta.test_command ?? "" };
}

// ---------------------------------------------------------------------------------------------
// HTTP

function send(res: http.ServerResponse, status: number, body: unknown, type = "application/json") {
  const payload = type === "application/json" ? JSON.stringify(body) : String(body);
  res.writeHead(status, { "content-type": `${type}; charset=utf-8`, "cache-control": "no-store", "x-content-type-options": "nosniff" });
  res.end(payload);
}

function readBody(req: http.IncomingMessage): Promise<any> {
  return new Promise((resolve) => {
    let data = "";
    req.on("data", (chunk) => {
      data += chunk;
      if (data.length > 1_000_000) req.destroy();
    });
    req.on("end", () => {
      try {
        resolve(JSON.parse(data || "{}"));
      } catch {
        resolve({});
      }
    });
  });
}

const LOOPBACK_HOSTS = new Set([`127.0.0.1:${PORT}`, `localhost:${PORT}`]);

const server = http.createServer(async (req, res) => {
  if (!LOOPBACK_HOSTS.has(req.headers.host ?? "")) return send(res, 403, { error: "loopback only" });
  const url = new URL(req.url ?? "/", `http://${req.headers.host}`);
  if (req.method === "POST") {
    const origin = req.headers.origin;
    if (!origin || !LOOPBACK_HOSTS.has(origin.replace(/^http:\/\//, ""))) return send(res, 403, { error: "cross-origin request refused" });
  }

  try {
    if (req.method === "GET" && url.pathname === "/") {
      return send(res, 200, fs.readFileSync(path.join(HARNESS, "harness", "ui", "index.html"), "utf8"), "text/html");
    }
    if (req.method === "GET" && url.pathname === "/api/status") {
      return send(res, 200, {
        binary: binary() ? path.relative(HARNESS, binary()!) : null,
        profiles: listProfiles(HARNESS, process.env),
        autoProfile: autoProfile(process.env),
        fixtures: listFixtures(),
        defaultRepo: process.env.UI_DEFAULT_REPO ?? "",
        run: current ? runSummary(current) : null,
      });
    }
    if (req.method === "GET" && url.pathname === "/api/stream") {
      res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-store", connection: "keep-alive" });
      if (current) {
        res.write(`event: replay\ndata: ${JSON.stringify({ run: runSummary(current), log: current.log, events: current.events })}\n\n`);
      }
      listeners.add(res);
      const ping = setInterval(() => res.write(": ping\n\n"), 15000);
      req.on("close", () => {
        clearInterval(ping);
        listeners.delete(res);
      });
      return;
    }
    if (req.method === "POST" && url.pathname === "/api/run") {
      const result = startRun(await readBody(req));
      return send(res, result.status, result.error ? { error: result.error } : result.run);
    }
    if (req.method === "POST" && url.pathname === "/api/run/stop") {
      return send(res, stopRun() ? 200 : 409, { stopping: true });
    }
    if (req.method === "POST" && url.pathname === "/api/fixture") {
      const prepared = prepareFixture(String((await readBody(req)).name ?? ""));
      return prepared ? send(res, 200, prepared) : send(res, 404, { error: "no such fixture" });
    }
    if (req.method === "GET" && url.pathname === "/api/runs") return send(res, 200, listBundles());
    const bundle = url.pathname.match(/^\/api\/runs\/([a-z]+)\/([^/]+)$/);
    if (req.method === "GET" && bundle) {
      const detail = bundleDetail(bundle[1], decodeURIComponent(bundle[2]));
      return detail ? send(res, 200, detail) : send(res, 404, { error: "no such bundle" });
    }
    if (req.method === "GET" && url.pathname === "/api/reports") return send(res, 200, listReports());
    const report = url.pathname.match(/^\/api\/reports\/([a-z]+)\/([^/]+\.md)$/);
    if (req.method === "GET" && report) {
      const root = REPORT_ROOTS[report[1]];
      const name = decodeURIComponent(report[2]);
      if (!root || !within(root, name) || name.includes("/")) return send(res, 404, { error: "no such report" });
      const text = readIf(path.join(root, name));
      return text === null ? send(res, 404, { error: "no such report" }) : send(res, 200, { name, markdown: text });
    }
    send(res, 404, { error: "not found" });
  } catch (error) {
    send(res, 500, { error: String((error as Error).message ?? error) });
  }
});

server.listen(PORT, HOST, () => {
  const keys = listProfiles(HARNESS, process.env).filter((p) => p.keyAvailable).map((p) => p.name);
  console.log(`Peach Ice Tea UI: http://${HOST}:${PORT}/`);
  console.log(keys.length ? `profiles with a key: ${keys.join(", ")}` : "no provider key in the environment: export AI_API_KEY=... and restart");
});

for (const signal of ["SIGINT", "SIGTERM"] as const) {
  process.on(signal, () => {
    stopRun();
    server.close();
    setTimeout(() => process.exit(0), 300);
  });
}
