import { useEffect, useState } from "react";
import { Sidebar } from "@/components/layout/Sidebar";
import type { Status as StatusName } from "@/components/layout/StatusPill";
import { RunPage } from "@/components/run/RunPage";
import { HistoryView } from "@/components/history/HistoryView";
import { BenchmarksView } from "@/components/benchmarks/BenchmarksView";
import { api, type Status } from "@/lib/api";

export type View = "run" | "history" | "bench";

export default function App() {
  const [view, setView] = useState<View>("run");
  const [status, setStatus] = useState<Status | null>(null);
  const [statusError, setStatusError] = useState("");
  const [runStatus, setRunStatus] = useState<StatusName>("idle");
  const [openBundleId, setOpenBundleId] = useState<string | null>(null);

  useEffect(() => {
    api<Status>("/api/status").then(setStatus).catch((err) => setStatusError(err.message));
  }, []);

  const live = runStatus === "running" || runStatus === "thinking";
  const keyCount = status?.profiles.filter((p) => p.keyAvailable).length ?? 0;

  function openBundle(id: string) {
    setOpenBundleId(id);
    setView("history");
  }

  return (
    <div className="flex min-h-screen bg-background">
      <Sidebar view={view} onView={setView} live={live} binary={status?.binary ?? null} keyCount={keyCount} status={runStatus} />
      <main className="min-w-0 flex-1 px-7 py-6 pb-12">
        {statusError && (
          <div className="mb-4 rounded-md bg-destructive-soft px-3 py-2 text-[13px] text-destructive">
            Cannot reach the UI server: {statusError}
          </div>
        )}
        {status && !status.binary && (
          <div className="mb-4 rounded-md bg-destructive-soft px-3 py-2 text-[13px] text-destructive">
            No harness binary yet: run <code className="rounded bg-panel-2 px-1 py-0.5 font-mono">make setup</code>, then reload.
          </div>
        )}
        {view === "run" && <RunPage status={status} onStatus={setRunStatus} onOpenBundle={openBundle} />}
        {view === "history" && <HistoryView openId={openBundleId} onOpen={setOpenBundleId} />}
        {view === "bench" && <BenchmarksView />}
      </main>
    </div>
  );
}
