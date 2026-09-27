import type { Envelope } from "./api";
import { fmtNum } from "./utils";

export type SpanKind = "agent" | "model" | "tool" | "io" | "marker";
export type SpanStatus = "ok" | "error";

export interface TraceSpan {
  id: string;
  label: string;
  kind: SpanKind;
  status?: SpanStatus;
  agentId?: string | null;
  parentId?: string;
  start: number;
  end: number;
  detail?: string;
}

export interface LaidSpan extends TraceSpan {
  depth: number;
  dur: number;
}

function toolArgs(ev: any): string {
  let args: any = {};
  try {
    args = JSON.parse(ev.arguments?.text ?? "{}");
  } catch {
    return String(ev.arguments?.text ?? "").slice(0, 160);
  }
  const pick =
    args.command ?? args.file_path ?? args.path ?? args.pattern ?? args.note ?? args.url ?? args.agent_id ??
    (args.todos ? `${args.todos.length} item(s)` : "");
  return String(pick).slice(0, 160);
}

/** Envelopes → spans with a start/end on a millisecond axis from the run's first event. */
export function buildSpans(telemetry: Envelope[], nowFallback?: number): TraceSpan[] {
  let runStartTs: number | null = null;
  for (const env of telemetry) {
    if (env.event?.type === "run_start") {
      runStartTs = Date.parse(env.timestamp);
      break;
    }
  }
  if (runStartTs == null) runStartTs = telemetry.length ? Date.parse(telemetry[0].timestamp) : Date.now();
  const rel = (ts?: number | null) => Math.max(0, (ts ?? runStartTs!) - runStartTs!);

  let mainAgentId: string | null = null;
  for (const env of telemetry) {
    if (env.agent_id) {
      mainAgentId = env.agent_id;
      break;
    }
  }

  const spans: TraceSpan[] = [];
  const agentOpen = new Map<string, number>();
  let seq = 0;

  for (const env of telemetry) {
    const ev = env.event || {};
    const ts = env.timestamp ? Date.parse(env.timestamp) : null;
    const agentId = env.agent_id || null;
    const parentId = agentId && agentId !== mainAgentId ? `agent:${agentId}` : undefined;
    const id = `s${seq++}`;

    if (ev.type === "agent_state" && agentId) {
      if (ev.to === "running") {
        agentOpen.set(agentId, ts ?? runStartTs!);
      } else if (ev.to === "ended") {
        if (agentId !== mainAgentId) {
          const startTs = agentOpen.has(agentId) ? agentOpen.get(agentId)! : ts ?? runStartTs!;
          spans.push({
            id: `agent:${agentId}`,
            label: `sub-agent · ${agentId.slice(0, 8)}`,
            kind: "agent",
            agentId,
            start: rel(startTs),
            end: rel(ts),
            detail: ev.iteration != null ? `${fmtNum(ev.iteration)} call(s)` : "finished",
          });
        }
        agentOpen.delete(agentId);
      } else {
        spans.push({
          id,
          label: `agent → ${ev.to}`,
          kind: "marker",
          status: /exhaust|unconfirm|fail/i.test(ev.to || "") ? "error" : "ok",
          agentId,
          start: rel(ts),
          end: rel(ts) + 160,
        });
      }
    } else if (ev.type === "model_call") {
      spans.push({
        id,
        label: `model #${String(ev.call_id || "").split("#").pop()}`,
        kind: "model",
        agentId,
        parentId,
        start: rel(ts) - (ev.duration_ms || 0),
        end: rel(ts),
        detail: `${fmtNum(ev.input_tokens)} in · ${fmtNum(ev.output_tokens)} out`,
      });
    } else if (ev.type === "tool_call") {
      spans.push({
        id,
        label: ev.name,
        kind: "tool",
        status: ev.success === false ? "error" : "ok",
        agentId,
        parentId,
        start: rel(ts) - (ev.duration_ms || 0),
        end: rel(ts),
        detail: toolArgs(ev),
      });
    } else if (ev.type === "test_run") {
      const ok = ev.exit_code === 0;
      spans.push({
        id,
        label: "run_tests",
        kind: "io",
        status: ok ? "ok" : "error",
        agentId,
        parentId,
        start: rel(ts) - (ev.duration_ms || 0),
        end: rel(ts),
        detail: ev.passed != null ? `${ev.passed} passed / ${ev.failed ?? 0} failed` : ev.failure_class || (ok ? "passed" : "failed"),
      });
    } else if (["recovery", "retry", "context_compaction", "prompt_suppressed", "error", "integrity"].includes(ev.type)) {
      const label =
        ev.type === "recovery" ? `recovery · ${ev.action || ""}` :
        ev.type === "retry" ? `retry ${ev.attempt || ""}` :
        ev.type === "context_compaction" ? "context compacted" :
        ev.type === "prompt_suppressed" ? "prompt suppressed" :
        ev.type === "integrity" ? `integrity · ${ev.kind || ""}` :
        `error · ${ev.kind || ""}`;
      const bad = ev.type === "error" || (ev.type === "integrity" && /refus|restor|violat/i.test((ev.kind || "") + (ev.detail || "")));
      spans.push({ id, label, kind: "marker", status: bad ? "error" : "ok", agentId, parentId, start: rel(ts), end: rel(ts) + 160 });
    }
  }

  for (const [agentId, startTs] of agentOpen) {
    if (agentId === mainAgentId) continue;
    spans.push({
      id: `agent:${agentId}`,
      label: `sub-agent · ${agentId.slice(0, 8)}`,
      kind: "agent",
      agentId,
      start: rel(startTs),
      end: rel(nowFallback ?? Date.now()),
      detail: "running",
    });
  }
  return spans;
}

/** Flattens spans into rows: children under their parent (a sub-agent's spans), siblings by start time. */
export function layoutSpans(spans: TraceSpan[]): LaidSpan[] {
  const ids = new Set(spans.map((s) => s.id));
  const byParent = new Map<string, TraceSpan[]>();
  for (const s of spans) {
    const key = s.parentId && ids.has(s.parentId) ? s.parentId : "";
    if (!byParent.has(key)) byParent.set(key, []);
    byParent.get(key)!.push(s);
  }
  const out: LaidSpan[] = [];
  const seen = new Set<string>();
  const walk = (parent: string, depth: number) => {
    for (const s of (byParent.get(parent) || []).slice().sort((a, b) => a.start - b.start)) {
      if (seen.has(s.id)) continue;
      seen.add(s.id);
      out.push({ ...s, depth, dur: Math.max(1, s.end - s.start) });
      walk(s.id, depth + 1);
    }
  };
  walk("", 0);
  for (const s of spans) if (!seen.has(s.id)) out.push({ ...s, depth: 0, dur: Math.max(1, s.end - s.start) });
  return out;
}
