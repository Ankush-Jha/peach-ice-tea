import { useEffect, useMemo, useState } from "react";
import { RunForm } from "./RunForm";
import { StatTiles, type Stats } from "./StatTiles";
import { AgentChips, type AgentInfo } from "./AgentChips";
import { LiveTimeline } from "./LiveTimeline";
import { Button } from "@/components/ui/button";
import { api, type Status } from "@/lib/api";
import { useRunStream } from "@/hooks/useRunStream";
import type { Status as StatusName } from "@/components/layout/StatusPill";

const EXIT: Record<number, string> = { 0: "completed", 1: "error", 2: "tool-failure limit", 3: "request limit", 4: "time budget", 5: "interrupted", 6: "doom-loop escalation" };

export function RunPage({ status, onStatus, onOpenBundle }: { status: Status | null; onStatus: (s: StatusName) => void; onOpenBundle: (id: string) => void }) {
  const { run, events, log } = useRunStream();
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    if (!run?.running) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [run?.running]);

  // Coarse status: idle when nothing has run, "thinking" once ~1.3s pass with no fresh telemetry
  // while a run is active, "running" right after an event lands, "done"/"failed" once it ends.
  useEffect(() => {
    if (!run) { onStatus("idle"); return; }
    if (!run.running) { onStatus(run.exitCode === 0 ? "done" : "failed"); return; }
    onStatus("running");
    const t = setTimeout(() => onStatus("thinking"), 1300);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [run, events.length]);

  const { stats, agents } = useMemo(() => {
    const s: Stats = { calls: 0, tools: 0, input: 0, output: 0, think: 0, tests: null };
    const agentMap = new Map<string, AgentInfo>();
    const touch = (id: string) => {
      if (!agentMap.has(id)) agentMap.set(id, { id, order: agentMap.size, calls: 0, tools: 0, state: "running" });
      return agentMap.get(id)!;
    };
    for (const env of events) {
      const ev = env.event || {};
      const agentId = env.agent_id;
      if (ev.type === "model_call") {
        s.calls++; s.input += ev.input_tokens || 0; s.output += ev.output_tokens || 0; s.think += ev.reasoning_tokens || 0;
        if (agentId) touch(agentId).calls++;
      }
      if (ev.type === "tool_call") {
        s.tools++;
        if (agentId) touch(agentId).tools++;
      }
      if (ev.type === "test_run") {
        s.tests = { ok: ev.exit_code === 0, label: ev.passed != null ? `${ev.passed}/${(ev.passed || 0) + (ev.failed || 0)}` : ev.failure_class || "ran" };
      }
      if (ev.type === "agent_state" && agentId) touch(agentId).state = ev.to;
    }
    return { stats: s, agents: [...agentMap.values()] };
  }, [events]);

  const elapsedMs = run ? (run.running ? now : run.startedAt + (events.length ? Date.parse(events[events.length - 1]?.timestamp || "") - run.startedAt : 0)) - run.startedAt : 0;

  async function stop() {
    try { await api("/api/run/stop"); } catch { /* surfaced via the SSE end event either way */ }
  }

  const finished = run && !run.running;
  const outcome = finished ? (EXIT[run.exitCode ?? -1] ?? `exit ${run.exitCode}`) : null;

  return (
    <section>
      <h1 className="mb-1 text-xl font-bold">Run a task</h1>
      <p className="mb-5 text-muted">
        One unattended run, the same path as <code className="rounded bg-panel-2 px-1 py-0.5 font-mono text-[12px]">make run</code>: the agent edits the repository, the harness runs its tests and records an evidence bundle.
      </p>
      <div className="grid items-start gap-4.5 lg:grid-cols-[minmax(300px,380px)_1fr]">
        <RunForm status={status} running={!!run?.running} onStarted={() => {}} onStop={stop} />
        <div>
          <div className="mb-3.5 flex flex-wrap items-center gap-2.5">
            <span className={
              "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-semibold " +
              (!run ? "border-border bg-panel-2 text-muted" :
                run.running ? "border-transparent bg-info-soft text-info" :
                run.exitCode === 0 ? "border-transparent bg-success-soft text-success" :
                [3, 4].includes(run.exitCode ?? -1) ? "border-transparent bg-warning-soft text-warning" :
                "border-transparent bg-destructive-soft text-destructive")
            }>
              {!run ? "idle" : run.running ? "running" : outcome}
            </span>
            {run && <span className="truncate font-mono text-[13px] text-muted">{run.profile} · {run.model || "profile default"} · {run.repo}</span>}
          </div>

          {finished && (
            <div className={
              "mb-3.5 flex items-center justify-between gap-3 rounded-lg px-4 py-3 " +
              (run!.exitCode === 0 ? "bg-success-soft text-success" : [3, 4].includes(run!.exitCode ?? -1) ? "bg-warning-soft text-warning" : "bg-destructive-soft text-destructive")
            }>
              <span>
                Run {outcome}{stats.tests ? ` · last test run ${stats.tests.label} ${stats.tests.ok ? "passing" : "failing"}` : ""}. The evidence bundle has the full report, diff and transcript.
              </span>
              <Button variant="default" onClick={() => onOpenBundle(run!.evidenceId)}>Open report</Button>
            </div>
          )}

          <StatTiles active={!!run} elapsedMs={elapsedMs} stats={stats} agentCount={agents.length} />
          <AgentChips agents={agents} />
          <LiveTimeline events={events} />

          <details className="mt-3.5">
            <summary className="cursor-pointer text-[13px] text-muted">Harness output</summary>
            <pre className="mt-2 max-h-[420px] overflow-auto rounded-md border border-border bg-panel-2 p-2.5 font-mono text-[12px]">{log.map((l) => l.text).join("\n")}</pre>
          </details>
        </div>
      </div>
    </section>
  );
}
