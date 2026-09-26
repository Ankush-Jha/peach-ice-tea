#!/usr/bin/env node

// Behavioural checks over real runs (R-EVAL-4, T0.7; D-078).
//
// Reads an evidence bundle's telemetry.jsonl and checks what the agent actually did, not what it
// said: every edited file was read first; the tests ran green after the last edit; withheld output
// was recovered from its handle rather than by blindly re-running the command; multi-file work used
// the todo list. Each check is pass / fail / n/a (the run gave it nothing to judge), never a guess.
//
//   node benchmarks/hackathon/behaviour.ts <evidence-dir> [<evidence-dir> ...]
//
// Exit code 1 if any check failed, so it can gate a suite.

import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";

export type Verdict = "pass" | "fail" | "n/a";

export interface CheckResult {
  check: string;
  verdict: Verdict;
  detail: string;
}

interface ToolCallEvent {
  type: "tool_call";
  name: string;
  success?: boolean;
  arguments?: { text?: string };
  result_summary?: { text?: string };
}

interface TestRunEvent {
  type: "test_run";
  exit_code?: number | null;
  origin?: string;
}

type Event = ToolCallEvent | TestRunEvent | { type: string };

const EDIT_TOOLS = new Set(["write", "patch", "multi_patch", "remove"]);

function args(event: ToolCallEvent): Record<string, unknown> {
  try {
    return JSON.parse(event.arguments?.text ?? "{}");
  } catch {
    return {};
  }
}

/** The call's target file. Telemetry truncates long arguments (a large `write`), which breaks the
 * JSON, so fall back to the leading `"file_path": "..."` field, which survives truncation. */
function filePath(event: ToolCallEvent): string | undefined {
  const a = args(event);
  const p = a.file_path ?? a.path;
  if (typeof p === "string") return p;
  return /"(?:file_path|path)"\s*:\s*"([^"]+)"/.exec(event.arguments?.text ?? "")?.[1];
}

/** An edit that happened: the runtime refuses some attempts (forge's own read-before-edit guard,
 * the test-integrity guard), and a refused attempt changed nothing. */
function isEdit(event: Event): event is ToolCallEvent {
  return event.type === "tool_call" && EDIT_TOOLS.has((event as ToolCallEvent).name) && (event as ToolCallEvent).success !== false;
}

function refusedEdits(events: Event[]): number {
  return events.filter(
    (e) => e.type === "tool_call" && EDIT_TOOLS.has((e as ToolCallEvent).name) && (e as ToolCallEvent).success === false,
  ).length;
}

/** Every edit to an existing file was preceded by a read of it. Creating a new file needs no read. */
export function readBeforePatch(events: Event[]): CheckResult {
  const read = new Set<string>();
  const unread: string[] = [];
  let edits = 0;
  for (const event of events) {
    if (event.type !== "tool_call") continue;
    const call = event as ToolCallEvent;
    const file = filePath(call);
    if (call.name === "read" && file && call.success !== false) read.add(file);
    if (isEdit(call) && file) {
      const overwrite = args(call).overwrite === true || /"overwrite"\s*:\s*true/.test(call.arguments?.text ?? "");
      const creates = call.name === "write" && !overwrite;
      edits += 1;
      if (!creates && !read.has(file)) unread.push(`${call.name} ${file}`);
      read.add(file);
    }
  }
  const refused = refusedEdits(events);
  const note = refused ? `; ${refused} attempt(s) refused by the runtime` : "";
  if (edits === 0) return { check: "read_before_patch", verdict: "n/a", detail: `no edits${note}` };
  return unread.length === 0
    ? { check: "read_before_patch", verdict: "pass", detail: `${edits} edit(s), each after a read${note}` }
    : { check: "read_before_patch", verdict: "fail", detail: `edited without reading: ${unread.join(", ")}` };
}

/** After the last edit, the agent itself ran the tests and they passed. */
export function verifiedAfterLastEdit(events: Event[]): CheckResult {
  let lastEdit = -1;
  events.forEach((event, i) => {
    if (isEdit(event)) lastEdit = i;
  });
  if (lastEdit < 0) return { check: "verified_after_last_edit", verdict: "n/a", detail: "no edits" };
  const green = events
    .slice(lastEdit + 1)
    .some((e) => e.type === "test_run" && (e as TestRunEvent).origin === "agent" && (e as TestRunEvent).exit_code === 0);
  return green
    ? { check: "verified_after_last_edit", verdict: "pass", detail: "a green agent test run followed the last edit" }
    : { check: "verified_after_last_edit", verdict: "fail", detail: "no green agent test run after the last edit" };
}

/** Output the harness withheld was recovered from its handle, not by re-running the same command blind. */
export function truncationAwareness(events: Event[]): CheckResult {
  const calls = events.filter((e) => e.type === "tool_call") as ToolCallEvent[];
  let withheld = 0;
  let recovered = 0;
  let blindReruns = 0;
  calls.forEach((call, i) => {
    const summary = call.result_summary?.text ?? "";
    if (!summary.includes("not shown")) return;
    withheld += 1;
    const handle = /read (\S+?) \(/.exec(summary)?.[1];
    const later = calls.slice(i + 1);
    if (handle && later.some((c) => c.name === "read" && filePath(c) === handle)) recovered += 1;
    const command = args(call).command;
    if (call.name === "shell" && later.some((c) => c.name === "shell" && args(c).command === command)) blindReruns += 1;
  });
  if (withheld === 0) return { check: "truncation_awareness", verdict: "n/a", detail: "nothing was withheld" };
  const detail = `${withheld} withheld, ${recovered} recovered via handle, ${blindReruns} blind re-run(s)`;
  return { check: "truncation_awareness", verdict: blindReruns === 0 ? "pass" : "fail", detail };
}

/** Work that edits two or more files uses the todo list. Smaller tasks are n/a: todos would be overhead. */
export function todoUsage(events: Event[]): CheckResult {
  const calls = events.filter((e) => e.type === "tool_call") as ToolCallEvent[];
  const edited = new Set(calls.filter(isEdit).map(filePath).filter(Boolean));
  if (edited.size < 2) return { check: "todo_usage", verdict: "n/a", detail: `${edited.size} file(s) edited` };
  const used = calls.some((c) => c.name === "todo_write");
  return {
    check: "todo_usage",
    verdict: used ? "pass" : "fail",
    detail: `${edited.size} files edited; todo_write ${used ? "used" : "never used"}`,
  };
}

export function checkEvents(events: Event[]): CheckResult[] {
  return [readBeforePatch(events), verifiedAfterLastEdit(events), truncationAwareness(events), todoUsage(events)];
}

export function readTelemetry(dir: string): Event[] {
  return fs
    .readFileSync(path.join(dir, "telemetry.jsonl"), "utf8")
    .split("\n")
    .filter((line) => line.trim())
    .map((line) => JSON.parse(line).event as Event);
}

function main() {
  const dirs = process.argv.slice(2);
  if (dirs.length === 0) {
    console.error("usage: node benchmarks/hackathon/behaviour.ts <evidence-dir> [...]");
    process.exit(2);
  }
  let failed = false;
  console.log("| Run | read_before_patch | verified_after_last_edit | truncation_awareness | todo_usage |");
  console.log("|---|---|---|---|---|");
  for (const dir of dirs) {
    const results = checkEvents(readTelemetry(dir));
    failed ||= results.some((r) => r.verdict === "fail");
    console.log(`| ${path.basename(path.dirname(dir))}/${path.basename(dir)} | ${results.map((r) => r.verdict).join(" | ")} |`);
    for (const r of results.filter((r) => r.verdict === "fail")) console.log(`|  ↳ ${r.check}: ${r.detail} | | | | |`);
  }
  process.exit(failed ? 1 : 0);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
