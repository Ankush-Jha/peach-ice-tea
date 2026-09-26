#!/usr/bin/env node

// Local, offline, credit-free evaluation runner for the hackathon-shaped suite
// (docs/harness/HACKATHON.md §9, TH.7 / R-HACK-8).
//
// Mirrors, per fixture: copy repo to a temp dir -> git init/commit -> compute our OWN
// SHA-256 manifest of the test files, independent of anything the harness does -> render
// the frozen prompt -> invoke `forge exec --json` with stdin closed and a hard, process-
// group-killing timeout -> run the fixture's tests independently -> re-hash the test files
// and diff against the manifest -> write a report outside the fixture repo copy.
//
// `--agent reference` and `--agent cheat` are stubs that never call forge, so the runner
// (and its integrity check) can be exercised with no model and no credit.

// Handle EPIPE errors gracefully (e.g. when piping to `head` or `jq` that closes early).
process.stdout.on("error", (error: NodeJS.ErrnoException) => {
  if (error.code === "EPIPE") process.exit(0);
  throw error;
});

import * as fs from "fs/promises";
import * as fsSync from "fs";
import * as os from "os";
import * as path from "path";
import * as crypto from "crypto";
import { fileURLToPath } from "url";
import { spawn } from "child_process";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

/** `benchmarks/hackathon` */
const HACKATHON_DIR = __dirname;
/** Repository root, two levels up from `benchmarks/hackathon`. */
const REPO_ROOT = path.resolve(HACKATHON_DIR, "..", "..");
const FIXTURES_DIR = path.join(HACKATHON_DIR, "fixtures");
const REPORTS_DIR = path.join(REPO_ROOT, "benchmarks", "reports", "hackathon");
const PROMPT_TEMPLATE_PATH = path.join(REPO_ROOT, "configuration", "prompt-template.md");

type Agent = "forge" | "reference" | "cheat";

/** A loud, unrecoverable configuration or fixture problem. Never swallowed. */
class HackathonError extends Error {}

function fail(message: string): never {
  throw new HackathonError(message);
}

interface Meta {
  language: string;
  test_command: string;
  integration_command?: string;
  /**
   * Paths (relative to the fixture's `repo/`) that count as test files for the
   * integrity check. Not part of the minimal `meta.json` shape, but required for an
   * unambiguous, harness-independent manifest: without it, "which files are tests"
   * would have to be guessed from naming conventions.
   */
  test_files: string[];
}

interface CliArgs {
  agent: Agent;
  suite: string;
  bin: string;
  timeoutMs: number;
  testTimeoutMs: number;
  label?: string;
}

const DEFAULT_TIMEOUT_MS = 10 * 60_000;
const DEFAULT_TEST_TIMEOUT_MS = 2 * 60_000;

function printUsage(): void {
  console.log(
    [
      "Usage: npm run hackathon -- --agent <forge|reference|cheat> [options]",
      "",
      "Options:",
      "  --agent <forge|reference|cheat>  Required. 'reference' applies solution.patch,",
      "                                    'cheat' edits a test file, 'forge' invokes the",
      "                                    real binary.",
      "  --suite <all|name[,name...]>     Fixtures to run. Default: all.",
      "  --bin <path>                     forge binary. Default: $PEACH_ICE_TEA_BIN or 'forge'.",
      "  --timeout-ms <n>                 Hard timeout for the agent step. Default: " +
        DEFAULT_TIMEOUT_MS +
        ".",
      "  --test-timeout-ms <n>            Timeout for independent test runs. Default: " +
        DEFAULT_TEST_TIMEOUT_MS +
        ".",
      "  --label <string>                 Report file label. Default: '<agent>-<suite>'.",
      "  -h, --help                       Show this help.",
    ].join("\n"),
  );
}

function parseArgs(argv: string[]): CliArgs {
  let agent: Agent | undefined;
  let suite = "all";
  let bin = process.env.PEACH_ICE_TEA_BIN ?? "forge";
  let timeoutMs = DEFAULT_TIMEOUT_MS;
  let testTimeoutMs = DEFAULT_TEST_TIMEOUT_MS;
  let label: string | undefined;

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    switch (arg) {
      case "--agent": {
        const value = argv[++i];
        if (value !== "forge" && value !== "reference" && value !== "cheat") {
          fail(`--agent must be one of forge|reference|cheat, got: ${value ?? "<missing>"}`);
        }
        agent = value;
        break;
      }
      case "--suite":
        suite = argv[++i] ?? fail("--suite requires a value");
        break;
      case "--bin":
        bin = argv[++i] ?? fail("--bin requires a value");
        break;
      case "--timeout-ms": {
        const raw = argv[++i] ?? fail("--timeout-ms requires a value");
        timeoutMs = Number(raw);
        if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
          fail(`--timeout-ms must be a positive number, got: ${raw}`);
        }
        break;
      }
      case "--test-timeout-ms": {
        const raw = argv[++i] ?? fail("--test-timeout-ms requires a value");
        testTimeoutMs = Number(raw);
        if (!Number.isFinite(testTimeoutMs) || testTimeoutMs <= 0) {
          fail(`--test-timeout-ms must be a positive number, got: ${raw}`);
        }
        break;
      }
      case "--label":
        label = argv[++i] ?? fail("--label requires a value");
        break;
      case "-h":
      case "--help":
        printUsage();
        process.exit(0);
        break;
      default:
        fail(`Unknown argument: ${arg}. Run with --help for usage.`);
    }
  }

  if (!agent) fail("--agent is required (forge|reference|cheat). Run with --help for usage.");
  return { agent, suite, bin, timeoutMs, testTimeoutMs, label };
}

// ---------------------------------------------------------------------------
// Process execution: every child is spawned in its own process group so the
// hard timeout can kill the whole group, not just the immediate child.
// ---------------------------------------------------------------------------

interface RunResult {
  code: number | null;
  signal: NodeJS.Signals | null;
  stdout: string;
  stderr: string;
  timedOut: boolean;
  wallMs: number;
}

function runProcess(
  command: string,
  cmdArgs: string[],
  opts: { cwd: string; timeoutMs: number },
): Promise<RunResult> {
  return new Promise((resolve, reject) => {
    const start = Date.now();
    let child: ReturnType<typeof spawn>;
    try {
      child = spawn(command, cmdArgs, {
        cwd: opts.cwd,
        detached: true, // own process group, so -pid kills the whole tree
        stdio: ["ignore", "pipe", "pipe"], // stdin closed: no interactive input is possible
      });
    } catch (err) {
      reject(err);
      return;
    }

    let stdout = "";
    let stderr = "";
    let timedOut = false;
    let settled = false;

    const timer = setTimeout(() => {
      timedOut = true;
      if (child.pid) {
        try {
          process.kill(-child.pid, "SIGKILL");
        } catch {
          // Process group already gone; nothing to do.
        }
      }
    }, opts.timeoutMs);

    child.stdout?.on("data", (chunk: Buffer) => (stdout += chunk.toString("utf8")));
    child.stderr?.on("data", (chunk: Buffer) => (stderr += chunk.toString("utf8")));

    child.on("error", (err) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      reject(err);
    });

    child.on("close", (code, signal) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve({ code, signal, stdout, stderr, timedOut, wallMs: Date.now() - start });
    });
  });
}

/** Runs a fixed shell command line (from `meta.json`, authored by us) via `/bin/sh -c`. */
function runShellCommand(commandLine: string, opts: { cwd: string; timeoutMs: number }): Promise<RunResult> {
  return runProcess("/bin/sh", ["-c", commandLine], opts);
}

const GIT_ENV_ARGS = ["-c", "user.email=hackathon@local", "-c", "user.name=hackathon"];

async function gitInitAndCommit(repoDir: string): Promise<void> {
  const timeoutMs = 30_000;
  const init = await runProcess("git", ["init", "-q"], { cwd: repoDir, timeoutMs });
  if (init.code !== 0) fail(`git init failed in ${repoDir}: ${init.stderr}`);

  const add = await runProcess("git", [...GIT_ENV_ARGS, "add", "-A"], { cwd: repoDir, timeoutMs });
  if (add.code !== 0) fail(`git add failed in ${repoDir}: ${add.stderr}`);

  const commit = await runProcess("git", [...GIT_ENV_ARGS, "commit", "-q", "-m", "fixture: initial state"], {
    cwd: repoDir,
    timeoutMs,
  });
  if (commit.code !== 0) fail(`git commit failed in ${repoDir}: ${commit.stderr}`);
}

// ---------------------------------------------------------------------------
// Integrity manifest
// ---------------------------------------------------------------------------

/** path (relative to repoDir) -> sha256 hex, or null if the file is missing/unreadable. */
type Manifest = Map<string, string | null>;

async function hashTestFiles(repoDir: string, testFiles: string[]): Promise<Manifest> {
  const manifest: Manifest = new Map();
  for (const rel of testFiles) {
    const abs = path.join(repoDir, rel);
    try {
      const content = await fs.readFile(abs);
      manifest.set(rel, crypto.createHash("sha256").update(content).digest("hex"));
    } catch {
      manifest.set(rel, null);
    }
  }
  return manifest;
}

function diffManifests(before: Manifest, after: Manifest): string[] {
  const violations: string[] = [];
  for (const [file, beforeHash] of before) {
    const afterHash = after.get(file);
    if (afterHash === undefined) {
      violations.push(`${file}: not re-checked after the run (internal error)`);
    } else if (beforeHash === null) {
      violations.push(`${file}: was missing before the run even started (fixture is broken)`);
    } else if (afterHash === null) {
      violations.push(`${file}: deleted or unreadable after the run`);
    } else if (afterHash !== beforeHash) {
      violations.push(
        `${file}: content changed (sha256 ${beforeHash.slice(0, 12)}... -> ${afterHash.slice(0, 12)}...)`,
      );
    }
  }
  return violations;
}

// ---------------------------------------------------------------------------
// Prompt rendering
// ---------------------------------------------------------------------------

function renderPrompt(template: string, issue: string, meta: Meta): string {
  const integrationLine = meta.integration_command
    ? `Integration test command: \`${meta.integration_command}\``
    : "";
  return template
    .replaceAll("{{ISSUE}}", issue.trim())
    .replaceAll("{{TEST_COMMAND}}", meta.test_command)
    .replaceAll("{{INTEGRATION_COMMAND_LINE}}", integrationLine);
}

// ---------------------------------------------------------------------------
// Agent stubs: credit-free ways to exercise the runner without a model.
// ---------------------------------------------------------------------------

const CHEAT_STUBS: Record<string, string> = {
  python: "import unittest\n\n\nclass TestStub(unittest.TestCase):\n    def test_stub(self):\n        pass\n",
  javascript: 'const test = require("node:test");\n\ntest("stub", () => {});\n',
};

async function applyStubAgent(
  agent: "reference" | "cheat",
  fixtureName: string,
  fixtureDir: string,
  repoDir: string,
  meta: Meta,
): Promise<{ note: string; wallMs: number }> {
  const start = Date.now();

  if (agent === "reference") {
    const patchPath = path.join(fixtureDir, "solution.patch");
    if (!fsSync.existsSync(patchPath)) {
      fail(`fixture "${fixtureName}" has no solution.patch, required for --agent reference`);
    }
    const result = await runProcess("git", ["apply", "--whitespace=nowarn", patchPath], {
      cwd: repoDir,
      timeoutMs: 30_000,
    });
    if (result.code !== 0) {
      fail(`git apply of solution.patch failed for "${fixtureName}" (exit ${result.code}): ${result.stderr}`);
    }
    return { note: "applied solution.patch", wallMs: Date.now() - start };
  }

  // agent === "cheat": edit a test file instead of touching the implementation, to prove
  // the integrity check catches it even though the mutated test is designed to pass.
  const target = meta.test_files[0];
  if (!target) fail(`fixture "${fixtureName}" has no test_files, required for --agent cheat`);
  const stub = CHEAT_STUBS[meta.language];
  if (!stub) fail(`--agent cheat has no stub test for language "${meta.language}" (fixture mismatch)`);
  await fs.writeFile(path.join(repoDir, target), stub);
  return { note: `overwrote ${target} with an always-passing stub`, wallMs: Date.now() - start };
}

// ---------------------------------------------------------------------------
// Metrics: read only from the exec JSON line, never from model-generated text.
// ---------------------------------------------------------------------------

interface MetricsSummary {
  llm_calls: number;
  failed_llm_calls: number;
  input_tokens: number;
  cached_input_tokens: number;
  output_tokens: number;
  reasoning_tokens: number;
  tool_calls: Record<string, number>;
  tool_errors: Record<string, number>;
  wall_ms: number;
}

const ZERO_METRICS: MetricsSummary = {
  llm_calls: 0,
  failed_llm_calls: 0,
  input_tokens: 0,
  cached_input_tokens: 0,
  output_tokens: 0,
  reasoning_tokens: 0,
  tool_calls: {},
  tool_errors: {},
  wall_ms: 0,
};

function lastJsonLine(stdout: string): unknown | null {
  const lines = stdout
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
  const last = lines[lines.length - 1];
  if (!last) return null;
  try {
    return JSON.parse(last);
  } catch {
    return null;
  }
}

function extractMetrics(parsed: unknown): MetricsSummary | null {
  if (!parsed || typeof parsed !== "object") return null;
  const obj = parsed as Record<string, unknown>;
  const metrics = obj.metrics;
  if (!metrics || typeof metrics !== "object") return null;
  const m = metrics as Record<string, unknown>;
  const asNum = (v: unknown): number => (typeof v === "number" ? v : 0);
  const asMap = (v: unknown): Record<string, number> =>
    v && typeof v === "object" ? (v as Record<string, number>) : {};
  return {
    llm_calls: asNum(m.llm_calls),
    failed_llm_calls: asNum(m.failed_llm_calls),
    input_tokens: asNum(m.input_tokens),
    cached_input_tokens: asNum(m.cached_input_tokens),
    output_tokens: asNum(m.output_tokens),
    reasoning_tokens: asNum(m.reasoning_tokens),
    tool_calls: asMap(m.tool_calls),
    tool_errors: asMap(m.tool_errors),
    wall_ms: asNum(m.wall_ms),
  };
}

// ---------------------------------------------------------------------------
// Per-fixture run
// ---------------------------------------------------------------------------

interface FixtureResult {
  fixture: string;
  language: string;
  agent: Agent;
  tmp_dir: string;
  tests_passed: boolean;
  integration_passed: boolean | null;
  integrity: "clean" | "violation";
  integrity_details: string[];
  success: boolean;
  exec: {
    invoked: boolean;
    note: string;
    exit_code: number | null;
    timed_out: boolean;
    outcome: string | null;
  };
  metrics: MetricsSummary;
  wall_ms: number;
  error: string | null;
}

async function readMeta(fixtureName: string, fixtureDir: string): Promise<Meta> {
  const metaPath = path.join(fixtureDir, "meta.json");
  let raw: string;
  try {
    raw = await fs.readFile(metaPath, "utf8");
  } catch {
    fail(`fixture "${fixtureName}" is missing meta.json at ${metaPath}`);
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (err) {
    fail(`fixture "${fixtureName}" has invalid JSON in meta.json: ${(err as Error).message}`);
  }
  const obj = parsed as Record<string, unknown>;
  if (typeof obj.language !== "string" || obj.language.length === 0) {
    fail(`fixture "${fixtureName}" meta.json is missing a "language" string`);
  }
  if (typeof obj.test_command !== "string" || obj.test_command.length === 0) {
    fail(`fixture "${fixtureName}" meta.json is missing a "test_command" string`);
  }
  if (!Array.isArray(obj.test_files) || obj.test_files.length === 0) {
    fail(`fixture "${fixtureName}" meta.json is missing a non-empty "test_files" array`);
  }
  for (const f of obj.test_files) {
    if (typeof f !== "string") fail(`fixture "${fixtureName}" meta.json has a non-string entry in "test_files"`);
  }
  if (obj.integration_command !== undefined && typeof obj.integration_command !== "string") {
    fail(`fixture "${fixtureName}" meta.json "integration_command" must be a string when present`);
  }
  return {
    language: obj.language,
    test_command: obj.test_command,
    test_files: obj.test_files as string[],
    ...(typeof obj.integration_command === "string" ? { integration_command: obj.integration_command } : {}),
  };
}

async function runFixture(fixtureName: string, args: CliArgs, promptTemplate: string): Promise<FixtureResult> {
  const fixtureDir = path.join(FIXTURES_DIR, fixtureName);
  const repoSrc = path.join(fixtureDir, "repo");
  const issuePath = path.join(fixtureDir, "issue.md");

  if (!fsSync.existsSync(repoSrc) || !fsSync.statSync(repoSrc).isDirectory()) {
    fail(`fixture "${fixtureName}" has no repo/ directory at ${repoSrc}`);
  }
  if (!fsSync.existsSync(issuePath)) {
    fail(`fixture "${fixtureName}" has no issue.md at ${issuePath}`);
  }

  const meta = await readMeta(fixtureName, fixtureDir);
  const issue = await fs.readFile(issuePath, "utf8");

  const tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), `hackathon-${fixtureName}-`));
  const repoDir = path.join(tmpDir, "repo");
  await fs.cp(repoSrc, repoDir, { recursive: true });

  await gitInitAndCommit(repoDir);

  const beforeManifest = await hashTestFiles(repoDir, meta.test_files);
  for (const [file, hash] of beforeManifest) {
    if (hash === null) fail(`fixture "${fixtureName}" lists test file "${file}" but it does not exist in repo/`);
  }

  const prompt = renderPrompt(promptTemplate, issue, meta);

  const fixtureStart = Date.now();
  let exec: FixtureResult["exec"];
  let metrics: MetricsSummary = ZERO_METRICS;
  let runError: string | null = null;

  if (args.agent === "reference" || args.agent === "cheat") {
    const { note, wallMs } = await applyStubAgent(args.agent, fixtureName, fixtureDir, repoDir, meta);
    exec = { invoked: false, note, exit_code: 0, timed_out: false, outcome: `stub:${args.agent}` };
    metrics = { ...ZERO_METRICS, wall_ms: wallMs };
  } else {
    let result: RunResult;
    try {
      result = await runProcess(args.bin, ["exec", "--json", prompt], { cwd: repoDir, timeoutMs: args.timeoutMs });
    } catch (err) {
      fail(
        `could not invoke forge ("${args.bin} exec --json ..."): ${(err as Error).message}. ` +
          `Set PEACH_ICE_TEA_BIN or pass --bin to point at the built binary.`,
      );
    }
    const parsed = lastJsonLine(result.stdout);
    const extracted = extractMetrics(parsed);
    metrics = extracted ?? { ...ZERO_METRICS, wall_ms: result.wallMs };
    const outcome =
      parsed && typeof parsed === "object" && typeof (parsed as Record<string, unknown>).outcome === "string"
        ? ((parsed as Record<string, unknown>).outcome as string)
        : null;
    exec = {
      invoked: true,
      note: result.timedOut
        ? `killed after ${args.timeoutMs}ms timeout (process group)`
        : `exited ${result.code ?? "null"}${result.signal ? ` (signal ${result.signal})` : ""}`,
      exit_code: result.code,
      timed_out: result.timedOut,
      outcome,
    };
    if (extracted === null) {
      runError = "forge exec produced no parseable JSON outcome line on its last stdout line";
    }
  }

  const testRun = await runShellCommand(meta.test_command, { cwd: repoDir, timeoutMs: args.testTimeoutMs });
  const testsPassed = testRun.code === 0 && !testRun.timedOut;

  let integrationPassed: boolean | null = null;
  if (meta.integration_command) {
    const integrationRun = await runShellCommand(meta.integration_command, {
      cwd: repoDir,
      timeoutMs: args.testTimeoutMs,
    });
    integrationPassed = integrationRun.code === 0 && !integrationRun.timedOut;
  }

  const afterManifest = await hashTestFiles(repoDir, meta.test_files);
  const violations = diffManifests(beforeManifest, afterManifest);
  const integrity: "clean" | "violation" = violations.length > 0 ? "violation" : "clean";

  const success = integrity === "clean" && testsPassed && integrationPassed !== false && runError === null;

  return {
    fixture: fixtureName,
    language: meta.language,
    agent: args.agent,
    tmp_dir: tmpDir,
    tests_passed: testsPassed,
    integration_passed: integrationPassed,
    integrity,
    integrity_details: violations,
    success,
    exec,
    metrics,
    wall_ms: Date.now() - fixtureStart,
    error: runError,
  };
}

// ---------------------------------------------------------------------------
// Fixture discovery
// ---------------------------------------------------------------------------

async function listFixtures(): Promise<string[]> {
  let entries: fsSync.Dirent[];
  try {
    entries = await fs.readdir(FIXTURES_DIR, { withFileTypes: true });
  } catch (err) {
    fail(`could not read fixtures directory ${FIXTURES_DIR}: ${(err as Error).message}`);
  }
  return entries
    .filter((e) => e.isDirectory())
    .map((e) => e.name)
    .sort();
}

function selectFixtures(suite: string, available: string[]): string[] {
  if (suite === "all") {
    if (available.length === 0) fail(`no fixtures found under ${FIXTURES_DIR}`);
    return available;
  }
  const requested = suite
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
  if (requested.length === 0) fail(`--suite "${suite}" did not name any fixtures`);
  for (const name of requested) {
    if (!available.includes(name)) {
      fail(`--suite requested unknown fixture "${name}". Available: ${available.join(", ") || "(none)"}`);
    }
  }
  return requested;
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

interface RunReport {
  run: {
    timestamp: string;
    agent: Agent;
    suite: string;
    bin: string;
  };
  fixtures: FixtureResult[];
  summary: {
    total: number;
    passed: number;
    failed: number;
    integrity_violations: number;
    all_success: boolean;
  };
}

function buildReport(args: CliArgs, timestamp: Date, results: FixtureResult[]): RunReport {
  const passed = results.filter((r) => r.success).length;
  const integrityViolations = results.filter((r) => r.integrity === "violation").length;
  return {
    run: {
      timestamp: timestamp.toISOString(),
      agent: args.agent,
      suite: args.suite,
      bin: args.agent === "forge" ? args.bin : "(not invoked)",
    },
    fixtures: results,
    summary: {
      total: results.length,
      passed,
      failed: results.length - passed,
      integrity_violations: integrityViolations,
      all_success: passed === results.length,
    },
  };
}

function renderMarkdown(report: RunReport): string {
  const lines: string[] = [];
  lines.push(`# Hackathon suite report — ${report.run.timestamp}`);
  lines.push("");
  lines.push(`Agent: \`${report.run.agent}\` · Suite: \`${report.run.suite}\` · Bin: \`${report.run.bin}\``);
  lines.push("");
  lines.push(
    `**${report.summary.passed}/${report.summary.total} fixtures succeeded** · ` +
      `${report.summary.integrity_violations} integrity violation(s)`,
  );
  lines.push("");
  lines.push("| Fixture | Language | Tests | Integration | Integrity | Success | LLM calls | Input tok | Output tok | Wall ms |");
  lines.push("|---|---|---|---|---|---|---|---|---|---|");
  for (const r of report.fixtures) {
    lines.push(
      `| ${r.fixture} | ${r.language} | ${r.tests_passed ? "pass" : "FAIL"} | ` +
        `${r.integration_passed === null ? "n/a" : r.integration_passed ? "pass" : "FAIL"} | ` +
        `${r.integrity === "clean" ? "clean" : "**VIOLATION**"} | ${r.success ? "yes" : "no"} | ` +
        `${r.metrics.llm_calls} | ${r.metrics.input_tokens} | ${r.metrics.output_tokens} | ${r.wall_ms} |`,
    );
  }
  lines.push("");
  for (const r of report.fixtures) {
    if (r.integrity_details.length > 0 || r.error) {
      lines.push(`## ${r.fixture}`);
      lines.push("");
      if (r.error) lines.push(`- error: ${r.error}`);
      for (const detail of r.integrity_details) lines.push(`- integrity violation: ${detail}`);
      lines.push(`- exec: ${r.exec.note}`);
      lines.push(`- tmp dir: \`${r.tmp_dir}\``);
      lines.push("");
    }
  }
  return lines.join("\n") + "\n";
}

function timestampSlug(date: Date): string {
  return date.toISOString().replace(/[-:]/g, "").replace(/\.\d+Z$/, "Z");
}

async function writeReport(report: RunReport, label: string, timestamp: Date): Promise<{ jsonPath: string; mdPath: string }> {
  await fs.mkdir(REPORTS_DIR, { recursive: true });
  const base = `${timestampSlug(timestamp)}-${label}`;
  const jsonPath = path.join(REPORTS_DIR, `${base}.json`);
  const mdPath = path.join(REPORTS_DIR, `${base}.md`);
  await fs.writeFile(jsonPath, JSON.stringify(report, null, 2) + "\n", "utf8");
  await fs.writeFile(mdPath, renderMarkdown(report), "utf8");
  return { jsonPath, mdPath };
}

function slugifySuite(suite: string): string {
  return suite === "all" ? "all" : suite.replace(/,/g, "+");
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));

  let promptTemplate: string;
  try {
    promptTemplate = await fs.readFile(PROMPT_TEMPLATE_PATH, "utf8");
  } catch {
    fail(`could not read prompt template at ${PROMPT_TEMPLATE_PATH}`);
  }

  const available = await listFixtures();
  const selected = selectFixtures(args.suite, available);

  console.log(`[hackathon] agent=${args.agent} suite=${args.suite} fixtures=[${selected.join(", ")}]`);

  const results: FixtureResult[] = [];
  for (const name of selected) {
    console.log(`\n[hackathon] === ${name} ===`);
    const result = await runFixture(name, args, promptTemplate);
    results.push(result);
    console.log(
      `[hackathon] ${name}: tests=${result.tests_passed ? "pass" : "FAIL"} ` +
        `integration=${result.integration_passed === null ? "n/a" : result.integration_passed ? "pass" : "FAIL"} ` +
        `integrity=${result.integrity} success=${result.success} (tmp: ${result.tmp_dir})`,
    );
    if (result.integrity === "violation") {
      for (const detail of result.integrity_details) console.log(`[hackathon]   VIOLATION: ${detail}`);
    }
    if (result.error) console.log(`[hackathon]   error: ${result.error}`);
  }

  const timestamp = new Date();
  const report = buildReport(args, timestamp, results);
  const label = args.label ?? `${args.agent}-${slugifySuite(args.suite)}`;
  const { jsonPath, mdPath } = await writeReport(report, label, timestamp);

  console.log(`\n[hackathon] ${report.summary.passed}/${report.summary.total} fixtures succeeded`);
  console.log(`[hackathon] report: ${jsonPath}`);
  console.log(`[hackathon] report: ${mdPath}`);

  process.exit(report.summary.all_success ? 0 : 1);
}

main().catch((err) => {
  const message = err instanceof Error ? err.message : String(err);
  console.error(`\n[hackathon] FATAL: ${message}`);
  process.exit(1);
});
