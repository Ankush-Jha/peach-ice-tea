import { agentColor, cn } from "@/lib/utils";

export interface AgentInfo {
  id: string;
  order: number;
  calls: number;
  tools: number;
  state: string;
}

export function AgentChips({ agents }: { agents: AgentInfo[] }) {
  if (!agents.length) return null;
  return (
    <div className="mb-3 flex flex-wrap gap-2">
      {agents.map((a) => {
        const live = a.state !== "ended";
        const color = agentColor(a.id);
        return (
          <span
            key={a.id}
            title={a.id}
            className={cn(
              "inline-flex items-center gap-1.5 rounded-full border border-border bg-panel-solid py-1 pl-2 pr-2.5 text-xs font-semibold text-muted",
              live && "text-foreground",
              !live && "opacity-50"
            )}
          >
            <span
              className={cn("h-2 w-2 rounded-full", live && "animate-[breathe_1.4s_ease-in-out_infinite]")}
              style={{ background: color }}
            />
            {a.order === 0 ? "main" : `sub·${a.order}`}
            <span className="font-mono text-[11px] font-medium text-muted">{a.calls}c/{a.tools}t</span>
          </span>
        );
      })}
    </div>
  );
}
