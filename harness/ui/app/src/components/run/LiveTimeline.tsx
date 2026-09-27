import { useMemo } from "react";
import { Card, CardHeader, CardTitle } from "@/components/ui/card";
import { describe } from "@/lib/describe";
import { agentColor } from "@/lib/utils";
import type { Envelope } from "@/lib/api";

const BAR_COLOR: Record<string, string> = { model: "var(--color-accent)", tool: "var(--color-live)", io: "var(--color-success)" };

export function LiveTimeline({ events }: { events: Envelope[] }) {
  // Re-derived from the full event list on every change (it only ever grows) rather than kept as
  // running state — simpler than reconciling "seen"/rolling-max across renders, and just as cheap
  // at the event volumes a single run produces.
  const rows = useMemo(() => {
    const seen = new Set<string>();
    let maxDur = 400;
    return events
      .map((env, i) => {
        const d = describe(env, seen);
        if (!d) return null;
        let barPct: number | null = null;
        let barColor = "var(--color-muted)";
        if (d.dur != null && d.kind) {
          maxDur = Math.max(maxDur, d.dur);
          barPct = Math.max(4, Math.min(100, (d.dur / maxDur) * 100));
          barColor = d.status === "error" ? "var(--color-destructive)" : BAR_COLOR[d.kind] ?? barColor;
        }
        return { key: i, d, barPct, barColor };
      })
      .filter((r): r is { key: number; d: NonNullable<ReturnType<typeof describe>>; barPct: number | null; barColor: string } => r !== null);
  }, [events]);

  return (
    <Card>
      <CardHeader>
        <CardTitle>Timeline</CardTitle>
        <span className="text-xs font-normal text-muted">from the run's own telemetry</span>
      </CardHeader>
      <div className="max-h-[480px] overflow-y-auto px-3 py-1.5">
        {rows.length === 0 && (
          <div className="px-4 py-9 text-center text-muted">
            <b className="mb-1 block text-foreground">No run yet</b>
            Load a sample or point at a repository, describe the task, and start.
          </div>
        )}
        {rows.map(({ key, d, barPct, barColor }) => (
          <div
            key={key}
            className="border-b border-dashed border-border py-1.5 pl-2.5 last:border-b-0"
            style={{ borderLeft: `3px solid ${d.agentId ? agentColor(d.agentId) : "transparent"}` }}
          >
            <div className={"grid items-center gap-2 text-[13px]" + (d.sub ? " pl-4" : "")} style={{ gridTemplateColumns: "64px 22px minmax(0,1fr) auto" }}>
              <span className="font-mono text-[11.5px] text-muted">{d.time}</span>
              <span className="text-center">{d.icon}</span>
              <span className="min-w-0 truncate">{d.node}</span>
              <span className="whitespace-nowrap font-mono text-[11.5px] text-muted">{d.num}</span>
            </div>
            {barPct != null && (
              <div className="ml-[86px] mt-1 h-[3px] overflow-hidden rounded-full bg-white/5">
                <div
                  className="h-full origin-left rounded-full"
                  style={{ width: `${barPct}%`, background: barColor, animation: "bar-in .55s cubic-bezier(.16,1,.3,1) both" }}
                />
              </div>
            )}
          </div>
        ))}
      </div>
    </Card>
  );
}
