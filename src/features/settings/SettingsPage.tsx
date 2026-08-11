import {
  CheckCircle2,
  CircleX,
  Cookie,
  DownloadCloud,
  Network,
  RefreshCw,
  RotateCcw,
  Save,
  Settings2,
  ShieldCheck,
  Trash2,
  Wrench,
} from "lucide-react";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { Page } from "../../components/Page";
import { Button, Field, Input, Select } from "../../components/ui";
import { useAppStore } from "../../stores/app-store";
import { api, normalizeError } from "../../lib/tauri";
import type { BrowserCookieSource, EngineId, EngineUpdateCandidate, ManagedEngineStatus, Preset, PresetIntent } from "../../types";

const browserOptions: { value: BrowserCookieSource; label: string }[] = [
  { value: "none", label: "Do not use browser cookies" },
  { value: "firefox", label: "Firefox" },
  { value: "chromium", label: "Chromium" },
  { value: "chrome", label: "Google Chrome" },
  { value: "brave", label: "Brave" },
  { value: "vivaldi", label: "Vivaldi" },
  { value: "edge", label: "Microsoft Edge" },
  { value: "opera", label: "Opera" },
];

export function SettingsPage() {
  const engines = useAppStore((state) => state.engines);
  const preferences = useAppStore((state) => state.enginePreferences);
  const setPreferences = useAppStore((state) => state.setEnginePreferences);
  const savePreferences = useAppStore((state) => state.saveEnginePreferences);
  const loading = useAppStore((state) => state.engineLoading);
  const detect = useAppStore((state) => state.detectEngines);
  const setError = useAppStore((state) => state.setError);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [testUrl, setTestUrl] = useState("");
  const [testResult, setTestResult] = useState("");
  const [testing, setTesting] = useState(false);
  const [managed, setManaged] = useState<ManagedEngineStatus[]>([]);
  const [candidate, setCandidate] = useState<EngineUpdateCandidate | null>(null);
  const [managing, setManaging] = useState<EngineId | null>(null);

  useEffect(() => {
    void api.listManagedEngines().then(setManaged).catch((error) => setError(normalizeError(error)));
  }, [setError]);

  async function save() {
    setSaving(true);
    setSaved(false);
    setSaved(await savePreferences());
    setSaving(false);
  }

  function replaceManaged(status: ManagedEngineStatus) {
    setManaged((items) => items.map((item) => item.engine === status.engine ? status : item));
  }

  async function checkUpdate(engine: EngineId) {
    setManaging(engine);
    setCandidate(null);
    try {
      setCandidate(await api.checkEngineUpdate(engine));
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setManaging(null);
    }
  }

  async function installUpdate(update: EngineUpdateCandidate) {
    setManaging(update.engine);
    try {
      replaceManaged(await api.installEngineUpdate(update));
      setCandidate(null);
      await detect();
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setManaging(null);
    }
  }

  async function testAccess() {
    if (!testUrl.trim()) return;
    setTesting(true);
    setTestResult("");
    try {
      const result = await api.testSiteAccess(testUrl.trim());
      setTestResult(`Access succeeded: ${result.siteName} — ${result.title}`);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setTesting(false);
    }
  }

  async function exportDiagnostics() {
    const path = await saveDialog({
      defaultPath: "nice-downloader-diagnostics.json",
      filters: [{ name: "JSON diagnostic bundle", extensions: ["json"] }],
    });
    if (!path) return;
    try {
      await api.exportDiagnostics(path);
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  return (
    <Page
      title="Settings"
      description="Application and engine configuration."
      actions={
        <Button variant="secondary" disabled={loading} onClick={() => void detect()}>
          <RefreshCw className={loading ? "animate-spin" : ""} size={15} />
          Detect again
        </Button>
      }
    >
      <section className="mb-8">
        <div className="mb-3 flex items-center gap-2">
          <ShieldCheck size={17} className="text-muted" />
          <h2 className="text-sm font-semibold">Site access</h2>
        </div>
        <div className="border border-line bg-panel p-5">
          <div className="grid gap-5 md:grid-cols-2">
            <Field
              label="Browser cookie source"
              hint="Used locally by yt-dlp for sites that require a signed-in session. Cookie contents never pass through React."
            >
              <div className="relative">
                <Cookie className="pointer-events-none absolute left-3 top-3.5 text-muted" size={16} />
                <Select
                  className="pl-9"
                  value={preferences.browserCookieSource}
                  onChange={(event) => {
                    setSaved(false);
                    setPreferences({
                      ...preferences,
                      browserCookieSource: event.target.value as BrowserCookieSource,
                      browserProfile: event.target.value === "none" ? null : preferences.browserProfile,
                    });
                  }}
                >
                  {browserOptions.map((option) => (
                    <option key={option.value} value={option.value}>{option.label}</option>
                  ))}
                </Select>
              </div>
            </Field>

            <Field
              label="Browser profile name"
              hint="Optional profile directory name, for example work.default-release. Paths are intentionally not accepted."
            >
              <Input
                value={preferences.browserProfile || ""}
                disabled={preferences.browserCookieSource === "none"}
                placeholder="default-release"
                onChange={(event) => {
                  setSaved(false);
                  setPreferences({ ...preferences, browserProfile: event.target.value || null });
                }}
                spellCheck={false}
              />
            </Field>

            <Field
              label="Network proxy"
              hint="Optional HTTP(S) or SOCKS proxy. Credentials are not stored in this milestone."
            >
              <div className="relative">
                <Network className="pointer-events-none absolute left-3 top-3.5 text-muted" size={16} />
                <Input
                  className="pl-9"
                  value={preferences.proxyUrl || ""}
                  placeholder="socks5://127.0.0.1:1080"
                  onChange={(event) => {
                    setSaved(false);
                    setPreferences({ ...preferences, proxyUrl: event.target.value || null });
                  }}
                  spellCheck={false}
                />
              </div>
            </Field>

            <Field
              label="Bandwidth limit (KiB/s)"
              hint="Optional global limit applied to yt-dlp, gallery-dl, aria2c, and curl. Minimum 64 KiB/s."
            >
              <Input
                type="number"
                min={64}
                value={preferences.bandwidthLimitKib ?? ""}
                placeholder="Unlimited"
                onChange={(event) => {
                  setSaved(false);
                  setPreferences({ ...preferences, bandwidthLimitKib: event.target.value ? Number(event.target.value) : null });
                }}
              />
            </Field>

            <div>
              <div className="text-sm font-medium text-ink">Request compatibility</div>
              <label className="mt-3 flex items-start gap-3 text-sm text-muted">
                <input
                  className="mt-0.5"
                  type="checkbox"
                  checked={preferences.enableImpersonation}
                  onChange={(event) => {
                    setSaved(false);
                    setPreferences({ ...preferences, enableImpersonation: event.target.checked });
                  }}
                />
                <span>
                  Use browser impersonation when the installed yt-dlp runtime supports it.
                  This can reduce anti-bot failures without exposing custom arguments.
                </span>
              </label>
            </div>
          </div>
          <div className="mt-5 flex items-center justify-between border-t border-line pt-4">
            <p className="text-xs text-muted">
              Close the selected browser if its cookie database is locked. Your desktop keyring may request access.
            </p>
            <div className="flex items-center gap-3">
              {saved ? <span className="text-xs text-emerald-400">Saved</span> : null}
              <Button disabled={saving} onClick={() => void save()}>
                <Save size={15} /> {saving ? "Saving" : "Save access settings"}
              </Button>
            </div>
          </div>
          <div className="mt-4 border-t border-line pt-4">
            <Field
              label="Test cookies and site access"
              hint="Runs a bounded metadata-only analysis with the saved browser/profile/proxy settings. No cookie values are returned or logged."
            >
              <div className="flex gap-2">
                <Input value={testUrl} onChange={(event) => setTestUrl(event.target.value)} placeholder="https://www.reddit.com/r/..." spellCheck={false} />
                <Button variant="secondary" disabled={testing || !testUrl.trim()} onClick={() => void testAccess()}>
                  {testing ? <RefreshCw className="animate-spin" size={15} /> : <ShieldCheck size={15} />}
                  Test access
                </Button>
              </div>
            </Field>
            {testResult ? <p className="mt-2 text-xs text-emerald-400">{testResult}</p> : null}
          </div>
        </div>
      </section>

      <section className="mb-8">
        <div className="mb-3 flex items-center gap-2">
          <DownloadCloud size={17} className="text-muted" />
          <h2 className="text-sm font-semibold">Managed engine versions</h2>
        </div>
        <div className="divide-y divide-line border border-line bg-panel">
          {managed.map((item) => (
            <article key={item.engine} className="flex flex-wrap items-center gap-3 p-4">
              <div className="min-w-48 flex-1">
                <h3 className="text-sm font-semibold">{item.engine === "ytDlp" ? "yt-dlp" : "gallery-dl"}</h3>
                <p className="mt-1 text-xs text-muted">
                  {item.activeVersion ? `Managed ${item.activeVersion} active` : "Using the executable from PATH"}
                  {item.pinned ? " · pinned" : ""}
                </p>
              </div>
              <Button variant="secondary" className="h-8 px-2.5" disabled={managing === item.engine} onClick={() => void checkUpdate(item.engine)}>
                <RefreshCw className={managing === item.engine ? "animate-spin" : ""} size={14} /> Check
              </Button>
              <Button
                variant="ghost"
                className="h-8 px-2.5"
                onClick={() => void api.pinManagedEngine(item.engine, !item.pinned).then(replaceManaged).catch((error) => setError(normalizeError(error)))}
              >
                {item.pinned ? "Unpin" : "Pin version"}
              </Button>
              <Button
                variant="ghost"
                className="h-8 px-2.5"
                disabled={!item.previousVersion}
                onClick={() => void api.rollbackManagedEngine(item.engine).then((status) => { replaceManaged(status); void detect(); }).catch((error) => setError(normalizeError(error)))}
              >
                <RotateCcw size={14} /> Roll back
              </Button>
            </article>
          ))}
          {candidate ? (
            <div className="flex items-center gap-3 bg-elevated p-4 text-sm">
              <div className="flex-1">
                Verified release <span className="font-semibold text-ink">{candidate.version}</span> is available.
                <p className="mt-1 font-mono text-[10px] text-muted">SHA-256 {candidate.sha256}</p>
              </div>
              <Button disabled={managing === candidate.engine} onClick={() => void installUpdate(candidate)}>
                <DownloadCloud size={15} /> Install verified update
              </Button>
            </div>
          ) : null}
        </div>
        <p className="mt-3 text-xs leading-5 text-muted">Managed files are checked against vendor-published SHA-256 manifests, version-tested, and retained for one-click rollback. FFmpeg remains system-managed.</p>
      </section>

      <section>
        <div className="mb-3 flex items-center gap-2">
          <Wrench size={17} className="text-muted" />
          <h2 className="text-sm font-semibold">Engines and capabilities</h2>
        </div>
        <div className="divide-y divide-line border border-line bg-panel">
          {engines.map((engine) => (
            <article key={engine.id} className="px-4 py-4">
              <div className="grid grid-cols-[36px_minmax(0,1fr)_auto] items-center gap-4">
                <div className={`grid size-9 place-items-center rounded-md ${engine.installed ? "bg-emerald-500/10 text-emerald-400" : "bg-red-500/10 text-red-400"}`}>
                  {engine.installed ? <CheckCircle2 size={18} /> : <CircleX size={18} />}
                </div>
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <h3 className="text-sm font-semibold">{engine.displayName}</h3>
                    {engine.required ? <span className="bg-blue-500/10 px-1.5 py-0.5 text-[10px] font-medium uppercase text-blue-400">Required</span> : null}
                  </div>
                  <p className="mt-1 truncate font-mono text-xs text-muted">{engine.executablePath || engine.statusMessage}</p>
                </div>
                <div className="text-right">
                  <div className={`text-xs font-medium ${engine.installed ? "text-emerald-400" : "text-red-400"}`}>{engine.installed ? "Installed" : "Not found"}</div>
                  <div className="mt-1 max-w-64 truncate text-xs text-muted">{engine.version || "—"}</div>
                </div>
              </div>
              {engine.capabilities.length > 0 ? (
                <div className="ml-[52px] mt-3 grid gap-2 border-t border-line/70 pt-3 sm:grid-cols-2">
                  {engine.capabilities.map((capability) => (
                    <div key={capability.id} className="flex items-start gap-2 text-xs">
                      <span className={`mt-1 size-1.5 shrink-0 rounded-full ${capability.available ? "bg-emerald-400" : "bg-amber-400"}`} />
                      <div>
                        <span className="font-medium text-ink">{capability.displayName}</span>
                        <p className="mt-0.5 text-muted">{capability.statusMessage}</p>
                      </div>
                    </div>
                  ))}
                </div>
              ) : null}
            </article>
          ))}
          {!loading && engines.length === 0 ? <div className="p-5 text-sm text-muted">No engine status is available.</div> : null}
        </div>
        <p className="mt-3 text-xs leading-5 text-muted">Managed yt-dlp/gallery-dl versions take priority over PATH. Other engines are discovered from PATH.</p>
        <div className="mt-5 flex items-center justify-between border border-line bg-panel p-4">
          <div><h3 className="text-sm font-semibold">Support diagnostics</h3><p className="mt-1 text-xs text-muted">Export engine status and sanitized recent errors. URLs, cookies, proxy values, and profile names are excluded.</p></div>
          <Button variant="secondary" onClick={() => void exportDiagnostics()}>Export diagnostics</Button>
        </div>
      </section>
    </Page>
  );
}

export function PresetsPage() {
  const setError = useAppStore((state) => state.setError);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [name, setName] = useState("");
  const [intent, setIntent] = useState<PresetIntent>({
    quality: "best",
    container: "mp4",
    videoCodecPreference: "any",
    audioQuality: "best",
    embedThumbnail: false,
    embedMetadata: true,
    outputConflictPolicy: "skip",
  });

  useEffect(() => {
    void api.listPresets().then(setPresets).catch((error) => setError(normalizeError(error)));
  }, [setError]);

  async function createPreset() {
    try {
      const preset = await api.savePreset({ name, intent });
      setPresets((items) => [...items, preset].sort((a, b) => a.name.localeCompare(b.name)));
      setName("");
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  return (
    <Page title="Presets" description="Reusable download intentions stored locally in SQLite.">
      <section className="border border-line bg-panel p-5">
        <div className="mb-4 flex items-center gap-2"><Settings2 size={17} className="text-muted" /><h2 className="text-sm font-semibold">New preset</h2></div>
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <Field label="Name"><Input value={name} onChange={(event) => setName(event.target.value)} placeholder="1080p compatibility" /></Field>
          <Field label="Quality"><Select value={intent.quality} onChange={(event) => setIntent({ ...intent, quality: event.target.value as PresetIntent["quality"] })}>
            <option value="best">Best</option><option value="2160">2160p</option><option value="1440">1440p</option><option value="1080">1080p</option><option value="720">720p</option><option value="480">480p</option><option value="audio">Audio only</option>
          </Select></Field>
          <Field label="Container"><Select value={intent.container} onChange={(event) => setIntent({ ...intent, container: event.target.value as PresetIntent["container"] })}>
            {(["mp4", "mkv", "webm", "mp3", "m4a", "opus"] as const).map((value) => <option key={value} value={value}>{value.toUpperCase()}</option>)}
          </Select></Field>
          <Field label="Video codec"><Select value={intent.videoCodecPreference} onChange={(event) => setIntent({ ...intent, videoCodecPreference: event.target.value as PresetIntent["videoCodecPreference"] })}>
            <option value="any">Automatic</option><option value="h264">H.264</option><option value="av1">AV1</option><option value="vp9">VP9</option>
          </Select></Field>
        </div>
        <div className="mt-4 flex flex-wrap items-center gap-5 text-sm text-muted">
          <label className="flex items-center gap-2"><input type="checkbox" checked={intent.embedMetadata} onChange={(event) => setIntent({ ...intent, embedMetadata: event.target.checked })} /> Embed metadata</label>
          <label className="flex items-center gap-2"><input type="checkbox" checked={intent.embedThumbnail} onChange={(event) => setIntent({ ...intent, embedThumbnail: event.target.checked })} /> Embed thumbnail</label>
          <Button className="ml-auto" disabled={!name.trim()} onClick={() => void createPreset()}>Save preset</Button>
        </div>
      </section>
      <div className="mt-5 divide-y divide-line border border-line bg-panel">
        {presets.map((preset) => (
          <article key={preset.id} className="flex items-center gap-4 p-4">
            <Settings2 size={18} className="text-muted" />
            <div className="min-w-0 flex-1"><h3 className="text-sm font-semibold">{preset.name}</h3><p className="mt-1 text-xs text-muted">{preset.intent.quality === "audio" ? "Audio only" : preset.intent.quality === "best" ? "Best quality" : `${preset.intent.quality}p`} · {preset.intent.container.toUpperCase()} · {preset.intent.videoCodecPreference.toUpperCase()}</p></div>
            <Button variant="danger" className="h-8 px-2.5" onClick={() => void api.deletePreset(preset.id).then(() => setPresets((items) => items.filter((item) => item.id !== preset.id))).catch((error) => setError(normalizeError(error)))}><Trash2 size={14} /> Delete</Button>
          </article>
        ))}
        {presets.length === 0 ? <p className="p-5 text-sm text-muted">No presets saved yet.</p> : null}
      </div>
    </Page>
  );
}
