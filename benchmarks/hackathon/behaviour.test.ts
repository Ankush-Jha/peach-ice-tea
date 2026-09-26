import { test } from "node:test";
import assert from "node:assert/strict";

import { readBeforePatch, todoUsage, truncationAwareness, verifiedAfterLastEdit } from "./behaviour.ts";

const call = (name: string, a: Record<string, unknown>, summary = "") => ({
  type: "tool_call",
  name,
  arguments: { text: JSON.stringify(a) },
  result_summary: { text: summary },
});
const testRun = (exit_code: number, origin = "agent") => ({ type: "test_run", exit_code, origin });

test("an edit after a read passes, an edit without one fails, and creating a file needs no read", () => {
  const good = [call("read", { file_path: "a.py" }), call("patch", { file_path: "a.py" }), call("write", { file_path: "new.py" })];
  const bad = [call("patch", { file_path: "a.py" })];

  assert.equal(readBeforePatch(good).verdict, "pass");
  assert.equal(readBeforePatch(bad).verdict, "fail");
  assert.equal(readBeforePatch([]).verdict, "n/a");
});

test("only a green agent run after the last edit counts as verified", () => {
  const edit = call("patch", { file_path: "a.py" });

  assert.equal(verifiedAfterLastEdit([edit, testRun(0)]).verdict, "pass");
  assert.equal(verifiedAfterLastEdit([testRun(0), edit]).verdict, "fail", "green before the edit proves nothing");
  assert.equal(verifiedAfterLastEdit([edit, testRun(1), testRun(0, "harness_final")]).verdict, "fail", "the harness's own run is not the agent verifying");
});

test("withheld output recovered via its handle passes; a blind re-run of the same command fails", () => {
  const truncated = call("shell", { command: "cargo test" }, "10 more lines not shown. Full output: read /tmp/x.txt (lines 1-9).");

  assert.equal(truncationAwareness([truncated, call("read", { file_path: "/tmp/x.txt" })]).verdict, "pass");
  assert.equal(truncationAwareness([truncated, call("shell", { command: "cargo test" })]).verdict, "fail");
  assert.equal(truncationAwareness([call("shell", { command: "ls" }, "a b")]).verdict, "n/a");
});

test("todo use is judged only on work that edits several files", () => {
  const two = [call("patch", { file_path: "a.py" }), call("patch", { file_path: "b.py" })];

  assert.equal(todoUsage(two).verdict, "fail");
  assert.equal(todoUsage([...two, call("todo_write", {})]).verdict, "pass");
  assert.equal(todoUsage([call("patch", { file_path: "a.py" })]).verdict, "n/a");
});

test("a refused edit is not an edit, and a truncated write still yields its path", () => {
  const refused = { ...call("patch", { file_path: "a.py" }), success: false };
  const truncatedWrite = {
    type: "tool_call",
    name: "write",
    success: true,
    arguments: { text: '{"file_path":"big.py","overwrite":true,"content":"xxxxxxxx' },
    result_summary: { text: "" },
  };

  assert.equal(readBeforePatch([refused]).verdict, "n/a");
  assert.match(readBeforePatch([refused]).detail, /1 attempt\(s\) refused/);
  assert.equal(readBeforePatch([truncatedWrite]).verdict, "fail", "an unread overwrite, found despite truncation");
});
