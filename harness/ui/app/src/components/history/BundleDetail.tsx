import { useEffect, useState } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Badge } from "@/components/ui/badge";
import { api, type BundleDetail as BundleData } from "@/lib/api";
import { Markdown } from "./Markdown";
import { DiffView } from "./DiffView";
import { TraceReplay } from "@/components/trace/TraceReplay";

export function BundleDetail({ id }: { id: string }) {
  const [data, setData] = useState<BundleData | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    setData(null);
    setError("");
    const [root, name] = id.split("/");
    api<BundleData>(`/api/runs/${root}/${encodeURIComponent(name)}`).then(setData).catch((err) => setError(err.message));
  }, [id]);

  if (error) return <div className="rounded-md bg-destructive-soft px-2.5 py-2 text-[13px] text-destructive">{error}</div>;
  if (!data) return <div className="p-9 text-center text-muted">Loading…</div>;

  const name = id.split("/")[1];

  return (
    <div className="overflow-hidden rounded-lg border border-border">
      <div className="flex items-center justify-between gap-3 border-b border-border px-4 py-3">
        <code className="font-mono text-sm font-bold">{name}</code>
        <Badge variant={data.summary?.outcome === "completed" ? "success" : "destructive"}>{data.summary?.outcome}</Badge>
      </div>
      <Tabs defaultValue="report">
        <TabsList>
          <TabsTrigger value="report">Report</TabsTrigger>
          <TabsTrigger value="timeline">Timeline</TabsTrigger>
          <TabsTrigger value="diff">Changes</TabsTrigger>
          <TabsTrigger value="task">Task</TabsTrigger>
        </TabsList>
        <TabsContent value="report" className="p-4">
          {data.reportMd ? <Markdown text={data.reportMd} /> : <Empty title="No report yet">The run may still be going, or ended before sealing.</Empty>}
        </TabsContent>
        <TabsContent value="timeline" className="p-4">
          {data.telemetry.length ? (
            <TraceReplay telemetry={data.telemetry} meta={{ id: name, model: data.summary?.model ?? null, running: data.summary?.outcome === "running" }} />
          ) : (
            <Empty>No telemetry.</Empty>
          )}
        </TabsContent>
        <TabsContent value="diff" className="p-4">
          {data.diff && data.diff.trim() ? <DiffView text={data.diff} /> : <Empty>No changes recorded.</Empty>}
        </TabsContent>
        <TabsContent value="task" className="p-4">
          {data.prompt ? <pre className="whitespace-pre-wrap text-[13px]">{data.prompt}</pre> : <Empty>No prompt recorded.</Empty>}
        </TabsContent>
      </Tabs>
    </div>
  );
}

function Empty({ title, children }: { title?: string; children?: React.ReactNode }) {
  return (
    <div className="p-9 text-center text-muted">
      {title && <b className="mb-1 block text-foreground">{title}</b>}
      {children}
    </div>
  );
}
