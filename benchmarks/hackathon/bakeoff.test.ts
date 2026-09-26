import { test } from "node:test";
import assert from "node:assert/strict";

import { rank, runCost, summarise } from "./bakeoff.ts";

const run = (success: boolean, input: number, cached: number, output: number) => ({
  fixture: "f", success, outcome: success ? "completed" : "error",
  llm_calls: 3, input_tokens: input, cached_input_tokens: cached, output_tokens: output, wall_ms: 1000,
});

test("cached input is billed at the cache-read rate", () => {
  const actual = runCost(run(true, 1000, 400, 100), { prompt: 1e-6, cacheRead: 1e-7, completion: 1e-5 });
  const expected = 600 * 1e-6 + 400 * 1e-7 + 100 * 1e-5;
  assert.ok(Math.abs(actual - expected) < 1e-12, `${actual} != ${expected}`);
});

test("summarise totals runs and leaves cost unknown without pricing", () => {
  const actual = summarise("m", [run(true, 10, 0, 1), run(false, 20, 0, 2)], undefined);
  assert.equal(actual.successes, 1);
  assert.equal(actual.runs, 2);
  assert.equal(actual.input_tokens, 30);
  assert.equal(actual.usd, null);
});

test("rank puts success before cost", () => {
  const cheapFailing = { ...summarise("cheap", [run(false, 1, 0, 1)], undefined), usd: 0.01 };
  const dearPassing = { ...summarise("dear", [run(true, 1, 0, 1)], undefined), usd: 5 };
  assert.deepEqual(rank([cheapFailing, dearPassing]).map((r) => r.model), ["dear", "cheap"]);
});
