export async function api<T = any>(path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, body === undefined ? {} : { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
  return data as T;
}

export interface Profile {
  name: string;
  keyVar: string;
  role: string;
  defaultModel: string | null;
  models: string[];
  keyAvailable: boolean;
}

export interface Status {
  binary: string | null;
  profiles: Profile[];
  autoProfile: string;
  fixtures: string[];
  defaultRepo: string;
  run: RunSummary | null;
}

export interface RunSummary {
  id: string;
  evidenceId: string;
  startedAt: number;
  profile: string;
  model: string | null;
  repo: string;
  exitCode: number | null;
  running: boolean;
}

export interface Envelope {
  schema_id?: string;
  schema_version?: string;
  run_id?: string;
  seq?: number;
  timestamp: string;
  conversation_id?: string;
  agent_id?: string;
  event: { type: string; [k: string]: any };
}

export interface BundleRow {
  id: string;
  root: string;
  name: string;
  outcome: string;
  model: string | null;
  calls: number | null;
  inputTokens: number | null;
  outputTokens: number | null;
  wallMs: number | null;
  tests: string | null;
  mtimeMs: number;
}

export interface BundleDetail {
  summary: BundleRow | null;
  reportMd: string | null;
  prompt: string | null;
  diff: string | null;
  telemetry: Envelope[];
}

export interface ReportRow {
  kind: string;
  name: string;
  mtimeMs: number;
}
