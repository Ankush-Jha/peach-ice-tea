import { useEffect, useState } from "react";
import { api, type ReportRow } from "@/lib/api";
import { Markdown } from "@/components/history/Markdown";
import { cn } from "@/lib/utils";

const KIND: Record<string, string> = { ab: "A/B", models: "Bake-off", tools: "Tool errors" };

export function BenchmarksView() {
  const [list, setList] = useState<ReportRow[] | null>(null);
  const [selected, setSelected] = useState<ReportRow | null>(null);
  const [markdown, setMarkdown] = useState<string | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    api<ReportRow[]>("/api/reports").then(setList).catch(() => setList([]));
  }, []);

  async function open(r: ReportRow) {
    setSelected(r);
    setError("");
    try {
      const res = await api<{ markdown: string }>(`/api/reports/${r.kind}/${encodeURIComponent(r.name)}`);
      setMarkdown(res.markdown);
    } catch (err: any) {
      setError(err.message);
    }
  }

  return (
    <section>
      <h1 className="mb-1 text-xl font-bold">Benchmarks</h1>
      <p className="mb-5 text-muted">
        A/B reports behind every shipped default, model bake-offs and tool error rates, from{" "}
        <code className="rounded bg-panel-2 px-1 py-0.5 font-mono text-[12px]">benchmarks/reports/</code>.
      </p>
      <div className="grid items-start gap-4.5 lg:grid-cols-[minmax(260px,340px)_1fr]">
        <div className="max-h-[calc(100vh-150px)] overflow-auto rounded-lg border border-border">
          {list && list.length === 0 && <div className="p-9 text-center text-muted">No reports yet.</div>}
          {list?.map((r) => (
            <button
              key={`${r.kind}/${r.name}`}
              onClick={() => open(r)}
              className={cn(
                "block w-full border-b border-border px-3.5 py-2.5 text-left last:border-b-0 hover:bg-panel-2",
                selected?.name === r.name && selected.kind === r.kind && "bg-accent-soft"
              )}
            >
              {r.name.replace(/\.md$/, "")}
              <small className="mt-0.5 block text-xs text-muted">{KIND[r.kind] || r.kind} · {new Date(r.mtimeMs).toLocaleString()}</small>
            </button>
          ))}
        </div>
        <div className="rounded-lg border border-border p-4">
          {error && <div className="rounded-md bg-destructive-soft px-2.5 py-2 text-[13px] text-destructive">{error}</div>}
          {!error && !markdown && <div className="p-9 text-center text-muted"><b className="mb-1 block text-foreground">Pick a report</b>Newest first.</div>}
          {!error && markdown && <Markdown text={markdown} />}
        </div>
      </div>
    </section>
  );
}
