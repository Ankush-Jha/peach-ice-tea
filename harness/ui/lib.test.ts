import { test } from "node:test";
import assert from "node:assert/strict";

import { autoProfile, keyFor, parseProfileModels, splitTelemetry, within } from "./lib.ts";

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
