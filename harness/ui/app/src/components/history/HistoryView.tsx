import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { api, type BundleRow } from "@/lib/api";
import { fmtMs, fmtNum } from "@/lib/utils";
import { BundleDetail } from "./BundleDetail";

const OUTCOME_VARIANT = (o: string): "success" | "info" | "destructive" =>
  o === "completed" ? "success" : o === "running" ? "info" : "destructive";

export function HistoryView({ openId, onOpen }: { openId: string | null; onOpen: (id: string) => void }) {
  const [rows, setRows] = useState<BundleRow[] | null>(null);

  useEffect(() => {
    api<BundleRow[]>("/api/runs").then(setRows).catch(() => setRows([]));
  }, []);

  return (
    <section>
      <h1 className="mb-1 text-xl font-bold">History</h1>
      <p className="mb-5 text-muted">
        Every evidence bundle: <code className="rounded bg-panel-2 px-1 py-0.5 font-mono text-[12px]">evidence/</code> from your runs, and the ones cited in{" "}
        <code className="rounded bg-panel-2 px-1 py-0.5 font-mono text-[12px]">documentation/</code>.
      </p>

      <div className="overflow-x-auto rounded-lg border border-border">
        {rows && rows.length === 0 ? (
          <div className="p-9 text-center text-muted"><b className="mb-1 block text-foreground">No evidence bundles yet</b>Runs you start here, or with make run, appear here.</div>
        ) : (
          <table className="w-full text-[13px]">
            <thead>
              <tr className="bg-panel-2">
                {["Run", "Outcome", "Model", "Calls", "Input", "Output", "Wall", "Final tests"].map((h) => (
                  <th key={h} className="border-b border-border px-2.5 py-1.5 text-left text-xs font-semibold text-muted">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {(rows ?? []).map((r) => (
                <tr key={r.id} className={"cursor-pointer hover:bg-panel-2 " + (r.id === openId ? "bg-accent-soft" : "")} onClick={() => onOpen(r.id)}>
                  <td className="border-b border-border px-2.5 py-1.5"><code className="font-mono">{r.name}</code>{r.root === "documentation" && <Badge variant="info" className="ml-1.5">cited</Badge>}</td>
                  <td className="border-b border-border px-2.5 py-1.5"><Badge variant={OUTCOME_VARIANT(r.outcome)}>{r.outcome}</Badge></td>
                  <td className="border-b border-border px-2.5 py-1.5">{r.model || "–"}</td>
                  <td className="border-b border-border px-2.5 py-1.5 text-right font-mono tabular-nums">{r.calls ?? "–"}</td>
                  <td className="border-b border-border px-2.5 py-1.5 text-right font-mono tabular-nums">{fmtNum(r.inputTokens)}</td>
                  <td className="border-b border-border px-2.5 py-1.5 text-right font-mono tabular-nums">{fmtNum(r.outputTokens)}</td>
                  <td className="border-b border-border px-2.5 py-1.5 text-right font-mono tabular-nums">{fmtMs(r.wallMs)}</td>
                  <td className="border-b border-border px-2.5 py-1.5">{r.tests || "–"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {openId && <div className="mt-4.5"><BundleDetail id={openId} /></div>}
    </section>
  );
}
