import type { SpanKind } from "@/lib/trace";

/** Small original geometric glyphs — not a copy of any icon set's artwork. */
export function TraceIcon({ kind }: { kind: SpanKind }) {
  const paths: Record<SpanKind, React.ReactNode> = {
    agent: <><circle cx="8" cy="8" r="5" fill="none" stroke="currentColor" strokeWidth="1.4" /><circle cx="8" cy="8" r="1.3" fill="currentColor" /><line x1="8" y1="1.6" x2="8" y2="3.1" stroke="currentColor" strokeWidth="1.4" /></>,
    model: <rect x="4.3" y="4.3" width="7.4" height="7.4" rx="1.6" fill="none" stroke="currentColor" strokeWidth="1.4" transform="rotate(45 8 8)" />,
    tool: <><path d="M8 2 L13 5 V11 L8 14 L3 11 V5 Z" fill="none" stroke="currentColor" strokeWidth="1.3" /><circle cx="8" cy="8" r="2.1" fill="none" stroke="currentColor" strokeWidth="1.3" /></>,
    io: <><line x1="3.2" y1="4.5" x2="12.8" y2="4.5" stroke="currentColor" strokeWidth="1.3" /><line x1="3.2" y1="8" x2="12.8" y2="8" stroke="currentColor" strokeWidth="1.3" /><line x1="3.2" y1="11.5" x2="9" y2="11.5" stroke="currentColor" strokeWidth="1.3" /></>,
    marker: <><path d="M8 2.2 L13.8 12.8 H2.2 Z" fill="none" stroke="currentColor" strokeWidth="1.3" /><line x1="8" y1="6.2" x2="8" y2="9.4" stroke="currentColor" strokeWidth="1.3" /><circle cx="8" cy="11" r=".8" fill="currentColor" /></>,
  };
  return (
    <svg width="13" height="13" viewBox="0 0 16 16" aria-hidden="true">
      {paths[kind]}
    </svg>
  );
}
