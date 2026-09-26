import { test } from "node:test";
import assert from "node:assert/strict";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import { autoProfile, keyFor, parseProfileModels, projectStats, splitTelemetry, within } from "./lib.ts";

test("a key only reaches the provider its shape belongs to", () => {
  const env = { AI_API_KEY: "sk-or-v1-abc" };

  assert.equal(keyFor({ name: "openrouter", keyVar: "OPENROUTER_API_KEY" }, env), "sk-or-v1-abc");
  assert.equal(keyFor({ name: "gemini", keyVar: "GEMINI_API_KEY" }, env), undefined, "an OpenRouter key must not go to Gemini");
  assert.equal(keyFor({ name: "nvidia-nim", keyVar: "NVIDIA_API_KEY" }, { NVIDIA_API_KEY: "nvapi-x" }), "nvapi-x");
  assert.equal(keyFor({ name: "gemini", keyVar: "GEMINI_API_KEY" }, { AI_API_KEY: "unknown-shape" }), "unknown-shape");
  assert.equal(keyFor({ name: "deepseek", keyVar: "DEEPSEEK_API_KEY" }, { AI_API_KEY: "unknown-shape" }), undefined);
});

test("the automatic profile matches harness/select-profile", () => {
  assert.equal(autoProfile({ AI_API_KEY: "sk-or-1" }), "openrouter");
  assert.equal(autoProfile({ AI_API_KEY: "nvapi-1" }), "nvidia-deepseek");
  assert.equal(autoProfile({ AI_API_KEY: "AIza1" }), "gemini");
  assert.equal(autoProfile({ PROFILE: "deepseek", AI_API_KEY: "sk-or-1" }), "deepseek");
});

test("paths outside a root are refused", () => {
  assert.equal(within("/h/evidence", "run-1"), true);
  assert.equal(within("/h/evidence", "../secrets"), false);
  assert.equal(within("/h/evidence", "/etc/passwd"), false);
});

test("profile models include the session default first", () => {
  const actual = parseProfileModels('[[providers.models]]\nid = "a"\n[[providers.models]]\nid = "b"\n[session]\nmodel_id = "b"\n');

  assert.deepEqual(actual, { defaultModel: "b", models: ["a", "b"] });
});

test("telemetry is split into complete events, keeping a half-written line", () => {
  const actual = splitTelemetry('{"seq":0}\n{"seq":1}\n{"se');

  assert.deepEqual(actual, { events: [{ seq: 0 }, { seq: 1 }], rest: '{"se' });
});

test("project stats tally checked and unchecked tasks, and decision headings", () => {
  const harness = fs.mkdtempSync(path.join(os.tmpdir(), "peach-ice-tea-stats-"));
  fs.mkdirSync(path.join(harness, "docs", "harness"), { recursive: true });
  fs.writeFileSync(path.join(harness, "docs", "harness", "TASKS.md"), "- [x] one\n- [ ] two\n- [x] three\n");
  fs.writeFileSync(path.join(harness, "docs", "harness", "DECISIONS.md"), "## D-001 — a\n\ntext\n\n## D-002 — b\n");

  const actual = projectStats(harness);

  assert.deepEqual(actual, { tasksDone: 2, tasksTotal: 3, decisions: 2 });
  fs.rmSync(harness, { recursive: true, force: true });
});

test("project stats fail open when the docs are missing, rather than throwing", () => {
  const actual = projectStats(fs.mkdtempSync(path.join(os.tmpdir(), "peach-ice-tea-empty-")));

  assert.deepEqual(actual, { tasksDone: 0, tasksTotal: 0, decisions: 0 });
});
