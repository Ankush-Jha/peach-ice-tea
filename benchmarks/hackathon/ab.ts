#!/usr/bin/env node

// A/B runner for the hackathon suite (PLAN.md W1-D step 5; CLAUDE.md principle 6).
//
// Runs a base arm and a candidate arm, which differ only by environment assignments (the
// harness's feature flags), over the same fixtures and seeds, through run.ts, which remains
// the one place that knows how to run and judge a fixture. It reports per arm: success rate
// with a 95% Wilson interval, integrity, input/output tokens, LLM calls and wall time, plus
// paired per-fixture deltas. The report goes to benchmarks/reports/ab/, which is committed,
// because an A/B report is the evidence a default change ships with.
//
// Money: a profile whose role is "evaluation" (gemini) spends a key's budget (D-025, D-025a).
// The runner prints an estimate and refuses to start such a run without --yes.

import * as fs from "fs/promises";
import * as path from "path";
import { fileURLToPath } from "url";
import { spawn } from "child_process";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "..", "..");
const RUN_TS = path.join(HERE, "run.ts");
const AB_REPORTS_DIR = path.join(REPO_ROOT, "benchmarks", "reports", "ab");

interface Args {
  suite: string;
  profile: string;
  seeds: number;
  base: Record<string, string>;
  cand: Record<string, string>;
  parallel: number;
  label: string;
  passThrough: string[];
  yes: boolean;
  estimateInrPerRun?: number;
}

function fail(message: string): never {
  console.error(`[ab] FATAL: ${message}`);
  process.exit(1);
}

function parseAssignments(raw: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const part of raw.split(",").map((p) => p.trim()).filter(Boolean)) {
    const eq = part.indexOf("=");
    if (eq <= 0) fail(`expected KEY=VALUE, got "${part}"`);
    out[part.slice(0, eq)] = part.slice(eq + 1);
  }
  return out;
}

function parseArgs(argv: string[]): Args {
  const args: Partial<Args> = { seeds: 3, parallel: 2, passThrough: [], yes: false, base: {}, cand: {} };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const next = () => argv[++i] ?? fail(`${a} requires a value`);
    switch (a) {
      case "--suite": args.suite = next(); break;
      case "--profile": args.profile = next(); break;
      case "--seeds": args.seeds = Number(next()); break;
      case "--base": args.base = parseAssignments(next()); break;
      case "--cand": args.cand = parseAssignments(next()); break;
      case "--parallel": args.parallel = Number(next()); break;
      case "--label": args.label = next(); break;
      case "--estimate-inr-per-run": args.estimateInrPerRun = Number(next()); break;
      case "--yes": args.yes = true; break;
      case "--max-requests":
      case "--max-duration-secs":
      case "--timeout-ms":
        args.passThrough!.push(a, next());
        break;
      case "-h":
      case "--help":
        console.log(
          "Usage: npm run hackathon:ab -- --profile <name> --suite <fixtures> --label <name>\n" +
            "         --base \"K=V,...\" --cand \"K=V,...\" [--seeds 3] [--parallel 2]\n" +
            "         [--max-requests N] [--max-duration-secs N] [--timeout-ms N]\n" +
            "         [--estimate-inr-per-run X] [--yes]",
        );
        process.exit(0);
      default:
        fail(`unknown argument ${a}`);
    }
  }
  if (!args.profile) fail("--profile is required");
  if (!args.suite) fail("--suite is required (name fixtures explicitly; A/B cost scales with it)");
  if (!args.label) fail("--label is required");
  if (!Number.isInteger(args.seeds) || args.seeds! < 1) fail("--seeds must be a positive integer");
  if (!Number.isInteger(args.parallel) || args.parallel! < 1) fail("--parallel must be a positive integer");
  // The arms may differ ONLY where declared: every key must be set in both, so nothing
  // leaks in from the caller's environment on one side only.
  const onlyOne = [
    ...Object.keys(args.cand!).filter((k) => !(k in args.base!)),
    ...Object.keys(args.base!).filter((k) => !(k in args.cand!)),
  ];
  if (onlyOne.length > 0) fail(`${onlyOne.join(", ")} set in only one arm; give both arms every key`);
  if (Object.keys(args.base!).length === 0) fail("--base and --cand must set at least one key");
  return args as Args;
}

async function profileRole(profile: string): Promise<string> {
  const env = await fs
    .readFile(path.join(REPO_ROOT, "configuration", "profiles", profile, "profile.env"), "utf8")
    .catch(() => fail(`profile "${profile}" not found`));
  return /^PROFILE_ROLE=(.*)$/m.exec(env)?.[1]?.trim() ?? "unspecified";
}

interface Job {
  arm: "base" | "cand";
  seed: number;
}

interface FixtureOutcome {
  fixture: string;
  arm: "base" | "cand";
  seed: number;
  success: boolean;
  integrity_clean: boolean;
  outcome: string | null;
  llm_calls: number;
  input_tokens: number;
  output_tokens: number;
  wall_ms: number;
}

function runJob(args: Args, job: Job): Promise<FixtureOutcome[]> {
  const env = { ...process.env, ...(job.arm === "base" ? args.base : args.cand) };
  const label = `ab-${args.label}-${job.arm}-s${job.seed}`;
  const childArgs = [
    RUN_TS, "--agent", "peach", "--profile", args.profile, "--suite", args.suite, "--label", label,
    ...args.passThrough,
  ];
  return new Promise((resolve, reject) => {
    const child = spawn("npx", ["tsx", ...childArgs], { env, stdio: ["ignore", "pipe", "pipe"] });
    let out = "";
    child.stdout.on("data", (c: Buffer) => (out += c.toString()));
    child.stderr.on("data", (c: Buffer) => (out += c.toString()));
    child.on("error", reject);
    child.on("close", async () => {
      const reportPath = /report: (\S+\.json)/.exec(out)?.[1];
      if (!reportPath) {
        reject(new Error(`run.ts produced no report for ${label}:\n${out.slice(-2000)}`));
        return;
      }
      const report = JSON.parse(await fs.readFile(reportPath, "utf8"));
      resolve(
        report.fixtures.map((f: any): FixtureOutcome => ({
          fixture: f.fixture,
          arm: job.arm,
          seed: job.seed,
          success: f.success,
          integrity_clean: f.integrity === "clean",
          outcome: f.exec?.outcome ?? null,
          llm_calls: f.metrics.llm_calls,
          input_tokens: f.metrics.input_tokens,
          output_tokens: f.metrics.output_tokens,
          wall_ms: f.metrics.wall_ms,
        })),
      );
      console.log(`[ab] done ${label}`);
    });
  });
}

/** 95% Wilson score interval for k successes out of n. */
export function wilson(k: number, n: number): [number, number] {
  if (n === 0) return [0, 0];
  const z = 1.96;
  const p = k / n;
  const denom = 1 + (z * z) / n;
  const centre = (p + (z * z) / (2 * n)) / denom;
  const half = (z * Math.sqrt((p * (1 - p)) / n + (z * z) / (4 * n * n))) / denom;
  return [Math.max(0, centre - half), Math.min(1, centre + half)];
}

const mean = (xs: number[]) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : NaN);

interface ArmSummary {
  runs: number;
  successes: number;
  success_rate: number;
  success_ci95: [number, number];
  integrity_violations: number;
  mean_input_tokens: number;
  mean_output_tokens: number;
  mean_llm_calls: number;
  mean_wall_ms: number;
}

export function summarise(rows: FixtureOutcome[]): ArmSummary {
  const k = rows.filter((r) => r.success).length;
  return {
    runs: rows.length,
    successes: k,
    success_rate: rows.length ? k / rows.length : NaN,
    success_ci95: wilson(k, rows.length),
    integrity_violations: rows.filter((r) => !r.integrity_clean).length,
    mean_input_tokens: mean(rows.map((r) => r.input_tokens)),
    mean_output_tokens: mean(rows.map((r) => r.output_tokens)),
    mean_llm_calls: mean(rows.map((r) => r.llm_calls)),
    mean_wall_ms: mean(rows.map((r) => r.wall_ms)),
  };
}

function pct(delta: number, base: number): string {
  if (!Number.isFinite(delta) || !Number.isFinite(base) || base === 0) return "n/a";
  return `${delta >= 0 ? "+" : ""}${((delta / base) * 100).toFixed(1)}%`;
}

function renderMarkdown(meta: Record<string, unknown>, base: ArmSummary, cand: ArmSummary, rows: FixtureOutcome[]): string {
  const fmt = (x: number) => (Number.isFinite(x) ? Math.round(x).toLocaleString("en-US") : "n/a");
  const ci = (s: ArmSummary) => `${(s.success_rate * 100).toFixed(0)}% [${(s.success_ci95[0] * 100).toFixed(0)}–${(s.success_ci95[1] * 100).toFixed(0)}%]`;
  const lines = [
    `# A/B — ${meta.label}`,
    "",
    `Profile \`${meta.profile}\` (${meta.role}) · suite \`${meta.suite}\` · seeds ${meta.seeds} · ${meta.timestamp}`,
    "",
    `Base: \`${JSON.stringify(meta.base)}\` · Candidate: \`${JSON.stringify(meta.cand)}\``,
    "",
    "| Metric | Base | Candidate | Δ |",
    "|---|---|---|---|",
    `| Success (95% CI) | ${ci(base)} | ${ci(cand)} | ${((cand.success_rate - base.success_rate) * 100).toFixed(0)} pts |`,
    `| Integrity violations | ${base.integrity_violations} | ${cand.integrity_violations} | |`,
    `| Input tokens / run | ${fmt(base.mean_input_tokens)} | ${fmt(cand.mean_input_tokens)} | ${pct(cand.mean_input_tokens - base.mean_input_tokens, base.mean_input_tokens)} |`,
    `| Output tokens / run | ${fmt(base.mean_output_tokens)} | ${fmt(cand.mean_output_tokens)} | ${pct(cand.mean_output_tokens - base.mean_output_tokens, base.mean_output_tokens)} |`,
    `| LLM calls / run | ${base.mean_llm_calls.toFixed(1)} | ${cand.mean_llm_calls.toFixed(1)} | ${pct(cand.mean_llm_calls - base.mean_llm_calls, base.mean_llm_calls)} |`,
    `| Wall time / run | ${fmt(base.mean_wall_ms / 1000)} s | ${fmt(cand.mean_wall_ms / 1000)} s | ${pct(cand.mean_wall_ms - base.mean_wall_ms, base.mean_wall_ms)} |`,
    "",
    "## Per run",
    "",
    "| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |",
    "|---|---|---|---|---|---|---|---|---|",
    ...rows
      .slice()
      .sort((a, b) => a.fixture.localeCompare(b.fixture) || a.seed - b.seed || a.arm.localeCompare(b.arm))
      .map(
        (r) =>
          `| ${r.fixture} | ${r.seed} | ${r.arm} | ${r.success ? "yes" : "no"} | ${r.outcome ?? "?"} | ${r.llm_calls} | ${r.input_tokens} | ${r.output_tokens} | ${(r.wall_ms / 1000).toFixed(0)} |`,
      ),
    "",
    `Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).`,
  ];
  return lines.join("\n") + "\n";
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));
  const role = await profileRole(args.profile);
  const fixtures = args.suite.split(",").filter(Boolean).length;
  const runs = fixtures * args.seeds * 2;
  const estimate = args.estimateInrPerRun !== undefined ? `≈ ₹${(runs * args.estimateInrPerRun).toFixed(0)}` : "not given";
  console.log(`[ab] ${runs} fixture-runs (${fixtures} fixtures × ${args.seeds} seeds × 2 arms), profile ${args.profile} (${role}); estimate ${estimate}`);
  if (role === "evaluation" && !args.yes) {
    fail("this profile spends a paid key's budget (D-025): pass --estimate-inr-per-run and --yes to proceed");
  }

  const jobs: Job[] = [];
  for (let seed = 1; seed <= args.seeds; seed++) jobs.push({ arm: "base", seed }, { arm: "cand", seed });
  const rows: FixtureOutcome[] = [];
  const queue = [...jobs];
  await Promise.all(
    Array.from({ length: Math.min(args.parallel, jobs.length) }, async () => {
      for (let job = queue.shift(); job; job = queue.shift()) rows.push(...(await runJob(args, job)));
    }),
  );

  const base = summarise(rows.filter((r) => r.arm === "base"));
  const cand = summarise(rows.filter((r) => r.arm === "cand"));
  const timestamp = new Date().toISOString();
  const meta = { label: args.label, profile: args.profile, role, suite: args.suite, seeds: args.seeds, base: args.base, cand: args.cand, timestamp };
  await fs.mkdir(AB_REPORTS_DIR, { recursive: true });
  const stem = path.join(AB_REPORTS_DIR, `${timestamp.slice(0, 10)}-${args.label}`);
  await fs.writeFile(`${stem}.json`, JSON.stringify({ meta, base, cand, runs: rows }, null, 2) + "\n");
  await fs.writeFile(`${stem}.md`, renderMarkdown(meta, base, cand, rows));
  console.log(`[ab] report: ${stem}.md`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((err) => fail(err instanceof Error ? err.message : String(err)));
}
