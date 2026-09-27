import { Card } from "@/components/ui/card";
import { fmtMs, fmtNum } from "@/lib/utils";

export interface Stats {
  calls: number;
  tools: number;
  input: number;
  output: number;
  think: number;
  tests: { ok: boolean; label: string } | null;
}

export function StatTiles({ active, elapsedMs, stats, agentCount }: { active: boolean; elapsedMs: number; stats: Stats; agentCount: number }) {
  const tiles: { label: string; value: React.ReactNode }[] = [
    { label: "Elapsed", value: active ? fmtMs(elapsedMs) : "–" },
    { label: "Model calls", value: active ? stats.calls : "–" },
    { label: "Tool calls", value: active ? stats.tools : "–" },
    { label: "Input tokens", value: active ? fmtNum(stats.input) : "–" },
    { label: "Output tokens", value: active ? fmtNum(stats.output) : "–" },
    { label: "Thinking tokens", value: active ? fmtNum(stats.think) : "–" },
    { label: "Agents", value: active ? agentCount : "–" },
    {
      label: "Last test run",
      value: stats.tests ? <span className={stats.tests.ok ? "text-success" : "text-destructive"}>{stats.tests.label}</span> : "–",
    },
  ];
  return (
    <div className="mb-3.5 grid grid-cols-2 gap-2.5 sm:grid-cols-4">
      {tiles.map((t) => (
        <Card key={t.label} className="min-w-0 px-3 py-2.5">
          <div className="truncate text-[18px] font-bold tabular-nums">{t.value}</div>
          <div className="text-xs text-muted">{t.label}</div>
        </Card>
      ))}
    </div>
  );
}
