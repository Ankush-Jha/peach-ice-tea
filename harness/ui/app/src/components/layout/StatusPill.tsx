import { cn } from "@/lib/utils";

export type Status = "idle" | "thinking" | "running" | "done" | "failed";

const DOT: Record<Status, string> = {
  idle: "bg-[#444]",
  thinking: "bg-accent animate-[breathe_1.6s_ease-in-out_infinite]",
  running: "bg-live animate-[breathe_1s_ease-in-out_infinite]",
  done: "bg-success",
  failed: "bg-destructive",
};

export function StatusPill({ status }: { status: Status }) {
  return (
    <span className="inline-flex items-center gap-1.5 text-xs font-bold capitalize">
      <span className={cn("h-[7px] w-[7px] shrink-0 rounded-full", DOT[status])} />
      {status}
    </span>
  );
}
