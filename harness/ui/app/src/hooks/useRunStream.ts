import { useEffect, useRef, useState } from "react";
import type { Envelope, RunSummary } from "@/lib/api";

export interface LogLine {
  id: number;
  text: string;
}

/** Mirrors the harness's own live event stream: replays what's in flight, then follows it. */
export function useRunStream() {
  const [run, setRun] = useState<RunSummary | null>(null);
  const [events, setEvents] = useState<Envelope[]>([]);
  const [log, setLog] = useState<LogLine[]>([]);
  const logIdRef = useRef(0);

  useEffect(() => {
    const source = new EventSource("/api/stream");
    source.addEventListener("replay", (e) => {
      const data = JSON.parse((e as MessageEvent).data);
      setEvents(data.events ?? []);
      setLog((data.log ?? []).map((text: string) => ({ id: logIdRef.current++, text })));
      setRun(data.run ?? null);
    });
    source.addEventListener("start", (e) => {
      setEvents([]);
      setLog([]);
      setRun(JSON.parse((e as MessageEvent).data));
    });
    source.addEventListener("telemetry", (e) => {
      setEvents((prev) => [...prev, JSON.parse((e as MessageEvent).data)]);
    });
    source.addEventListener("log", (e) => {
      setLog((prev) => [...prev, { id: logIdRef.current++, text: JSON.parse((e as MessageEvent).data) }]);
    });
    source.addEventListener("end", (e) => {
      setRun(JSON.parse((e as MessageEvent).data));
    });
    return () => source.close();
  }, []);

  return { run, events, log };
}
