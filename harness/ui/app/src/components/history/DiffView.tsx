export function DiffView({ text }: { text: string }) {
  return (
    <pre className="overflow-auto rounded-md border border-border bg-panel-2 p-3 font-mono text-[12px] leading-relaxed">
      {text.split("\n").map((line, i) => {
        const cls = line.startsWith("diff --git") || line.startsWith("+++") || line.startsWith("---")
          ? "text-foreground font-bold"
          : line.startsWith("@@") ? "text-info"
          : line.startsWith("+") ? "text-success"
          : line.startsWith("-") ? "text-destructive"
          : "text-muted";
        return <div key={i} className={cls}>{line || " "}</div>;
      })}
    </pre>
  );
}
