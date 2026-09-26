import { test } from "node:test";
import assert from "node:assert/strict";

import { addFixture, render, type ToolErrorTable } from "./tool_errors.ts";

test("errors and calls are summed per model and tool, and rendered as a rate", () => {
  const table: ToolErrorTable = new Map();
  addFixture(table, "m", { read: 3, patch: 2 }, { patch: 1 });
  addFixture(table, "m", { read: 1 }, {});

  const actual = render(table);

  assert.equal(actual, "| Model | patch | read |\n|---|---|---|\n| `m` | 1/2 (50%) | 0/4 |");
});
