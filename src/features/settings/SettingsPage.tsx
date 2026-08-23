import {
  AlertTriangle,
  CheckCircle2,
  CircleX,
  Cookie,
  DownloadCloud,
  Eraser,
  Globe2,
  HardDrive,
  KeyRound,
  Lock,
  MonitorPlay,
  Network,
  RefreshCw,
  RotateCcw,
  Save,
  Settings2,
  ShieldCheck,
  Trash2,
  Wrench,
} from "lucide-react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { Page } from "../../components/Page";
import { Button, Field, Input, Select } from "../../components/ui";
import { useAppStore } from "../../stores/app-store";
import { api, normalizeError } from "../../lib/tauri";
import { formatBytes } from "../../lib/format";
import type {
  BrowserCookieSource,
  AuthenticationProfile,
  BrowserProfileInfo,
  DesktopPreferences,
  LibraryPlaybackCacheStatus,
  LibraryPrivacyStatus,
  NativePlaybackStatus,
  EngineId,
  EngineUpdateCandidate,
  ManagedEngineStatus,
  Preset,
  PresetIntent,
  SitePresetRule,
  SponsorBlockCategory,
} from "../../types";

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

const sponsorCategoryOptions: { value: SponsorBlockCategory; label: string }[] = [
  { value: "sponsor", label: "Sponsors" },
  { value: "intro", label: "Intros" },
  { value: "outro", label: "Outros" },
  { value: "selfPromotion", label: "Self-promotion" },
  { value: "interaction", label: "Interaction reminders" },
  { value: "preview", label: "Previews" },
  { value: "filler", label: "Filler" },
  { value: "musicOfftopic", label: "Off-topic music" },
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
  const [settingsTab, setSettingsTab] = useState<"access" | "profiles" | "privacy" | "desktop" | "engines">("access");
  const missingRequiredEngines = engines.filter((engine) => engine.required && !engine.installed);

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
      <div className="mb-7 inline-flex gap-1 rounded-[10px] border border-line/60 bg-panel/55 p-1 shadow-control" role="tablist" aria-label="Settings sections">
        {([ ["access", "Network & cookies"], ["profiles", "Site profiles"], ["privacy", "Library privacy"], ["desktop", "Desktop"], ["engines", "Engines & diagnostics"] ] as const).map(([id, label]) => <button key={id} role="tab" aria-selected={settingsTab === id} onClick={() => setSettingsTab(id)} className={`rounded-[7px] px-4 py-2 text-[13px] font-semibold transition-all duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60 ${settingsTab === id ? "bg-elevated text-ink shadow-control" : "text-muted hover:text-ink"}`}>{label}</button>)}
      </div>
      {settingsTab === "access" ? (
      <section className="mb-8">
        <div className="mb-3 flex items-center gap-2">
          <ShieldCheck size={17} className="text-muted" />
          <h2 className="text-sm font-semibold">Site access</h2>
        </div>
        <div className="panel-shadow rounded-[12px] border border-line/60 bg-panel/75 p-5">
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
      ) : null}

      {settingsTab === "profiles" ? <AuthenticationProfiles /> : null}
      {settingsTab === "privacy" ? <LibraryPrivacySettings /> : null}
      {settingsTab === "desktop" ? <DesktopSettings /> : null}

      {settingsTab === "engines" ? <>
      {missingRequiredEngines.length > 0 ? (
        <div className="mb-6 flex items-start gap-3 rounded-[11px] border border-amber-400/25 bg-amber-400/[0.07] p-4" role="status">
          <AlertTriangle className="mt-0.5 shrink-0 text-amber-400" size={18} />
          <div>
            <h2 className="text-sm font-semibold text-ink">Required tools are missing</h2>
            <p className="mt-1 text-xs leading-5 text-muted">
              This may be a Slim build. Install {missingRequiredEngines.map((engine) => engine.displayName).join(" and ")} through your operating system, or install a verified managed version when offered below. Then select Detect again.
            </p>
          </div>
        </div>
      ) : null}
      <section className="mb-8">
        <div className="mb-3 flex items-center gap-2">
          <DownloadCloud size={17} className="text-muted" />
          <h2 className="text-sm font-semibold">Managed engine versions</h2>
        </div>
        <div className="panel-shadow divide-y divide-line/65 overflow-hidden rounded-[11px] border border-line/60 bg-panel/70">
          {managed.map((item) => (
            <article key={item.engine} className="flex flex-wrap items-center gap-3 p-4">
              <div className="min-w-48 flex-1">
                <h3 className="text-sm font-semibold">{item.engine === "ytDlp" ? "yt-dlp" : "gallery-dl"}</h3>
                <p className="mt-1 text-xs text-muted">
                  {item.activeVersion ? `Managed ${item.activeVersion} active` : "Using the bundled baseline or a system executable"}
                  {item.pinned ? " · pinned" : ""}
                </p>
              </div>
              <Button variant="secondary" className="h-8 px-2.5" disabled={!item.supported || managing === item.engine} onClick={() => void checkUpdate(item.engine)}>
                <RefreshCw className={managing === item.engine ? "animate-spin" : ""} size={14} /> Check
              </Button>
              <Button
                variant="ghost"
                className="h-8 px-2.5"
                disabled={!item.supported}
                onClick={() => void api.pinManagedEngine(item.engine, !item.pinned).then(replaceManaged).catch((error) => setError(normalizeError(error)))}
              >
                {item.pinned ? "Unpin" : "Pin version"}
              </Button>
              <Button
                variant="ghost"
                className="h-8 px-2.5"
                disabled={!item.supported || !item.previousVersion}
                onClick={() => void api.rollbackManagedEngine(item.engine).then((status) => { replaceManaged(status); void detect(); }).catch((error) => setError(normalizeError(error)))}
              >
                <RotateCcw size={14} /> Roll back
              </Button>
            </article>
          ))}
          {candidate ? (
            <div className="flex items-center gap-3 bg-accent/[0.06] p-4 text-sm">
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
        <p className="mt-3 text-xs leading-5 text-muted">Managed files are checked against vendor-published SHA-256 manifests, version-tested, and retained for one-click rollback. Release packages provide bundled engine baselines.</p>
      </section>

      <section>
        <div className="mb-3 flex items-center gap-2">
          <Wrench size={17} className="text-muted" />
          <h2 className="text-sm font-semibold">Engines and capabilities</h2>
        </div>
        <div className="panel-shadow divide-y divide-line/65 overflow-hidden rounded-[11px] border border-line/60 bg-panel/70">
          {engines.map((engine) => (
            <article key={engine.id} className="px-4 py-4">
              <div className="grid grid-cols-[36px_minmax(0,1fr)_auto] items-center gap-4">
                <div className={`grid size-9 place-items-center rounded-[8px] border ${engine.installed ? "border-emerald-500/15 bg-emerald-500/[0.07] text-emerald-400" : "border-red-500/15 bg-red-500/[0.07] text-red-400"}`}>
                  {engine.installed ? <CheckCircle2 size={18} /> : <CircleX size={18} />}
                </div>
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <h3 className="text-sm font-semibold">{engine.displayName}</h3>
                    {engine.required ? <span className="rounded-[4px] bg-accent/10 px-1.5 py-0.5 text-[9px] font-semibold tracking-[0.04em] text-accent">Required</span> : null}
                    {engine.source ? <span className="rounded-[4px] border border-line/70 px-1.5 py-0.5 text-[9px] font-semibold tracking-[0.04em] text-muted">{engine.source === "managed" ? "Managed" : engine.source === "bundled" ? "Bundled" : "System"}</span> : null}
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
        <p className="mt-3 text-xs leading-5 text-muted">Resolution order: managed update, bundled baseline when present, then PATH and standard user executable folders. Slim builds omit the bundled baseline.</p>
        <div className="panel-shadow mt-5 flex items-center justify-between rounded-[10px] border border-line/60 bg-panel/70 p-4">
          <div><h3 className="text-sm font-semibold">Support diagnostics</h3><p className="mt-1 text-xs text-muted">Export engine status and sanitized recent errors. URLs, cookies, proxy values, and profile names are excluded.</p></div>
          <Button variant="secondary" onClick={() => void exportDiagnostics()}>Export diagnostics</Button>
        </div>
      </section>
      </> : null}
    </Page>
  );
}

function LibraryPrivacySettings() {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [status, setStatus] = useState<LibraryPrivacyStatus | null>(null);
  const [passphrase, setPassphrase] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [disablePassphrase, setDisablePassphrase] = useState("");
  const [disableConfirmed, setDisableConfirmed] = useState(false);
  const [disableOpen, setDisableOpen] = useState(false);
  const [autoLockMinutes, setAutoLockMinutes] = useState(15);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const update = (next: LibraryPrivacyStatus) => {
      if (disposed) return;
      setStatus(next);
      setAutoLockMinutes(next.autoLockMinutes);
      if (!next.configured || next.locked) {
        setDisablePassphrase("");
        setDisableConfirmed(false);
        setDisableOpen(false);
      }
    };
    void api.getLibraryPrivacyStatus().then(update).catch((error) => setError(normalizeError(error)));
    void api.onLibraryPrivacyChanged(update).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [setError]);

  async function configure() {
    if (passphrase !== confirmation) {
      setError({ code: "library_privacy_confirmation", message: "The Library privacy passphrases do not match." });
      return;
    }
    setBusy(true);
    try {
      const next = await api.configureLibraryPrivacy({ passphrase, autoLockMinutes });
      setStatus(next);
      setPassphrase("");
      setConfirmation("");
      setNotice({ message: status?.configured ? "Library privacy settings updated." : "Private Library protection enabled.", view: "settings" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function unlock() {
    setBusy(true);
    try {
      const next = await api.unlockLibraryPrivacy(passphrase);
      setStatus(next);
      setPassphrase("");
      setNotice({ message: "Private Library unlocked for this session.", view: "settings" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function lock() {
    setBusy(true);
    try {
      setStatus(await api.lockLibraryPrivacy());
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function disableProtection() {
    setBusy(true);
    try {
      const next = await api.disableLibraryPrivacy(disablePassphrase);
      setStatus(next);
      setDisablePassphrase("");
      setDisableConfirmed(false);
      setDisableOpen(false);
      const retainedRules = next.privateSourceCount + next.privateFolderCount + next.privateItemCount;
      setNotice({
        message: retainedRules > 0
          ? `Library privacy disabled. ${retainedRules} retained privacy rule${retainedRules === 1 ? " is" : "s are"} currently visible.`
          : "Library privacy disabled and the passphrase was removed.",
        view: "settings",
      });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  if (!status) return <div className="p-8 text-sm text-muted">Loading Library privacy…</div>;

  const retainedRuleCount = status.privateSourceCount + status.privateFolderCount + status.privateItemCount;

  return (
    <section className="mb-8">
      <div className="mb-3 flex items-center gap-2"><Lock size={17} className="text-muted" /><h2 className="text-sm font-semibold">Locked private Library</h2></div>
      <div className="panel-shadow overflow-hidden rounded-[12px] border border-line/60 bg-panel/75">
        <div className="flex flex-wrap items-start gap-4 border-b border-line/60 p-5">
          <div className={`grid size-11 shrink-0 place-items-center rounded-[11px] ${status.configured && status.locked ? "bg-accent/12 text-accent" : "bg-elevated text-muted"}`}>
            {status.configured && status.locked ? <Lock size={20} /> : <KeyRound size={20} />}
          </div>
          <div className="min-w-0 flex-1">
            <h3 className="text-[15px] font-semibold">{!status.configured ? "Set a privacy passphrase" : status.locked ? "Private media is hidden" : "Private media is visible"}</h3>
            <p className="mt-1 text-xs leading-5 text-muted">
              {status.configured
                ? `${status.privateSourceCount} private sources · ${status.privateFolderCount} private folders · ${status.privateItemCount} individual items`
                : retainedRuleCount > 0
                  ? `${retainedRuleCount} retained privacy rule${retainedRuleCount === 1 ? " is" : "s are"} currently visible. Enable protection to lock them again.`
                  : "Protect selected sources, folders, and videos behind an in-memory unlock state."}
            </p>
          </div>
          {status.configured && !status.locked ? <Button variant="secondary" disabled={busy} onClick={() => void lock()}><Lock size={15} /> Lock now</Button> : null}
        </div>

        <div className="grid gap-5 p-5 md:grid-cols-2">
          <Field label={status.configured && status.locked ? "Passphrase" : status.configured ? "New passphrase" : "Passphrase"} hint={status.configured && status.locked ? "Unlocks private records only in this running app session." : "Use 6–256 characters. Nice Downloader stores a salted verifier, never the passphrase."}>
            <Input type="password" autoComplete="off" value={passphrase} onChange={(event) => setPassphrase(event.target.value)} />
          </Field>
          {status.configured && status.locked ? null : <Field label="Confirm passphrase"><Input type="password" autoComplete="off" value={confirmation} onChange={(event) => setConfirmation(event.target.value)} /></Field>}
          {status.configured && status.locked ? null : <Field label="Automatic lock" hint="Activity is checked in Rust; minimizing or hiding the app locks immediately."><Select value={autoLockMinutes} onChange={(event) => setAutoLockMinutes(Number(event.target.value))}>{[1, 5, 15, 30, 60, 120, 240].map((minutes) => <option key={minutes} value={minutes}>{minutes < 60 ? `${minutes} minute${minutes === 1 ? "" : "s"}` : `${minutes / 60} hour${minutes === 60 ? "" : "s"}`}</option>)}</Select></Field>}
        </div>

        <div className="mx-5 mb-5 rounded-[10px] border border-amber-400/20 bg-amber-400/5 p-4 text-xs leading-5 text-muted">
          <strong className="text-ink">App-level privacy, not disk encryption.</strong> Locked items are blocked from the UI, playback, search, analysis, backups, and LAN sharing. Original media files and the SQLite database remain readable to someone with filesystem access.
        </div>
        {disableOpen && status.configured && !status.locked ? (
          <div className="mx-5 mb-5 rounded-[10px] border border-red-400/25 bg-red-400/[0.06] p-4" aria-labelledby="disable-library-privacy-title">
            <h4 id="disable-library-privacy-title" className="text-sm font-semibold text-red-300">Disable Library privacy?</h4>
            <p className="mt-1 text-xs leading-5 text-muted">The passphrase verifier will be deleted. Existing private item, folder, and source rules will be preserved, but their media will be visible in the Library, search, recent activity, playback, and sharing until protection is enabled again.</p>
            <div className="mt-4 grid gap-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-end">
              <Field label="Current passphrase" hint="Required even though the Library is currently unlocked.">
                <Input type="password" autoComplete="current-password" value={disablePassphrase} onChange={(event) => setDisablePassphrase(event.target.value)} />
              </Field>
              <Button variant="danger" disabled={busy || disablePassphrase.length < 6 || !disableConfirmed} onClick={() => void disableProtection()}><Trash2 size={15} /> {busy ? "Disabling…" : "Remove passphrase"}</Button>
            </div>
            <label className="mt-3 flex items-start gap-2 text-xs leading-5 text-muted">
              <input className="mt-1" type="checkbox" checked={disableConfirmed} onChange={(event) => setDisableConfirmed(event.target.checked)} />
              <span>I understand that retained private media will be visible until I enable protection again.</span>
            </label>
          </div>
        ) : null}
        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-line/60 px-5 py-4">
          {status.configured && !status.locked ? <Button variant="ghost" disabled={busy} aria-expanded={disableOpen} onClick={() => { setDisableOpen((open) => !open); setDisablePassphrase(""); setDisableConfirmed(false); }}><Trash2 size={15} /> {disableOpen ? "Cancel disabling" : "Disable protection"}</Button> : <span />}
          {status.configured && status.locked
            ? <Button disabled={busy || passphrase.length < 6} onClick={() => void unlock()}><KeyRound size={15} /> {busy ? "Unlocking…" : "Unlock private Library"}</Button>
            : <Button disabled={busy || passphrase.length < 6 || confirmation.length < 6} onClick={() => void configure()}><ShieldCheck size={15} /> {busy ? "Saving…" : status.configured ? "Update protection" : "Enable protection"}</Button>}
        </div>
      </div>
    </section>
  );
}

function DesktopSettings() {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [preferences, setPreferences] = useState<DesktopPreferences | null>(null);
  const [trayAvailable, setTrayAvailable] = useState(true);
  const [playbackCache, setPlaybackCache] = useState<LibraryPlaybackCacheStatus | null>(null);
  const [nativePlayback, setNativePlayback] = useState<NativePlaybackStatus | null>(null);
  const [clearingPlaybackCache, setClearingPlaybackCache] = useState(false);
  useEffect(() => {
    void Promise.all([api.getDesktopPreferences(), api.getDesktopRuntimeStatus(), api.getLibraryPlaybackCacheStatus(), api.getNativePlaybackStatus()])
      .then(([saved, runtime, cache, native]) => { setPreferences(saved); setTrayAvailable(runtime.trayAvailable); setPlaybackCache(cache); setNativePlayback(native); })
      .catch((error) => setError(normalizeError(error)));
  }, [setError]);
  async function saveDesktop() {
    if (!preferences) return;
    try {
      const saved = await api.updateDesktopPreferences(preferences);
      setPreferences(saved);
      window.dispatchEvent(new CustomEvent("nice-downloader:desktop-preferences", { detail: saved }));
      setNotice({ message: "Desktop and accessibility settings saved.", view: "settings" });
    }
    catch (error) { setError(normalizeError(error)); }
  }
  async function clearPlaybackCache() {
    if (clearingPlaybackCache) return;
    setClearingPlaybackCache(true);
    try {
      const result = await api.clearLibraryPlaybackCache();
      const status = await api.getLibraryPlaybackCacheStatus();
      setPlaybackCache(status);
      setNotice({ message: result.removedFiles > 0 ? `Cleared ${result.removedFiles} compatibility ${result.removedFiles === 1 ? "copy" : "copies"} (${formatBytes(result.freedBytes)}).` : "The compatibility cache is already empty.", view: "settings" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setClearingPlaybackCache(false);
    }
  }
  if (!preferences) return <div className="p-8 text-sm text-muted">Loading desktop preferences…</div>;
  return <section className="mb-8">
    <div className="mb-3 flex items-center gap-2"><Settings2 size={17} className="text-muted" /><h2 className="text-sm font-semibold">Desktop and accessibility</h2></div>
    <div className="panel-shadow border border-line/60 bg-panel/75 p-5">
      <div className="grid gap-5 md:grid-cols-2">
        <Field label="When closing the window" hint={trayAvailable ? "Background mode keeps the Rust queue alive and leaves the app available from the tray." : "The system tray is unavailable, so closing must quit the application."}><Select value={preferences.closeBehavior} onChange={(event) => setPreferences({ ...preferences, closeBehavior: event.target.value as DesktopPreferences["closeBehavior"] })}><option value="ask">Ask while downloads are active</option><option value="background" disabled={!trayAvailable}>Keep running in system tray</option><option value="quit">Quit the application</option></Select></Field>
        <div className="space-y-4 pt-1"><label className="flex items-start gap-3 text-sm text-muted"><input className="mt-1" type="checkbox" disabled={!trayAvailable} checked={preferences.startMinimized && trayAvailable} onChange={(event) => setPreferences({ ...preferences, startMinimized: event.target.checked })} /><span><strong className="block font-medium text-ink">Start in the background</strong>Open from the system tray when ready.</span></label><label className="flex items-start gap-3 text-sm text-muted"><input className="mt-1" type="checkbox" checked={preferences.notifyWhenBackgrounded} onChange={(event) => setPreferences({ ...preferences, notifyWhenBackgrounded: event.target.checked })} /><span><strong className="block font-medium text-ink">Background completion notices</strong>Use desktop notifications when supported.</span></label></div>
      </div>
      <div className="mt-6 grid gap-5 border-t border-line/60 pt-5 md:grid-cols-2"><Field label="Interface scale" hint="Enlarges the complete interface, including fixed-size controls and media details."><Select value={preferences.interfaceScalePercent} onChange={(event) => setPreferences({ ...preferences, interfaceScalePercent: Number(event.target.value) as DesktopPreferences["interfaceScalePercent"] })}><option value={90}>90% — Compact</option><option value={100}>100% — Default</option><option value={110}>110% — Large</option><option value={125}>125% — Extra large</option></Select></Field><div className="space-y-4 pt-1"><label className="flex items-start gap-3 text-sm text-muted"><input className="mt-1" type="checkbox" checked={preferences.reduceMotion} onChange={(event) => setPreferences({ ...preferences, reduceMotion: event.target.checked })} /><span><strong className="block font-medium text-ink">Reduce motion</strong>Stops decorative animation and minimizes transitions.</span></label><label className="flex items-start gap-3 text-sm text-muted"><input className="mt-1" type="checkbox" checked={preferences.highContrast} onChange={(event) => setPreferences({ ...preferences, highContrast: event.target.checked })} /><span><strong className="block font-medium text-ink">Higher contrast</strong>Strengthens borders, muted text, and keyboard focus indicators.</span></label></div></div>
      <div className="mt-6 grid gap-5 border-t border-line/60 pt-5 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
        <Field label="Embedded playback compatibility" hint="Controls whether Nice Downloader may make a temporary stream-copy when the Linux WebView cannot play a file reliably."><Select value={preferences.playbackCompatibility} onChange={(event) => setPreferences({ ...preferences, playbackCompatibility: event.target.value as DesktopPreferences["playbackCompatibility"] })}><option value="ask">Ask before creating a copy</option><option value="automatic">Create copies automatically</option><option value="never">Never create automatically</option></Select></Field>
        <div className="rounded-[10px] border border-line/60 bg-elevated/35 p-4"><div className="flex items-start gap-3"><div className="grid size-9 shrink-0 place-items-center rounded-[8px] bg-elevated text-accent"><HardDrive size={16} /></div><div className="min-w-0 flex-1"><h3 className="text-xs font-semibold text-ink">Compatibility cache</h3><p className="mt-1 text-[11px] leading-5 text-muted">Timing copies use lossless stream-copy. Explicit H.264 fallbacks re-encode only when the WebView rejects that copy. Originals are never changed.</p>{playbackCache ? <p className="number-font mt-2 text-[10px] text-muted">{playbackCache.fileCount} {playbackCache.fileCount === 1 ? "copy" : "copies"} · {formatBytes(playbackCache.totalBytes)} of {formatBytes(playbackCache.maxBytes)} · {playbackCache.maxFiles} file limit</p> : <p className="mt-2 text-[10px] text-muted">Cache usage unavailable.</p>}</div></div><Button variant="secondary" className="mt-3 h-9 w-full text-xs" disabled={clearingPlaybackCache || !playbackCache || playbackCache.fileCount === 0} onClick={() => void clearPlaybackCache()}>{clearingPlaybackCache ? <RefreshCw className="animate-spin" size={14} /> : <Eraser size={14} />} Clear compatibility cache</Button></div>
      </div>
      <div className={`mt-5 flex items-start gap-3 rounded-[10px] border p-4 ${nativePlayback?.available ? "border-accent/25 bg-accent/5" : "border-line/60 bg-elevated/25"}`}><div className={`grid size-9 shrink-0 place-items-center rounded-[8px] ${nativePlayback?.available ? "bg-accent/10 text-accent" : "bg-elevated text-muted"}`}><MonitorPlay size={17} /></div><div><h3 className="text-xs font-semibold text-ink">Native mpv playback · {nativePlayback?.available ? "Ready" : "Unavailable"}</h3><p className="mt-1 text-[11px] leading-5 text-muted">{nativePlayback?.detail || "Checking whether mpv is available…"}</p>{nativePlayback?.version ? <p className="number-font mt-2 text-[10px] text-muted">{nativePlayback.version}</p> : null}</div></div>
      <div className="mt-5 flex items-center justify-between border-t border-line/60 pt-4"><p className="text-xs text-muted">System reduced-motion and increased-contrast preferences are also honored automatically.</p><Button onClick={() => void saveDesktop()}><Save size={15} /> Save preferences</Button></div>
    </div>
  </section>;
}

function AuthenticationProfiles() {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [profiles, setProfiles] = useState<AuthenticationProfile[]>([]);
  const [browserProfiles, setBrowserProfiles] = useState<BrowserProfileInfo[]>([]);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [hostSuffix, setHostSuffix] = useState("");
  const [browser, setBrowser] = useState<BrowserCookieSource>("none");
  const [browserProfile, setBrowserProfile] = useState("");
  const [proxyUrl, setProxyUrl] = useState("");
  const [cookieFilePath, setCookieFilePath] = useState("");
  const [cookieConfigured, setCookieConfigured] = useState(false);
  const [clearCookieFile, setClearCookieFile] = useState(false);
  const [enabled, setEnabled] = useState(true);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    void Promise.all([api.listAuthenticationProfiles(), api.discoverBrowserProfiles()])
      .then(([savedProfiles, discovered]) => { setProfiles(savedProfiles); setBrowserProfiles(discovered); })
      .catch((error) => setError(normalizeError(error)));
  }, [setError]);

  function reset() {
    setEditingId(null); setName(""); setHostSuffix(""); setBrowser("none"); setBrowserProfile("");
    setProxyUrl(""); setCookieFilePath(""); setCookieConfigured(false); setClearCookieFile(false); setEnabled(true);
  }

  function edit(profile: AuthenticationProfile) {
    setEditingId(profile.id); setName(profile.name); setHostSuffix(profile.hostSuffix);
    setBrowser(profile.browserCookieSource); setBrowserProfile(profile.browserProfile || "");
    setProxyUrl(profile.proxyUrl || ""); setCookieConfigured(profile.cookieFileConfigured);
    setCookieFilePath(""); setClearCookieFile(false); setEnabled(profile.enabled);
  }

  async function pickCookieFile() {
    const path = await openDialog({ multiple: false, directory: false, title: "Import Netscape cookies.txt", filters: [{ name: "Cookie text files", extensions: ["txt"] }] });
    if (path) { setCookieFilePath(path); setClearCookieFile(false); }
  }

  async function saveProfile() {
    setSaving(true);
    try {
      const saved = await api.saveAuthenticationProfile({ id: editingId, name, hostSuffix, browserCookieSource: browser, browserProfile: browserProfile || null, proxyUrl: proxyUrl || null, cookieFilePath: cookieFilePath || null, clearCookieFile, enabled });
      setProfiles((current) => [...current.filter((item) => item.id !== saved.id), saved].sort((a, b) => a.name.localeCompare(b.name)));
      reset(); setNotice({ message: "Site authentication profile saved.", view: "settings" });
    } catch (error) { setError(normalizeError(error)); } finally { setSaving(false); }
  }

  async function remove(profile: AuthenticationProfile) {
    try { await api.deleteAuthenticationProfile(profile.id); setProfiles((current) => current.filter((item) => item.id !== profile.id)); if (editingId === profile.id) reset(); }
    catch (error) { setError(normalizeError(error)); }
  }

  const matchingBrowserProfiles = browserProfiles.filter((profile) => profile.browser === browser);
  return <section className="mb-8">
    <div className="mb-3 flex items-center gap-2"><Cookie size={17} className="text-muted" /><h2 className="text-sm font-semibold">Per-site authentication</h2></div>
    <div className="grid gap-5 xl:grid-cols-[minmax(360px,.8fr)_minmax(440px,1.2fr)]">
      <div className="panel-shadow border border-line/60 bg-panel/75 p-5">
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-1 2xl:grid-cols-2">
          <Field label="Profile name"><Input value={name} onChange={(event) => setName(event.target.value)} placeholder="Reddit account" /></Field>
          <Field label="Site hostname" hint="Matches the hostname and its subdomains."><Input value={hostSuffix} onChange={(event) => setHostSuffix(event.target.value)} placeholder="reddit.com" spellCheck={false} /></Field>
          <Field label="Browser cookies"><Select value={browser} onChange={(event) => { setBrowser(event.target.value as BrowserCookieSource); setBrowserProfile(""); }}>{browserOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</Select></Field>
          <Field label="Discovered browser profile"><Select disabled={browser === "none"} value={browserProfile} onChange={(event) => setBrowserProfile(event.target.value)}><option value="">Automatic/default</option>{matchingBrowserProfiles.map((profile) => <option key={profile.path} value={profile.name}>{profile.name}</option>)}</Select></Field>
          <Field label="Profile proxy"><Input value={proxyUrl} onChange={(event) => setProxyUrl(event.target.value)} placeholder="Optional SOCKS or HTTP proxy" spellCheck={false} /></Field>
          <Field label="Managed cookies.txt" hint="Copied into private app storage with owner-only permissions on Linux."><div className="flex gap-2"><Input readOnly value={cookieFilePath || (cookieConfigured && !clearCookieFile ? "Stored securely" : "")} placeholder="No managed cookie file" /><Button variant="secondary" className="px-3" onClick={() => void pickCookieFile()}>Choose</Button></div>{cookieConfigured ? <label className="mt-2 flex items-center gap-2 text-xs text-muted"><input type="checkbox" checked={clearCookieFile} onChange={(event) => setClearCookieFile(event.target.checked)} /> Remove stored cookie file</label> : null}</Field>
        </div>
        <div className="mt-4 flex items-center justify-between border-t border-line/60 pt-4"><label className="flex items-center gap-2 text-xs text-muted"><input type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} /> Enabled</label><div className="flex gap-2">{editingId ? <Button variant="secondary" onClick={reset}>Cancel</Button> : null}<Button disabled={saving || !name.trim() || !hostSuffix.trim()} onClick={() => void saveProfile()}><Save size={15} />{saving ? "Saving" : editingId ? "Update profile" : "Add profile"}</Button></div></div>
      </div>
      <div className="panel-shadow divide-y divide-line/60 overflow-hidden border border-line/60 bg-panel/70">
        {profiles.map((profile) => <article key={profile.id} className="flex items-center gap-4 p-4"><div className={`size-2 shrink-0 rounded-full ${profile.enabled ? "bg-emerald-400" : "bg-muted"}`} /><div className="min-w-0 flex-1"><div className="flex items-center gap-2"><h3 className="truncate text-sm font-semibold">{profile.name}</h3>{profile.cookieFileConfigured ? <span className="bg-accent/10 px-1.5 py-0.5 text-[9px] font-semibold text-accent">COOKIE FILE</span> : null}</div><p className="mt-1 truncate text-xs text-muted">*.{profile.hostSuffix} · {profile.browserCookieSource === "none" ? "no browser" : profile.browserCookieSource}{profile.browserProfile ? ` / ${profile.browserProfile}` : ""}{profile.proxyUrl ? " · proxy" : ""}</p></div><Button variant="secondary" className="h-8 px-2.5 text-xs" onClick={() => edit(profile)}>Edit</Button><Button variant="secondary" className="h-8 px-2.5 text-red-400" onClick={() => void remove(profile)}><Trash2 size={14} /></Button></article>)}
        {profiles.length === 0 ? <div className="p-8 text-center text-sm text-muted">No per-site profiles. Global access settings will be used.</div> : null}
      </div>
    </div>
    <p className="mt-3 text-xs leading-5 text-muted">The longest matching hostname wins. Cookie contents and managed paths never cross the Tauri boundary or enter diagnostic exports.</p>
  </section>;
}

export function PresetsPage() {
  const setError = useAppStore((state) => state.setError);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [rules, setRules] = useState<SitePresetRule[]>([]);
  const [name, setName] = useState("");
  const [intent, setIntent] = useState<PresetIntent>({
    quality: "best",
    container: "mp4",
    videoCodecPreference: "any",
    audioQuality: "best",
    embedThumbnail: false,
    embedMetadata: true,
    outputConflictPolicy: "skip",
    sponsorblockCategories: [],
    filenameTemplate: "%(title).180B [%(id)s].%(ext)s",
  });
  const [ruleName, setRuleName] = useState("");
  const [hostSuffix, setHostSuffix] = useState("");
  const [pathPrefix, setPathPrefix] = useState("");
  const [rulePresetId, setRulePresetId] = useState("");

  useEffect(() => {
    void Promise.all([api.listPresets(), api.listSitePresetRules()])
      .then(([savedPresets, savedRules]) => {
        setPresets(savedPresets);
        setRules(savedRules);
        setRulePresetId((current) => current || savedPresets[0]?.id || "");
      })
      .catch((error) => setError(normalizeError(error)));
  }, [setError]);

  async function createPreset() {
    try {
      const preset = await api.savePreset({ name, intent });
      setPresets((items) => [...items, preset].sort((a, b) => a.name.localeCompare(b.name)));
      setRulePresetId((current) => current || preset.id);
      setName("");
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  async function createRule() {
    try {
      const rule = await api.saveSitePresetRule({
        name: ruleName,
        hostSuffix,
        pathPrefix: pathPrefix || null,
        presetId: rulePresetId,
        enabled: true,
      });
      setRules((items) => [...items, rule].sort((a, b) => a.name.localeCompare(b.name)));
      setRuleName("");
      setHostSuffix("");
      setPathPrefix("");
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  return (
    <Page title="Presets" description="Reusable download intentions stored locally in SQLite.">
      <section className="panel-shadow rounded-[12px] border border-line/60 bg-panel/75 p-5">
        <div className="mb-5 flex items-center gap-3"><div className="grid size-9 place-items-center rounded-[8px] bg-accent/10 text-accent"><Settings2 size={17} /></div><div><h2 className="text-[15px] font-semibold tracking-[-0.02em]">New preset</h2><p className="mt-0.5 text-[11px] text-muted">Save an intent, not a command string.</p></div></div>
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
        <div className="mt-4 border-t border-line pt-4">
          <div className="mb-2 text-xs font-medium text-ink">SponsorBlock categories</div>
          <div className="flex flex-wrap gap-x-5 gap-y-2 text-xs text-muted">
            {sponsorCategoryOptions.map((category) => (
              <label key={category.value} className="flex items-center gap-2">
                <input type="checkbox" checked={intent.sponsorblockCategories.includes(category.value)} onChange={(event) => setIntent({ ...intent, sponsorblockCategories: event.target.checked ? [...intent.sponsorblockCategories, category.value] : intent.sponsorblockCategories.filter((value) => value !== category.value) })} />
                {category.label}
              </label>
            ))}
          </div>
        </div>
        <Field label="Filename template" hint="Optional safe filename pattern applied with this preset.">
          <Input className="mt-4" value={intent.filenameTemplate ?? ""} onChange={(event) => setIntent({ ...intent, filenameTemplate: event.target.value })} spellCheck={false} />
        </Field>
      </section>
      <div className="panel-shadow mt-5 divide-y divide-line/65 overflow-hidden rounded-[11px] border border-line/60 bg-panel/70">
        {presets.map((preset) => (
          <article key={preset.id} className="flex items-center gap-4 p-4 transition-colors duration-200 hover:bg-elevated/25">
            <div className="grid size-9 place-items-center rounded-[8px] bg-elevated text-muted"><Settings2 size={16} /></div>
            <div className="min-w-0 flex-1"><h3 className="text-sm font-semibold">{preset.name}</h3><p className="mt-1 text-xs text-muted">{preset.intent.quality === "audio" ? "Audio only" : preset.intent.quality === "best" ? "Best quality" : `${preset.intent.quality}p`} · {preset.intent.container.toUpperCase()} · {preset.intent.videoCodecPreference.toUpperCase()}</p></div>
            <Button variant="danger" className="h-8 px-2.5" onClick={() => void api.deletePreset(preset.id).then(() => { setPresets((items) => items.filter((item) => item.id !== preset.id)); setRules((items) => items.filter((rule) => rule.presetId !== preset.id)); }).catch((error) => setError(normalizeError(error)))}><Trash2 size={14} /> Delete</Button>
          </article>
        ))}
        {presets.length === 0 ? <p className="p-5 text-sm text-muted">No presets saved yet.</p> : null}
      </div>
      <section className="panel-shadow mt-5 rounded-[12px] border border-line/60 bg-panel/75 p-5">
        <div className="mb-5 flex items-center gap-3"><div className="grid size-9 place-items-center rounded-[8px] bg-accent/10 text-accent"><Globe2 size={17} /></div><div><h2 className="text-[15px] font-semibold tracking-[-0.02em]">Automatic site rules</h2><p className="mt-1 text-[11px] text-muted">Apply a saved preset when a URL host and optional path prefix match. Subdomains match automatically.</p></div></div>
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <Field label="Rule name"><Input value={ruleName} onChange={(event) => setRuleName(event.target.value)} placeholder="YouTube compatibility" /></Field>
          <Field label="Host"><Input value={hostSuffix} onChange={(event) => setHostSuffix(event.target.value)} placeholder="youtube.com" spellCheck={false} /></Field>
          <Field label="Path prefix" hint="Optional, such as /shorts/"><Input value={pathPrefix} onChange={(event) => setPathPrefix(event.target.value)} placeholder="/shorts/" spellCheck={false} /></Field>
          <Field label="Preset"><Select value={rulePresetId} onChange={(event) => setRulePresetId(event.target.value)}><option value="">Choose preset</option>{presets.map((preset) => <option key={preset.id} value={preset.id}>{preset.name}</option>)}</Select></Field>
        </div>
        <div className="mt-4 flex justify-end"><Button disabled={!ruleName.trim() || !hostSuffix.trim() || !rulePresetId} onClick={() => void createRule()}>Save site rule</Button></div>
      </section>
      <div className="panel-shadow mt-5 divide-y divide-line/65 overflow-hidden rounded-[11px] border border-line/60 bg-panel/70">
        {rules.map((rule) => (
          <article key={rule.id} className="flex items-center gap-4 p-4 transition-colors duration-200 hover:bg-elevated/25">
            <div className="grid size-9 place-items-center rounded-[8px] bg-elevated text-muted"><Globe2 size={16} /></div>
            <div className="min-w-0 flex-1"><h3 className="text-sm font-semibold">{rule.name}{!rule.enabled ? <span className="ml-2 text-xs font-normal text-muted">Disabled</span> : null}</h3><p className="mt-1 text-xs text-muted">*.{rule.hostSuffix}{rule.pathPrefix ?? ""} → {presets.find((preset) => preset.id === rule.presetId)?.name ?? "Deleted preset"}</p></div>
            <Button variant="secondary" className="h-8 px-2.5" onClick={() => void api.saveSitePresetRule({ ...rule, enabled: !rule.enabled }).then((updated) => setRules((items) => items.map((item) => item.id === updated.id ? updated : item))).catch((error) => setError(normalizeError(error)))}>{rule.enabled ? "Disable" : "Enable"}</Button>
            <Button variant="danger" className="h-8 px-2.5" onClick={() => void api.deleteSitePresetRule(rule.id).then(() => setRules((items) => items.filter((item) => item.id !== rule.id))).catch((error) => setError(normalizeError(error)))}><Trash2 size={14} /> Delete</Button>
          </article>
        ))}
        {rules.length === 0 ? <p className="p-5 text-sm text-muted">No automatic site rules configured.</p> : null}
      </div>
    </Page>
  );
}
