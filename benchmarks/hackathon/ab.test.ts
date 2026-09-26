import { test } from "node:test";
import assert from "node:assert/strict";

import { summarise, wilson } from "./ab.ts";

test("wilson interval matches the textbook values", () => {
  const [lo, hi] = wilson(8, 10);
  assert.ok(Math.abs(lo - 0.4902) < 1e-3, `lo ${lo}`);
  assert.ok(Math.abs(hi - 0.9433) < 1e-3, `hi ${hi}`);
  assert.deepEqual(wilson(0, 0), [0, 0]);
  const [lo0] = wilson(0, 3);
  assert.equal(lo0, 0);
});

test("summarise counts successes, violations and means", () => {
  const row = (success: boolean, clean: boolean, input: number) => ({
    fixture: "f", arm: "base" as const, seed: 1, success, integrity_clean: clean, outcome: "completed",
    llm_calls: 4, input_tokens: input, output_tokens: 10, wall_ms: 1000,
  });

  const actual = summarise([row(true, true, 100), row(false, false, 300)]);

  assert.equal(actual.runs, 2);
  assert.equal(actual.successes, 1);
  assert.equal(actual.integrity_violations, 1);
  assert.equal(actual.mean_input_tokens, 200);
});
