import type { ReactNode } from "react";
import type { Envelope } from "./api";
import { fmtMs, fmtNum } from "./utils";
import { Badge } from "@/components/ui/badge";

export interface Described {
  time: string;
  icon: string;
  sub?: boolean;
  node: ReactNode;
  num?: string;
  kind?: "model" | "tool" | "io";
  status?: "ok" | "error";
  dur?: number;
  agentId: string | null;
}

const ORIGIN: Record<string, string> = { agent: "agent", harness_final: "harness · final", harness_runtime_gate: "harness · gate" };

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

/**
 * One telemetry envelope → one renderable row. `seen` is a Set threaded through a single render pass
 * (the live feed, or one bundle's full replay) — it's how a first-time agent_id is told apart from a
 * returning one, so "Agent launched" fires exactly once per agent, in either view.
 */
export function describe(env: Envelope, seen?: Set<string>): Described | null {
  const ev = env.event || {};
  const agentId = env.agent_id || null;
  const isNewAgent = !!(agentId && seen && !seen.has(agentId));
  if (agentId && seen) seen.add(agentId);
  const time = env.timestamp ? new Date(env.timestamp).toLocaleTimeString([], { hour12: false }) : "";

  switch (ev.type) {
    case "run_start":
      return { time, icon: "▶", node: <>Run started in <code className="rounded bg-panel-2 px-1 py-0.5 font-mono text-[12px]">{ev.repo_root}</code></>, agentId };
    case "model_call": {
      const n = String(ev.call_id || "").split("#").pop();
      const cached = ev.cached_tokens ? ` · ${fmtNum(ev.cached_tokens)} cached` : "";
      const thinking = ev.reasoning_tokens ? ` · ${fmtNum(ev.reasoning_tokens)} thinking` : "";
      return {
        time, icon: "◆", agentId, kind: "model", dur: ev.duration_ms,
        node: <>Model call <b>#{n}</b> <Badge variant="neutral">{ev.finish_reason || ""}</Badge></>,
        num: `${fmtNum(ev.input_tokens)} in · ${fmtNum(ev.output_tokens)} out${cached}${thinking} · ${fmtMs(ev.duration_ms)}`,
      };
    }
    case "tool_call":
      return {
        time, icon: ev.success ? "✓" : "✗", sub: true, agentId, kind: "tool", status: ev.success === false ? "error" : "ok", dur: ev.duration_ms,
        node: <><code className="rounded bg-panel-2 px-1 py-0.5 font-mono text-[12px]">{ev.name}</code> {toolArgs(ev)}{ev.success === false && <Badge variant="destructive">failed</Badge>}</>,
        num: fmtMs(ev.duration_ms),
      };
    case "test_run": {
      const ok = ev.exit_code === 0;
      const counts = ev.passed != null ? ` ${ev.passed} passed / ${ev.failed ?? 0} failed` : "";
      return {
        time, icon: "⚑", sub: ev.origin === "agent", agentId, kind: "io", status: ok ? "ok" : "error", dur: ev.duration_ms,
        node: <>Tests <Badge variant={ok ? "success" : "destructive"}>{ev.failure_class || (ok ? "passed" : "failed")}</Badge> <Badge variant="info">{ORIGIN[ev.origin] || ev.origin}</Badge>{counts}</>,
        num: fmtMs(ev.duration_ms),
      };
    }
    case "agent_state": {
      let node: ReactNode;
      if (ev.to === "running") node = isNewAgent ? <b>Agent launched</b> : "Agent state → running";
      else if (ev.to === "ended") node = <>Agent finished{ev.iteration != null && ` · ${fmtNum(ev.iteration)} call(s)`}</>;
      else node = <>Agent → <Badge variant={/exhaust|unconfirm|fail/i.test(ev.to || "") ? "warning" : "info"}>{ev.to}</Badge></>;
      return { time, icon: ev.to === "running" ? "✦" : ev.to === "ended" ? "○" : "◈", agentId, node: <>{node}{ev.reason && ` — ${ev.reason}`}</> };
    }
    case "recovery":
      return { time, icon: "↺", sub: true, agentId, node: <>Recovery <code className="rounded bg-panel-2 px-1 py-0.5 font-mono text-[12px]">{ev.action}</code> {String(ev.trigger || "").slice(0, 120)}{ev.attribution && <Badge variant="warning">{ev.attribution}</Badge>}</> };
    case "context_compaction":
      return { time, icon: "⇲", agentId, node: <>Context compacted: {ev.messages_before} → {ev.messages_after} messages</>, num: ev.tokens_before_estimated ? `${fmtNum(ev.tokens_before_estimated)} → ${fmtNum(ev.tokens_after_estimated)} tok` : undefined };
    case "retry":
      return { time, icon: "⟳", sub: true, agentId, node: <>Retry {ev.attempt}{ev.max_attempts ? `/${ev.max_attempts}` : ""} <Badge variant="warning">{String(ev.reason || "").slice(0, 60)}</Badge></> };
    case "integrity":
      return { time, icon: "⛨", agentId, node: <>Integrity <Badge variant={/refus|restor|violat/i.test((ev.kind || "") + (ev.detail || "")) ? "warning" : "success"}>{ev.kind}</Badge> {ev.detail}</> };
    case "error":
      return { time, icon: "!", agentId, node: <><Badge variant="destructive">{ev.kind}</Badge> {String(ev.message || "").slice(0, 200)}</> };
    case "prompt_suppressed":
      return { time, icon: "⤫", sub: true, agentId, node: <>Prompt answered unattended: {ev.prompt_kind}</> };
    case "run_end":
      return { time, icon: "■", agentId, node: <>Run ended <Badge variant={ev.outcome === "completed" ? "success" : "destructive"}>{ev.outcome}</Badge></>, num: fmtMs(ev.duration_ms) };
    default:
      return null;
  }
}
