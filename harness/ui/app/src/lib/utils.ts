import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function fmtNum(n: number | null | undefined): string {
  if (n == null) return "–";
  if (n >= 1e6) return (n / 1e6).toFixed(2) + "M";
  if (n >= 1e4) return (n / 1e3).toFixed(1) + "k";
  return n.toLocaleString();
}

export function fmtMs(ms: number | null | undefined): string {
  if (ms == null) return "–";
  if (ms < 1000) return `${Math.round(ms)} ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)} s`;
  return `${Math.floor(ms / 60000)}m ${Math.round((ms % 60000) / 1000)}s`;
}

export function shortId(id: string | null | undefined): string {
  return String(id ?? "").slice(0, 8);
}

/** Deterministic per-agent colour, shared by trace bars, chips and the timeline's identity dots. */
const AGENT_PALETTE = ["#f0a5ff", "#5ee6d0", "#ffb454", "#7dd3fc", "#f472b6", "#a3e635", "#c4b5fd", "#fb923c"];
export function agentColor(id: string | null | undefined): string {
  let h = 0;
  const s = String(id ?? "");
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) >>> 0;
  return AGENT_PALETTE[h % AGENT_PALETTE.length];
}
