import {
  CheckCircle2,
  CircleX,
  Cookie,
  Network,
  RefreshCw,
  Save,
  Settings2,
  ShieldCheck,
  Wrench,
} from "lucide-react";
import { useState } from "react";
import { Page } from "../../components/Page";
import { Button, Field, Input, Select } from "../../components/ui";
import { useAppStore } from "../../stores/app-store";
import type { BrowserCookieSource } from "../../types";

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
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  async function save() {
    setSaving(true);
    setSaved(false);
    setSaved(await savePreferences());
    setSaving(false);
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
        </div>
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
        <p className="mt-3 text-xs leading-5 text-muted">Executables are discovered from PATH. Managed sidecars and automatic updates remain planned work.</p>
      </section>
    </Page>
  );
}

export function PresetsPage() {
  return (
    <Page title="Presets" description="Reusable download intentions will arrive in a later milestone.">
      <div className="grid min-h-64 place-items-center border border-dashed border-line text-center">
        <div>
          <Settings2 className="mx-auto text-muted" size={24} />
          <h2 className="mt-3 text-sm font-semibold">Presets are coming next</h2>
          <p className="mt-1 text-sm text-muted">The intent model is already designed for saved presets.</p>
        </div>
      </div>
    </Page>
  );
}
