#!/usr/bin/env node

// Per-tool error rate by model (R-EVAL-3, T0.9; D-079), from the run reports peach's own
// metrics fill in (tool_calls / tool_errors per fixture), never from model claims.
//
//   node benchmarks/hackathon/tool_errors.ts <run-report.json> [...]
//
// A run report is what run.ts (and bakeoff.ts through it) writes to benchmarks/reports/hackathon/.
// The model comes from each run's own telemetry (model_call.model).

import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";

export interface Tally {
  calls: number;
  errors: number;
}

/** model → tool → calls/errors, summed over every fixture of every report. */
export type ToolErrorTable = Map<string, Map<string, Tally>>;

export function addFixture(table: ToolErrorTable, model: string, toolCalls: Record<string, number>, toolErrors: Record<string, number>) {
  const byTool = table.get(model) ?? new Map<string, Tally>();
  for (const [tool, calls] of Object.entries(toolCalls ?? {})) {
    const tally = byTool.get(tool) ?? { calls: 0, errors: 0 };
    tally.calls += calls;
    tally.errors += toolErrors?.[tool] ?? 0;
    byTool.set(tool, tally);
  }
  table.set(model, byTool);
}

export function render(table: ToolErrorTable): string {
  const tools = [...new Set([...table.values()].flatMap((byTool) => [...byTool.keys()]))].sort();
  const lines = [
    `| Model | ${tools.join(" | ")} |`,
    `|---|${tools.map(() => "---").join("|")}|`,
    ...[...table.entries()].map(([model, byTool]) => {
      const cells = tools.map((tool) => {
        const t = byTool.get(tool);
        return t ? `${t.errors}/${t.calls}${t.errors ? ` (${Math.round((t.errors * 100) / t.calls)}%)` : ""}` : "–";
      });
      return `| \`${model}\` | ${cells.join(" | ")} |`;
    }),
  ];
  return lines.join("\n");
}

/** The model the run actually called, from its own telemetry (`model_call.model`); the report's
 * file name only when the bundle is gone. */
function modelOf(reportPath: string, fixture: { evidence_dir?: string }): string {
  try {
    const lines = fs.readFileSync(path.join(fixture.evidence_dir ?? "", "telemetry.jsonl"), "utf8").split("\n");
    for (const line of lines) {
      const event = line.trim() ? JSON.parse(line).event : undefined;
      if (event?.type === "model_call" && event.model) return event.model;
    }
  } catch {
    // Fall through to the file name.
  }
  return path.basename(reportPath, ".json").replace(/^\d{8}T\d{6}Z-/, "");
}

function main() {
  const reports = process.argv.slice(2);
  if (reports.length === 0) {
    console.error("usage: node benchmarks/hackathon/tool_errors.ts <run-report.json> [...]");
    process.exit(2);
  }
  const table: ToolErrorTable = new Map();
  for (const report of reports) {
    const data = JSON.parse(fs.readFileSync(report, "utf8"));
    for (const fixture of data.fixtures ?? []) {
      // A run refused before its first tool call has nothing to rate.
      if (!fixture.metrics || Object.keys(fixture.metrics.tool_calls ?? {}).length === 0) continue;
      addFixture(table, modelOf(report, fixture), fixture.metrics.tool_calls, fixture.metrics.tool_errors);
    }
  }
  console.log(render(table));
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
