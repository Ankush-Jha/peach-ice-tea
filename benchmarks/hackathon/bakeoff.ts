#!/usr/bin/env node

// Model bake-off (MM.2; D-049, D-050): the same harness, the same fixtures, one model per arm.
//
// Each model runs the suite through run.ts under the `openrouter` profile, with the model chosen by
// FORGE_SESSION__MODEL_ID, so every arm differs only in the model. Models run in parallel (each
// run.ts has its own temp dirs); fixtures within a model run in sequence. Cost is priced from
// OpenRouter's live per-token rates against the token counts forge measured, never from model claims
// (HACKATHON §16). The report goes to benchmarks/reports/models/.

import * as fs from "fs/promises";
import * as path from "path";
import { fileURLToPath } from "url";
import { spawn } from "child_process";

import { wilson } from "./ab.ts";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "..", "..");
const RUN_TS = path.join(HERE, "run.ts");
const REPORTS_DIR = path.join(REPO_ROOT, "benchmarks", "reports", "models");

export interface Pricing {
  /** USD per token. */
  prompt: number;
  completion: number;
  cacheRead: number;
}

export interface FixtureRun {
  fixture: string;
  seed: number;
  /** The run never got a fair try: the provider refused for account reasons (no credit, exhausted quota).
   * Excluded from success rates, and counted separately, so an account limit is never read as a model result. */
  blocked: boolean;
  success: boolean;
  outcome: string;
  llm_calls: number;
  input_tokens: number;
  cached_input_tokens: number;
  output_tokens: number;
  wall_ms: number;
}

export interface ModelSummary {
  model: string;
  successes: number;
  /** Runs that reached a fair outcome (not blocked). */
  runs: number;
  blocked: number;
  llm_calls: number;
  input_tokens: number;
  output_tokens: number;
  wall_ms: number;
  usd: number | null;
}

/** USD for one run. Cached input is billed at the cache-read rate when OpenRouter lists one. */
export function runCost(run: FixtureRun, price: Pricing): number {
  const uncached = Math.max(0, run.input_tokens - run.cached_input_tokens);
  return uncached * price.prompt + run.cached_input_tokens * price.cacheRead + run.output_tokens * price.completion;
}

export function summarise(model: string, all: FixtureRun[], price: Pricing | undefined): ModelSummary {
  const runs = all.filter((r) => !r.blocked);
  const sum = (f: (r: FixtureRun) => number) => all.reduce((acc, r) => acc + f(r), 0);
  return {
    model,
    successes: runs.filter((r) => r.success).length,
    runs: runs.length,
    blocked: all.length - runs.length,
    llm_calls: sum((r) => r.llm_calls),
    input_tokens: sum((r) => r.input_tokens),
    output_tokens: sum((r) => r.output_tokens),
    wall_ms: sum((r) => r.wall_ms),
    usd: price ? sum((r) => runCost(r, price)) : null,
  };
}

/** A provider refusal for account reasons, from forge's own exec error (D-040, D-050). */
export function isAccountLimit(execError: string | undefined): boolean {
  return /provider quota exhausted|Invalid Status Code: 402/.test(execError ?? "");
}

/** Models with any fair run first; then success rate (principle 1), then cost, then wall time. */
export function rank(rows: ModelSummary[]): ModelSummary[] {
  return [...rows].sort(
    (a, b) =>
      Number(b.runs > 0) - Number(a.runs > 0) ||
      b.successes / Math.max(1, b.runs) - a.successes / Math.max(1, a.runs) ||
      (a.usd ?? Infinity) - (b.usd ?? Infinity) ||
      a.wall_ms - b.wall_ms,
  );
}

function fail(message: string): never {
  console.error(`[bakeoff] FATAL: ${message}`);
  process.exit(1);
}

async function pricing(): Promise<Map<string, Pricing>> {
  const res = await fetch("https://openrouter.ai/api/v1/models");
  if (!res.ok) fail(`could not fetch OpenRouter pricing: HTTP ${res.status}`);
  const body = (await res.json()) as { data: { id: string; pricing?: Record<string, string> }[] };
  const out = new Map<string, Pricing>();
  for (const m of body.data) {
    const p = m.pricing ?? {};
    const prompt = Number(p.prompt ?? 0);
    out.set(m.id, {
      prompt,
      completion: Number(p.completion ?? 0),
      cacheRead: p.input_cache_read !== undefined ? Number(p.input_cache_read) : prompt,
    });
  }
  return out;
}

function runModel(model: string, args: string[], logPath: string): Promise<string | null> {
  return new Promise((resolve) => {
    const env = { ...process.env, FORGE_SESSION__MODEL_ID: model };
    const child = spawn("npx", ["tsx", RUN_TS, "--agent", "forge", "--profile", "openrouter", ...args], {
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let out = "";
    const chunks: Buffer[] = [];
    child.stdout.on("data", (d: Buffer) => {
      out += d.toString();
      chunks.push(d);
    });
    child.stderr.on("data", (d: Buffer) => chunks.push(d));
    child.on("close", async () => {
      await fs.writeFile(logPath, Buffer.concat(chunks));
      const match = /report: (\S+\.json)/.exec(out);
      resolve(match?.[1] ?? null);
    });
  });
}

function successCell(r: ModelSummary): string {
  if (r.runs === 0) return "–";
  const [lo, hi] = wilson(r.successes, r.runs);
  return `${r.successes}/${r.runs} [${Math.round(lo * 100)}–${Math.round(hi * 100)}%]`;
}

function render(rows: ModelSummary[], perFixture: Map<string, FixtureRun[]>, meta: Record<string, string>): string {
  const fixtures = [...new Set([...perFixture.values()].flat().map((r) => r.fixture))];
  const lines = [
    `# Model bake-off — ${meta.label}`,
    "",
    `Profile \`openrouter\` · suite \`${meta.suite}\` · ${meta.seeds} seed(s) · ${meta.timestamp} · harness at \`${meta.commit}\``,
    "",
    "Ranked by success, then cost, then wall time. Cost is OpenRouter's list price applied to forge's own token counts.",
    "",
    "Blocked = the provider refused for account reasons (no credit, exhausted quota); excluded from success.",
    "",
    "| Model | Success (95% CI) | Blocked | LLM calls | Input tok | Output tok | Wall s | Est. USD |",
    "|---|---|---|---|---|---|---|---|",
    ...rows.map(
      (r) =>
        `| \`${r.model}\` | ${successCell(r)} | ${r.blocked} | ${r.llm_calls} | ${r.input_tokens.toLocaleString("en")} | ${r.output_tokens.toLocaleString("en")} | ${(r.wall_ms / 1000).toFixed(0)} | ${r.usd === null ? "?" : r.usd.toFixed(3)} |`,
    ),
    "",
    "## Per fixture (✓ pass / ✗ fail / ⊘ blocked, LLM calls; one cell per seed)",
    "",
    `| Model | ${fixtures.join(" | ")} |`,
    `|---|${fixtures.map(() => "---").join("|")}|`,
    ...rows.map((r) => {
      const runs = perFixture.get(r.model) ?? [];
      const cells = fixtures.map((f) => {
        const seeds = runs.filter((x) => x.fixture === f);
        if (seeds.length === 0) return "–";
        return seeds
          .map((run) => (run.blocked ? "⊘" : `${run.success ? "✓" : "✗"} ${run.llm_calls}${run.success ? "" : ` (${run.outcome})`}`))
          .join(", ");
      });
      return `| \`${r.model}\` | ${cells.join(" | ")} |`;
    }),
    "",
    `${meta.seeds} seed(s) per model. With one seed a single failure is weak evidence: shortlist, then rerun at k = 3.`,
    "",
  ];
  return lines.join("\n");
}

async function main() {
  const argv = process.argv.slice(2);
  let models: string[] = [];
  let suite = "all";
  let label = "bakeoff";
  let seeds = 1;
  const passThrough: string[] = [];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i] ?? "";
    const next = () => argv[++i] ?? fail(`${a} requires a value`);
    if (a === "--models") models = next().split(",").map((m) => m.trim()).filter(Boolean);
    else if (a === "--suite") suite = next();
    else if (a === "--label") label = next();
    else if (a === "--seeds") seeds = Number(next());
    else if (["--bin", "--max-requests", "--max-duration-secs", "--timeout-ms"].includes(a)) {
      const value = next();
      passThrough.push(a, value);
    } else fail(`unknown argument ${a} (use --models a,b --suite s --label l [--bin p] [--max-requests n] ...)`);
  }
  if (models.length === 0) fail("--models is required");
  if (!Number.isInteger(seeds) || seeds < 1) fail("--seeds must be a positive integer");
  if (!process.env.OPENROUTER_API_KEY) fail("OPENROUTER_API_KEY is not set");

  const prices = await pricing();
  for (const m of models) if (!prices.has(m)) fail(`OpenRouter does not list model "${m}"`);

  const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\..*/, "Z");
  const logDir = path.join(REPORTS_DIR, "logs", `${stamp}-${label}`);
  await fs.mkdir(logDir, { recursive: true });

  // Models in parallel; each model's seeds in sequence, so one model never competes with itself for rate limit.
  const reportPaths = await Promise.all(
    models.map(async (m) => {
      const slug = m.replace(/[^A-Za-z0-9.-]+/g, "_");
      const paths: (string | null)[] = [];
      for (let seed = 1; seed <= seeds; seed++) {
        console.log(`[bakeoff] start ${m} s${seed}`);
        const p = await runModel(
          m,
          ["--suite", suite, "--label", `bakeoff-${label}-${slug}-s${seed}`, ...passThrough],
          path.join(logDir, `${slug}-s${seed}.log`),
        );
        console.log(`[bakeoff] done  ${m} s${seed}${p ? "" : " (no report)"}`);
        paths.push(p);
      }
      return paths;
    }),
  );

  const perFixture = new Map<string, FixtureRun[]>();
  const rows: ModelSummary[] = [];
  for (const [i, model] of models.entries()) {
    const runs: FixtureRun[] = [];
    for (const [s, reportPath] of (reportPaths[i] ?? []).entries()) {
      if (!reportPath) continue;
      const report = JSON.parse(await fs.readFile(reportPath, "utf8"));
      for (const f of report.fixtures ?? []) {
        const execError = f.evidence_dir
          ? await fs
              .readFile(path.join(f.evidence_dir, "exec.json"), "utf8")
              .then((t) => JSON.parse(t).error as string | undefined)
              .catch(() => undefined)
          : undefined;
        runs.push({
          fixture: f.fixture,
          seed: s + 1,
          blocked: !f.success && isAccountLimit(execError),
          success: Boolean(f.success),
          outcome: f.exec?.outcome ?? f.error ?? "?",
          llm_calls: f.metrics?.llm_calls ?? 0,
          input_tokens: f.metrics?.input_tokens ?? 0,
          cached_input_tokens: f.metrics?.cached_input_tokens ?? 0,
          output_tokens: f.metrics?.output_tokens ?? 0,
          wall_ms: f.wall_ms ?? 0,
        });
      }
    }
    perFixture.set(model, runs);
    rows.push(summarise(model, runs, prices.get(model)));
  }

  const commit = await new Promise<string>((resolve) => {
    const c = spawn("git", ["rev-parse", "--short", "HEAD"], { cwd: REPO_ROOT });
    let s = "";
    c.stdout.on("data", (d: Buffer) => (s += d.toString()));
    c.on("close", () => resolve(s.trim() || "unknown"));
  });
  const ranked = rank(rows);
  const md = render(ranked, perFixture, { label, suite, seeds: String(seeds), timestamp: new Date().toISOString(), commit });
  const base = path.join(REPORTS_DIR, `${stamp.slice(0, 8)}-${label}`);
  await fs.writeFile(`${base}.md`, md);
  await fs.writeFile(`${base}.json`, JSON.stringify({ label, suite, commit, ranked, perFixture: Object.fromEntries(perFixture) }, null, 2));
  console.log(md);
  console.log(`[bakeoff] report: ${base}.md`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((e) => fail(String(e?.stack ?? e)));
}
