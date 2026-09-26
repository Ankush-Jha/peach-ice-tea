// Pure helpers for the local UI server (harness/ui/server.ts, D-092). Kept apart from the
// server so they can be tested without spawning anything.

import * as fs from "fs";
import * as path from "path";

/** A configuration profile as the UI offers it. Never carries a key, only whether one exists. */
export interface Profile {
  name: string;
  keyVar: string;
  role: string;
  defaultModel: string | null;
  models: string[];
  keyAvailable: boolean;
}

/** Provider key variables the harness knows; the child gets none of them directly. */
export const PROVIDER_KEY_VARS = [
  "AI_API_KEY",
  "GEMINI_API_KEY",
  "DEEPSEEK_API_KEY",
  "NVIDIA_API_KEY",
  "OPENROUTER_API_KEY",
  "OPENAI_API_KEY",
  "ANTHROPIC_API_KEY",
];

/** The provider variable a key's shape belongs to, mirroring harness/select-profile (D-081). */
export function keyVarOfShape(key: string): string | null {
  if (key.startsWith("sk-or-")) return "OPENROUTER_API_KEY";
  if (key.startsWith("nvapi-")) return "NVIDIA_API_KEY";
  if (key.startsWith("AIza")) return "GEMINI_API_KEY";
  return null;
}

/**
 * The key a run on `profile` should use: the profile's own variable if set, else AI_API_KEY when
 * its shape belongs to that provider (an unrecognised shape only for gemini, select-profile's
 * default). Never a key of another provider's shape.
 */
export function keyFor(profile: Pick<Profile, "name" | "keyVar">, env: NodeJS.ProcessEnv): string | undefined {
  const own = env[profile.keyVar];
  if (own) return own;
  const ai = env.AI_API_KEY;
  if (!ai) return undefined;
  const shape = keyVarOfShape(ai);
  if (shape === profile.keyVar) return ai;
  if (shape === null && profile.name === "gemini") return ai;
  return undefined;
}

/** The profile `make run` would pick for AI_API_KEY, as harness/select-profile does. */
export function autoProfile(env: NodeJS.ProcessEnv): string {
  if (env.PROFILE) return env.PROFILE;
  const ai = env.AI_API_KEY ?? "";
  if (ai.startsWith("sk-or-")) return "openrouter";
  if (ai.startsWith("nvapi-")) return "nvidia-deepseek";
  return "gemini";
}

/** Reads `KEY=value` lines, skipping comments and blanks. */
export function parseEnvFile(text: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eq = trimmed.indexOf("=");
    if (eq > 0) out[trimmed.slice(0, eq)] = trimmed.slice(eq + 1);
  }
  return out;
}

/** Model ids a profile's peach.toml registers, and its session default. */
export function parseProfileModels(toml: string): { defaultModel: string | null; models: string[] } {
  const models = [...toml.matchAll(/^\s*id\s*=\s*"([^"]+)"/gm)].map((m) => m[1]);
  const def = toml.match(/^\s*model_id\s*=\s*"([^"]+)"/m)?.[1] ?? null;
  const all = def && !models.includes(def) ? [def, ...models] : models;
  return { defaultModel: def, models: all };
}

/** Every profile under configuration/profiles, with key availability for this process. */
export function listProfiles(harness: string, env: NodeJS.ProcessEnv): Profile[] {
  const dir = path.join(harness, "configuration", "profiles");
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .filter((d) => d.isDirectory() && fs.existsSync(path.join(dir, d.name, "profile.env")))
    .map((d) => {
      const vars = parseEnvFile(fs.readFileSync(path.join(dir, d.name, "profile.env"), "utf8"));
      const tomlPath = path.join(dir, d.name, "peach.toml");
      const toml = fs.existsSync(tomlPath) ? fs.readFileSync(tomlPath, "utf8") : "";
      const profile = { name: d.name, keyVar: vars.PROFILE_KEY_VAR ?? "", role: vars.PROFILE_ROLE ?? "" };
      return { ...profile, ...parseProfileModels(toml), keyAvailable: keyFor(profile, env) !== undefined };
    })
    .sort((a, b) => (a.role === b.role ? a.name.localeCompare(b.name) : a.role === "evaluation" ? -1 : 1));
}

/** Whether `candidate` resolves inside `root` (no `..` escapes, no absolute jumps). */
export function within(root: string, candidate: string): boolean {
  const rel = path.relative(path.resolve(root), path.resolve(root, candidate));
  return rel === "" || (!rel.startsWith("..") && !path.isAbsolute(rel));
}

/** A one-line summary of an evidence bundle, from its report.json (or exec.json while partial). */
export interface BundleSummary {
  id: string;
  root: string;
  name: string;
  outcome: string;
  model: string | null;
  calls: number | null;
  inputTokens: number | null;
  outputTokens: number | null;
  wallMs: number | null;
  tests: string | null;
  mtimeMs: number;
}

function readJson(file: string): any {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch {
    return null;
  }
}

/** Summarises the bundle at `dir`; `null` when it holds no evidence at all. */
export function summarizeBundle(rootId: string, dir: string): BundleSummary | null {
  const report = readJson(path.join(dir, "report.json"));
  const exec = readJson(path.join(dir, "exec.json"));
  if (!report && !exec && !fs.existsSync(path.join(dir, "telemetry.jsonl"))) return null;
  const testing = report?.testing;
  const tests =
    testing && testing.ran ? `${testing.class} ${testing.passed ?? "?"}/${(testing.passed ?? 0) + (testing.failed ?? 0)}` : null;
  return {
    id: `${rootId}/${path.basename(dir)}`,
    root: rootId,
    name: path.basename(dir),
    outcome: report?.outcome?.outcome ?? exec?.outcome ?? "running",
    model: report?.outcome?.model ?? null,
    calls: report?.model_calls?.calls ?? null,
    inputTokens: report?.tokens?.input ?? null,
    outputTokens: report?.tokens?.output ?? null,
    wallMs: report?.execution?.wall_ms ?? null,
    tests,
    mtimeMs: fs.statSync(dir).mtimeMs,
  };
}

/** Splits appended telemetry text into complete events plus the unfinished tail. */
export function splitTelemetry(buffered: string): { events: any[]; rest: string } {
  const lines = buffered.split("\n");
  const rest = lines.pop() ?? "";
  const events = lines.flatMap((line) => {
    if (!line.trim()) return [];
    try {
      return [JSON.parse(line)];
    } catch {
      return [];
    }
  });
  return { events, rest };
}
