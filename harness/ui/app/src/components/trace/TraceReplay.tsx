import { useEffect, useMemo, useRef, useState } from "react";
import { Pause, Play } from "lucide-react";
import { buildSpans, layoutSpans, type LaidSpan } from "@/lib/trace";
import { agentColor, cn, fmtMs } from "@/lib/utils";
import { TraceIcon } from "./icons";
import type { Envelope } from "@/lib/api";

const NICE_TICKS = [50, 100, 250, 500, 1000, 2000, 5000, 10000, 30000, 60000, 120000];

interface RowHandle {
  fill: HTMLDivElement | null;
  d: HTMLSpanElement | null;
  ms: HTMLSpanElement | null;
  el: HTMLDivElement | null;
}

/**
 * A scrubbable waterfall replay of one finished run — a real time axis, a draggable playhead, and
 * play/pause, matching a browser's network waterfall more than a log viewer. Positions are written
 * straight to the DOM every frame via refs (not React state) so dragging and playback stay smooth
 * regardless of how many spans a run produced.
 */
export function TraceReplay({ telemetry, meta }: { telemetry: Envelope[]; meta: { id: string; model: string | null; running: boolean } }) {
  const rows = useMemo(() => layoutSpans(buildSpans(telemetry)), [telemetry]);
  const total = useMemo(() => Math.max(1000, ...rows.map((r) => r.start + r.dur)), [rows]);
  const maxDepth = useMemo(() => Math.max(0, ...rows.map((r) => r.depth)), [rows]);
  const gutter = Math.min(220, Math.max(120, 56 + maxDepth * 14));
  const ticks = useMemo(() => {
    const step = NICE_TICKS.find((n) => total / n <= 8) || total / 4;
    const out: number[] = [];
    for (let x = step; x < total; x += step) out.push(x);
    return out;
  }, [total]);

  const rowRefs = useRef<RowHandle[]>([]);
  const playheadRef = useRef<HTMLDivElement>(null);
  const railFillRef = useRef<HTMLDivElement>(null);
  const railThumbRef = useRef<HTMLDivElement>(null);
  const clockRef = useRef<HTMLSpanElement>(null);
  const railRef = useRef<HTMLDivElement>(null);
  const scrubRef = useRef<HTMLDivElement>(null);

  const tRef = useRef(total);
  const lastRef = useRef(0);
  const rafRef = useRef(0);
  const [playing, setPlaying] = useState(false);

  const paint = () => {
    const t = tRef.current;
    const pct = t / total;
    if (playheadRef.current) playheadRef.current.style.transform = `translateX(${(pct * 100).toFixed(3)}%)`;
    if (railFillRef.current) railFillRef.current.style.transform = `scaleX(${pct.toFixed(4)})`;
    if (railThumbRef.current) railThumbRef.current.style.left = `${(pct * 100).toFixed(3)}%`;
    if (clockRef.current) clockRef.current.textContent = `${(t / 1000).toFixed(2)}s / ${(total / 1000).toFixed(2)}s`;
    railRef.current?.setAttribute("aria-valuenow", String(Math.round(t)));
    rowRefs.current.forEach((h, i) => {
      const row = rows[i];
      if (!row || !h) return;
      const p = row.dur > 0 ? Math.min(1, Math.max(0, (t - row.start) / row.dur)) : t >= row.start ? 1 : 0;
      if (h.fill) h.fill.style.transform = `translateY(-50%) scaleX(${p.toFixed(4)})`;
      const state = t < row.start ? "queued" : p < 1 ? "running" : row.status === "error" ? "error" : "done";
      if (h.el && h.el.dataset.state !== state) h.el.dataset.state = state;
      if (h.ms) h.ms.textContent = state === "queued" ? "" : fmtMs(row.dur * p);
      if (h.d) h.d.textContent = state === "queued" || p < 1 ? "" : row.detail || "";
    });
  };

  useEffect(() => {
    tRef.current = total;
    rowRefs.current = rows.map(() => ({ fill: null, d: null, ms: null, el: null }));
    paint();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rows, total]);

  const frame = (now: number) => {
    const dt = Math.min(now - (lastRef.current || now), 80);
    lastRef.current = now;
    tRef.current = Math.min(total, tRef.current + dt);
    paint();
    if (tRef.current >= total) {
      setPlaying(false);
      rafRef.current = 0;
      return;
    }
    rafRef.current = requestAnimationFrame(frame);
  };

  const seek = (ms: number) => {
    tRef.current = Math.min(total, Math.max(0, ms));
    lastRef.current = 0;
    paint();
  };

  const togglePlay = () => {
    setPlaying((was) => {
      const next = !was;
      if (next) {
        if (tRef.current >= total) tRef.current = 0;
        lastRef.current = 0;
        rafRef.current = requestAnimationFrame(frame);
      } else {
        cancelAnimationFrame(rafRef.current);
      }
      return next;
    });
  };

  useEffect(() => () => cancelAnimationFrame(rafRef.current), []);

  const scrubFrom = (clientX: number, el: HTMLElement) => {
    const r = el.getBoundingClientRect();
    if (r.width > 0) seek(((clientX - r.left) / r.width) * total);
  };

  if (!rows.length) return <div className="p-8 text-center text-muted">No spans to trace.</div>;

  return (
    <div className="overflow-hidden rounded-lg border border-border bg-panel-solid" style={{ ["--gutter" as any]: `${gutter}px` }}>
      <div className="flex items-center gap-2.5 border-b border-border px-3.5 py-2.5">
        <span className={cn("h-[7px] w-[7px] shrink-0 rounded-full bg-muted", meta.running && "animate-[breathe_1s_ease-in-out_infinite] bg-live")} />
        <span className="font-mono text-[13px] font-bold">{meta.id}</span>
        <span className="text-xs text-muted">{meta.model ? `${meta.model} · ` : ""}{rows.length} spans</span>
        <span className="ml-auto rounded-full border border-border px-2.5 py-0.5 text-[11px] text-muted">{meta.running ? "Running" : "Completed"}</span>
      </div>

      <div className="relative px-3.5 pb-2.5 pt-1.5">
        <div className="relative mb-0.5 h-5">
          {ticks.map((x) => (
            <span key={x} className="absolute top-1 -translate-x-1/2 whitespace-nowrap font-mono text-[10px] text-muted">
              {x < 1000 ? `${Math.round(x)}ms` : `${x / 1000}s`}
            </span>
          ))}
        </div>
        <div className="pointer-events-none absolute inset-x-3.5 bottom-2.5 top-5" style={{ left: "calc(var(--gutter) + 14px)" }}>
          {ticks.map((x) => <i key={x} className="absolute inset-y-0 w-px bg-border" style={{ left: `${(x / total) * 100}%` }} />)}
        </div>

        <div className="relative">
          {rows.map((row, i) => (
            <TraceRow key={row.id} row={row} total={total} gutter={gutter} registerRef={(h) => (rowRefs.current[i] = h)} />
          ))}
        </div>

        <div className="pointer-events-none absolute bottom-2.5 top-5" style={{ left: "calc(var(--gutter) + 14px)", right: "14px" }}>
          <div ref={playheadRef} className="relative h-full w-full" style={{ transform: "translateX(0%)" }}>
            <i className="absolute inset-y-0 w-px bg-live" />
          </div>
        </div>
        <div
          ref={scrubRef}
          className="absolute bottom-2.5 top-5 cursor-ew-resize"
          style={{ left: "calc(var(--gutter) + 14px)", right: "14px" }}
          onPointerDown={(e) => { e.currentTarget.setPointerCapture(e.pointerId); scrubFrom(e.clientX, e.currentTarget); }}
          onPointerMove={(e) => { if (e.currentTarget.hasPointerCapture(e.pointerId)) scrubFrom(e.clientX, e.currentTarget); }}
        />
      </div>

      <div className="flex items-center gap-3 border-t border-border px-3.5 py-2.5">
        <button
          type="button"
          onClick={togglePlay}
          aria-label={playing ? "Pause" : "Play"}
          className="flex h-[30px] w-[30px] flex-none items-center justify-center rounded-full border border-border bg-transparent text-foreground transition-[background-color,border-color,transform] duration-150 hover:border-foreground/40 hover:bg-panel-2 active:scale-[0.94]"
        >
          {playing ? <Pause size={13} fill="currentColor" /> : <Play size={13} fill="currentColor" className="translate-x-px" />}
        </button>
        <div
          ref={railRef}
          role="slider"
          tabIndex={0}
          aria-label="Playhead"
          aria-valuemin={0}
          aria-valuemax={Math.round(total)}
          className="relative h-[26px] flex-1 cursor-ew-resize touch-none"
          onPointerDown={(e) => { e.currentTarget.setPointerCapture(e.pointerId); scrubFrom(e.clientX, e.currentTarget); }}
          onPointerMove={(e) => { if (e.currentTarget.hasPointerCapture(e.pointerId)) scrubFrom(e.clientX, e.currentTarget); }}
          onKeyDown={(e) => {
            const step = total / 50;
            if (e.key === "ArrowRight" || e.key === "ArrowUp") seek(tRef.current + step);
            else if (e.key === "ArrowLeft" || e.key === "ArrowDown") seek(tRef.current - step);
            else if (e.key === "Home") seek(0);
            else if (e.key === "End") seek(total);
            else return;
            e.preventDefault();
          }}
        >
          <div className="absolute left-1.5 right-1.5 top-1/2 h-1 -translate-y-1/2 overflow-hidden rounded-full bg-panel-2">
            <div ref={railFillRef} className="h-full w-full origin-left bg-live" style={{ transform: "scaleX(0)" }} />
          </div>
          <div ref={railThumbRef} className="absolute top-1/2 h-[11px] w-[11px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-live" style={{ left: "0%" }} />
        </div>
        <span ref={clockRef} className="w-[112px] flex-none text-right font-mono text-[11px] text-muted" />
      </div>
    </div>
  );
}

function TraceRow({ row, total, gutter, registerRef }: { row: LaidSpan; total: number; gutter: number; registerRef: (h: RowHandle) => void }) {
  const left = (row.start / total) * 100;
  const width = Math.max(0.3, (row.dur / total) * 100);
  const ac = row.agentId ? agentColor(row.agentId) : null;
  const fillColor = row.status === "error" ? "var(--color-destructive)" : row.kind === "agent" ? "var(--color-accent)" : row.kind === "marker" ? "var(--color-warning)" : "var(--color-live)";

  return (
    <div
      ref={(el) => registerRef({ el, fill: el?.querySelector<HTMLDivElement>("[data-part=fill]") ?? null, d: el?.querySelector<HTMLSpanElement>("[data-part=d]") ?? null, ms: el?.querySelector<HTMLSpanElement>("[data-part=ms]") ?? null })}
      data-state="queued"
      className="group relative flex h-[30px] items-center hover:bg-white/[0.025]"
    >
      <div className="flex flex-none items-center gap-1.5 overflow-hidden pr-2.5" style={{ width: gutter, paddingLeft: row.depth * 14 + 10 }} title={row.label}>
        <span style={{ color: ac ?? "var(--color-muted)" }} className="shrink-0 opacity-80 group-data-[state=queued]:opacity-40">
          <TraceIcon kind={row.kind} />
        </span>
        <b className="truncate font-mono text-[11.5px] font-medium group-data-[state=queued]:text-muted">{row.label}</b>
      </div>
      <div className="relative h-full flex-1">
        <span className="absolute top-1/2 h-1.5 -translate-y-1/2 rounded-full bg-white/[0.055]" style={{ left: `${left}%`, width: `${width}%` }} />
        <div data-part="fill" className="absolute top-1/2 h-1.5 origin-left -translate-y-1/2 rounded-full" style={{ left: `${left}%`, width: `${width}%`, background: fillColor, transform: "translateY(-50%) scaleX(0)" }} />
      </div>
      <div className="flex w-[150px] flex-none items-center justify-end gap-2 pl-2.5 opacity-0 transition-opacity group-data-[state=done]:opacity-100 group-data-[state=error]:opacity-100">
        <span data-part="d" className="truncate font-mono text-[11px] text-muted" />
        <span data-part="ms" className="flex-none font-mono text-[11px] tabular-nums" />
      </div>
    </div>
  );
}
