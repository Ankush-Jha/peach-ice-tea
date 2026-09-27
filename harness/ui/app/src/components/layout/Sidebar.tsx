import { BarChart3, History, PlayCircle } from "lucide-react";
import { cn } from "@/lib/utils";
import { StatusPill, type Status } from "./StatusPill";
import type { View } from "@/App";

const NAV: { view: View; label: string; icon: typeof PlayCircle }[] = [
  { view: "run", label: "Run a task", icon: PlayCircle },
  { view: "history", label: "History", icon: History },
  { view: "bench", label: "Benchmarks", icon: BarChart3 },
];

export function Sidebar({
  view,
  onView,
  live,
  binary,
  keyCount,
  status,
}: {
  view: View;
  onView: (v: View) => void;
  live: boolean;
  binary: string | null;
  keyCount: number;
  status: Status;
}) {
  return (
    <aside className="sticky top-0 flex h-screen w-[220px] flex-none flex-col gap-5 border-r border-border bg-panel px-3.5 py-5">
      <div className="flex items-center gap-2.5 px-1.5">
        <svg width="26" height="26" viewBox="0 0 32 32" aria-hidden="true">
          <rect width="32" height="32" rx="9" fill="#8b5cf6" />
          <path d="M13 9 L13 23 L24 16 Z" fill="#fff" />
        </svg>
        <div>
          <div className="text-[15px] font-semibold leading-tight">Peach Ice Tea</div>
          <div className="text-xs text-muted">coding-agent harness</div>
        </div>
      </div>

      <nav className="flex flex-col gap-0.5">
        {NAV.map(({ view: v, label, icon: Icon }) => (
          <button
            key={v}
            onClick={() => onView(v)}
            className={cn(
              "flex items-center justify-between gap-2 rounded-md px-2.5 py-2 text-left text-sm text-muted transition-colors hover:bg-panel-2 hover:text-foreground",
              view === v && "bg-accent-soft font-semibold text-foreground"
            )}
          >
            <span className="flex items-center gap-2">
              <Icon size={15} strokeWidth={2} />
              {label}
            </span>
            {v === "run" && live && (
              <span className="rounded-full bg-live-soft px-1.5 py-0.5 text-[10px] font-bold text-live">live</span>
            )}
          </button>
        ))}
      </nav>

      <div className="mt-auto flex flex-col gap-1.5 px-1.5 text-xs text-muted">
        <div className="flex items-center justify-between gap-2">
          <span>Binary</span>
          <span className={cn("truncate font-mono", !binary && "text-destructive")}>{binary ? binary.replace(/^target\//, "") : "not built"}</span>
        </div>
        <div className="flex items-center justify-between gap-2">
          <span>Keys</span>
          <span className={cn(!keyCount && "text-destructive")}>{keyCount ? `${keyCount} profile(s)` : "none"}</span>
        </div>
        <div className="flex items-center justify-between gap-2">
          <span>Status</span>
          <StatusPill status={status} />
        </div>
      </div>
    </aside>
  );
}
