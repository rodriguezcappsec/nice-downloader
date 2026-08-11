import { open } from "@tauri-apps/plugin-dialog";
import {
  Clock3,
  FolderOpen,
  Pause,
  Play,
  Plus,
  RefreshCw,
  Rss,
  Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import { Page } from "../../components/Page";
import { Button, EmptyState, Field, Input, Select } from "../../components/ui";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type { Preset, Subscription } from "../../types";

export function SubscriptionsPage() {
  const defaultDestination = useAppStore((state) => state.defaultDestination);
  const setError = useAppStore((state) => state.setError);
  const setView = useAppStore((state) => state.setView);
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [checking, setChecking] = useState<string | null>(null);
  const [result, setResult] = useState("");
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [presetId, setPresetId] = useState("");
  const [destination, setDestination] = useState(defaultDestination);
  const [intervalMinutes, setIntervalMinutes] = useState(360);
  const [maxEntries, setMaxEntries] = useState(10);

  useEffect(() => {
    void Promise.all([api.listSubscriptions(), api.listPresets()])
      .then(([savedSubscriptions, savedPresets]) => {
        setSubscriptions(savedSubscriptions);
        setPresets(savedPresets);
        setPresetId((current) => current || savedPresets[0]?.id || "");
      })
      .catch((error) => setError(normalizeError(error)))
      .finally(() => setLoading(false));
  }, [setError]);

  useEffect(() => {
    if (!destination && defaultDestination) setDestination(defaultDestination);
  }, [defaultDestination, destination]);

  async function chooseDestination() {
    const selected = await open({ directory: true, multiple: false, defaultPath: destination || undefined });
    if (typeof selected === "string") setDestination(selected);
  }

  async function createSubscription() {
    setSaving(true);
    setResult("");
    try {
      const subscription = await api.saveSubscription({
        name,
        url,
        presetId,
        destination,
        checkIntervalMinutes: intervalMinutes,
        maxEntries,
        enabled: true,
      });
      setSubscriptions((items) => [...items, subscription].sort((a, b) => a.name.localeCompare(b.name)));
      setName("");
      setUrl("");
      setResult("Subscription saved. Its first check is due now.");
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setSaving(false);
    }
  }

  async function checkNow(subscription: Subscription) {
    setChecking(subscription.id);
    setResult("");
    try {
      const check = await api.checkSubscriptionNow(subscription.id);
      setSubscriptions((items) => items.map((item) => item.id === check.subscription.id ? check.subscription : item));
      setResult(`${check.queuedCount} queued · ${check.skippedCount} already downloaded or unavailable · ${check.discoveredCount} inspected`);
    } catch (error) {
      setError(normalizeError(error));
      void api.listSubscriptions().then(setSubscriptions).catch(() => undefined);
    } finally {
      setChecking(null);
    }
  }

  async function toggleSubscription(subscription: Subscription) {
    try {
      const updated = await api.saveSubscription({ ...subscription, enabled: !subscription.enabled });
      setSubscriptions((items) => items.map((item) => item.id === updated.id ? updated : item));
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  return (
    <Page
      title="Subscriptions"
      description="Periodically inspect channels and playlists, then queue only entries that are not in the download archive."
      actions={<Button variant="secondary" onClick={() => void api.listSubscriptions().then(setSubscriptions).catch((error) => setError(normalizeError(error)))}><RefreshCw size={15} /> Refresh</Button>}
    >
      <section className="panel-shadow rounded-[12px] border border-line/60 bg-panel/75 p-5">
        <div className="mb-5 flex items-center gap-3">
          <div className="grid size-9 place-items-center rounded-[8px] bg-accent/10 text-accent"><Plus size={17} /></div>
          <div><h2 className="text-[15px] font-semibold tracking-[-0.02em]">New subscription</h2><p className="mt-0.5 text-[11px] text-muted">Checks run locally in Rust and obey the saved download preset.</p></div>
        </div>
        {presets.length === 0 ? (
          <div className="flex items-center justify-between rounded-[9px] border border-amber-500/25 bg-amber-500/[0.06] p-4 text-[13px]">
            <span>Create a preset before subscribing to a channel or playlist.</span>
            <Button variant="secondary" onClick={() => setView("presets")}>Open presets</Button>
          </div>
        ) : (
          <>
            <div className="grid gap-4 lg:grid-cols-2 xl:grid-cols-4">
              <Field label="Name"><Input value={name} onChange={(event) => setName(event.target.value)} placeholder="Documentary channel" /></Field>
              <Field label="Channel or playlist URL"><Input value={url} onChange={(event) => setUrl(event.target.value)} placeholder="https://..." spellCheck={false} /></Field>
              <Field label="Preset"><Select value={presetId} onChange={(event) => setPresetId(event.target.value)}>{presets.map((preset) => <option key={preset.id} value={preset.id}>{preset.name}</option>)}</Select></Field>
              <Field label="Destination"><div className="flex gap-2"><Input value={destination} onChange={(event) => setDestination(event.target.value)} spellCheck={false} /><Button variant="secondary" className="px-3" onClick={() => void chooseDestination()} aria-label="Choose destination"><FolderOpen size={16} /></Button></div></Field>
              <Field label="Check interval"><Select value={intervalMinutes} onChange={(event) => setIntervalMinutes(Number(event.target.value))}><option value={15}>Every 15 minutes</option><option value={60}>Every hour</option><option value={360}>Every 6 hours</option><option value={720}>Every 12 hours</option><option value={1440}>Daily</option><option value={10080}>Weekly</option></Select></Field>
              <Field label="Recent entries per check" hint="Bounded to avoid unexpectedly queueing an entire channel."><Select value={maxEntries} onChange={(event) => setMaxEntries(Number(event.target.value))}>{[1, 3, 5, 10, 15, 20].map((value) => <option key={value} value={value}>{value}</option>)}</Select></Field>
            </div>
            <div className="mt-5 flex items-center justify-between border-t border-line/60 pt-4"><p className="text-[11px] text-muted">The archive remains authoritative even if a History record is removed.</p><Button disabled={saving || !name.trim() || !url.trim() || !presetId || !destination} onClick={() => void createSubscription()}><Rss size={15} /> {saving ? "Saving" : "Create subscription"}</Button></div>
          </>
        )}
      </section>

      {result ? <div className="mt-5 rounded-[9px] border border-accent/25 bg-accent/[0.06] p-3 text-[12px] text-accent">{result}</div> : null}

      <div className="mt-5">
        {loading ? <div className="p-8 text-center text-sm text-muted">Loading subscriptions…</div> : subscriptions.length === 0 ? (
          <EmptyState icon={<Rss size={20} />} title="No subscriptions yet" description="Add a channel or playlist above. The scheduler will check it without keeping this page open." />
        ) : (
          <div className="panel-shadow divide-y divide-line/65 overflow-hidden rounded-[11px] border border-line/60 bg-panel/70">
            {subscriptions.map((subscription) => (
              <article key={subscription.id} className="grid grid-cols-[40px_minmax(0,1fr)_auto] items-center gap-4 p-4 transition-colors hover:bg-elevated/25">
                <div className={`grid size-10 place-items-center rounded-[9px] ${subscription.enabled ? "bg-accent/10 text-accent" : "bg-elevated text-muted"}`}><Rss size={17} /></div>
                <div className="min-w-0"><div className="flex items-center gap-2"><h3 className="truncate text-[13px] font-semibold">{subscription.name}</h3>{!subscription.enabled ? <span className="rounded-[4px] bg-elevated px-1.5 py-0.5 text-[9px] text-muted">Paused</span> : null}</div><p className="mt-1 truncate text-[11px] text-muted">{subscription.url}</p><div className="mt-2 flex flex-wrap gap-3 text-[10px] text-muted"><span className="flex items-center gap-1"><Clock3 size={11} /> Next {formatRelative(subscription.nextCheckAt)}</span><span>Every {formatInterval(subscription.checkIntervalMinutes)}</span><span>Up to {subscription.maxEntries} entries</span>{subscription.lastError ? <span className="text-red-400">{subscription.lastError}</span> : null}</div></div>
                <div className="flex items-center gap-1"><Button variant="ghost" className="h-8 px-2.5" disabled={!subscription.enabled || checking === subscription.id} onClick={() => void checkNow(subscription)}><RefreshCw className={checking === subscription.id ? "animate-spin" : ""} size={14} /> Check now</Button><Button variant="ghost" className="h-8 px-2.5" onClick={() => void toggleSubscription(subscription)}>{subscription.enabled ? <Pause size={14} /> : <Play size={14} />}{subscription.enabled ? "Pause" : "Enable"}</Button><Button variant="danger" className="h-8 px-2" aria-label={`Delete ${subscription.name}`} onClick={() => void api.deleteSubscription(subscription.id).then(() => setSubscriptions((items) => items.filter((item) => item.id !== subscription.id))).catch((error) => setError(normalizeError(error)))}><Trash2 size={14} /></Button></div>
              </article>
            ))}
          </div>
        )}
      </div>
    </Page>
  );
}

function formatInterval(minutes: number): string {
  if (minutes % 10_080 === 0) return `${minutes / 10_080} week`;
  if (minutes % 1_440 === 0) return `${minutes / 1_440} day`;
  if (minutes % 60 === 0) return `${minutes / 60} hr`;
  return `${minutes} min`;
}

function formatRelative(value: string): string {
  const milliseconds = new Date(value).getTime() - Date.now();
  if (!Number.isFinite(milliseconds) || milliseconds <= 0) return "now";
  const minutes = Math.ceil(milliseconds / 60_000);
  if (minutes < 60) return `in ${minutes} min`;
  const hours = Math.ceil(minutes / 60);
  if (hours < 24) return `in ${hours} hr`;
  return `in ${Math.ceil(hours / 24)} d`;
}
