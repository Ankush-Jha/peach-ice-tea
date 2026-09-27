import { useEffect, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input, Textarea } from "@/components/ui/input";
import { api, type Status } from "@/lib/api";

export function RunForm({ status, running, onStarted, onStop }: { status: Status | null; running: boolean; onStarted: () => void; onStop: () => void }) {
  const [repo, setRepo] = useState("");
  const [prompt, setPrompt] = useState("");
  const [profile, setProfile] = useState("auto");
  const [model, setModel] = useState("");
  const [testCommand, setTestCommand] = useState("");
  const [maxDuration, setMaxDuration] = useState("");
  const [fixture, setFixture] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    if (status?.defaultRepo && !repo) setRepo(status.defaultRepo);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status]);

  const activeProfileName = profile === "auto" ? status?.autoProfile : profile;
  const activeProfile = status?.profiles.find((p) => p.name === activeProfileName);

  async function loadFixture() {
    if (!fixture) return;
    try {
      const f = await api<{ repo: string; prompt: string; testCommand: string }>("/api/fixture", { name: fixture });
      setRepo(f.repo);
      setPrompt(f.prompt.trim());
      setTestCommand(f.testCommand || "");
      setError("");
    } catch (err: any) {
      setError(err.message);
    }
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError("");
    try {
      await api("/api/run", {
        repo: repo.trim(),
        prompt,
        profile,
        model,
        testCommand,
        maxDurationSecs: Number(maxDuration) || undefined,
      });
      onStarted();
    } catch (err: any) {
      setError(err.message);
    }
  }

  return (
    <Card>
      <CardHeader><CardTitle>Task</CardTitle></CardHeader>
      <CardContent>
        <form onSubmit={submit} className="flex flex-col gap-3.5">
          <label className="flex flex-col gap-1.5 text-[13px] font-semibold">
            Try a sample
            <span className="flex gap-2">
              <select
                value={fixture}
                onChange={(e) => setFixture(e.target.value)}
                className="h-9 flex-1 rounded-md border border-border bg-panel-2 px-2 text-sm outline-none focus:border-accent"
              >
                <option value="">Choose a sample repository…</option>
                {status?.fixtures.map((f) => <option key={f} value={f}>{f}</option>)}
              </select>
              <Button type="button" variant="default" onClick={loadFixture}>Load</Button>
            </span>
            <span className="text-xs font-normal text-muted">Copies the sample to a scratch folder, so it is never modified.</span>
          </label>

          <label className="flex flex-col gap-1.5 text-[13px] font-semibold">
            Repository
            <Input value={repo} onChange={(e) => setRepo(e.target.value)} placeholder="/path/to/repository" required />
          </label>

          <label className="flex flex-col gap-1.5 text-[13px] font-semibold">
            Task
            <Textarea value={prompt} onChange={(e) => setPrompt(e.target.value)} placeholder="Describe the issue to fix…" required className="min-h-[150px]" />
          </label>

          <label className="flex flex-col gap-1.5 text-[13px] font-semibold">
            Profile
            <select
              value={profile}
              onChange={(e) => setProfile(e.target.value)}
              className="h-9 rounded-md border border-border bg-panel-2 px-2 text-sm outline-none focus:border-accent"
            >
              <option value="auto">Automatic ({status?.autoProfile}{status?.profiles.find((p) => p.name === status.autoProfile)?.keyAvailable ? "" : " — no key"})</option>
              {status?.profiles.map((p) => <option key={p.name} value={p.name}>{p.name} · {p.role}{p.keyAvailable ? "" : " — no key"}</option>)}
            </select>
            <span className={"text-xs font-normal " + (activeProfile && !activeProfile.keyAvailable ? "text-destructive" : "text-muted")}>
              {activeProfile ? (activeProfile.keyAvailable ? `Key found in ${activeProfile.keyVar}${activeProfile.role === "development" ? ". Development profile: not for the judged run." : "."}` : `No key: export ${activeProfile.keyVar} (or AI_API_KEY) and restart make ui.`) : ""}
            </span>
          </label>

          <label className="flex flex-col gap-1.5 text-[13px] font-semibold">
            Model <span className="font-normal text-muted">Optional; the profile's default otherwise.</span>
            <Input value={model} onChange={(e) => setModel(e.target.value)} list="models" placeholder={activeProfile?.defaultModel ? `default: ${activeProfile.defaultModel}` : "profile default"} />
            <datalist id="models">{activeProfile?.models.map((m) => <option key={m} value={m} />)}</datalist>
          </label>

          <div className="flex gap-2">
            <label className="flex flex-1 flex-col gap-1.5 text-[13px] font-semibold">
              Test command
              <Input value={testCommand} onChange={(e) => setTestCommand(e.target.value)} placeholder="detected" />
            </label>
            <label className="flex w-[110px] flex-none flex-col gap-1.5 text-[13px] font-semibold">
              Time limit (s)
              <Input type="number" min={60} step={60} value={maxDuration} onChange={(e) => setMaxDuration(e.target.value)} placeholder="default" />
            </label>
          </div>

          {error && <div className="rounded-md bg-destructive-soft px-2.5 py-2 text-[13px] text-destructive">{error}</div>}

          <div className="flex gap-2">
            <Button type="submit" variant="primary" className="flex-1" disabled={running}>Start run</Button>
            <Button type="button" variant="danger" onClick={onStop} disabled={!running}>Stop</Button>
          </div>
        </form>
      </CardContent>
    </Card>
  );
}
