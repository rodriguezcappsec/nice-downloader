import { open } from "@tauri-apps/plugin-dialog";
import {
  ChevronRight,
  Clock3,
  FolderOpen,
  Pause,
  Play,
  Plus,
  RefreshCw,
  Rss,
  Trash2,
  X,
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
  const [createOpen, setCreateOpen] = useState(false);
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
        if (savedSubscriptions.length === 0) setCreateOpen(true);
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
      setCreateOpen(false);
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
      setChecking((current) => current === subscription.id ? null : current);
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

  const enabledCount = subscriptions.filter((subscription) => subscription.enabled).length;
  const attentionCount = subscriptions.filter((subscription) => Boolean(subscription.lastError)).length;

  return (
    <Page
      title="Subscriptions"
      description="Watch channels and playlists locally, then queue only media you do not already have."
      actions={<Button variant="secondary" disabled={loading} onClick={() => void api.listSubscriptions().then(setSubscriptions).catch((error) => setError(normalizeError(error)))}><RefreshCw className={loading ? "animate-spin" : ""} size={15} /> Refresh</Button>}
    >
      <section className="ingest-card relative overflow-hidden rounded-media border border-line/45 px-6 py-7 sm:px-8" aria-labelledby="subscription-scheduler-title">
        <div className="pointer-events-none absolute -right-10 -top-20 size-72 rounded-full bg-accent/[0.09] blur-3xl" />
        <div className="relative grid gap-7 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
          <div>
            <p className="text-[10px] font-semibold tracking-[0.1em] text-accent">Local scheduler</p>
            <h2 id="subscription-scheduler-title" className="mt-3 max-w-2xl text-[30px] font-semibold leading-[1.05] tracking-[-0.05em] text-balance">New releases arrive without turning your library into a duplicate pile.</h2>
            <p className="mt-3 max-w-2xl text-xs leading-5 text-muted">Checks run in the background through saved presets. Your download archive remains authoritative even when a History record is removed.</p>
          </div>
          <div className="flex flex-wrap items-end gap-2">
            <dl className="mr-2 grid grid-cols-2 gap-px overflow-hidden rounded-panel border border-line/50 bg-line/50">
              <SchedulerMetric label="Active" value={enabledCount} />
              <SchedulerMetric label="Attention" value={attentionCount} warning={attentionCount > 0} />
            </dl>
            <Button className="h-11" onClick={() => setCreateOpen((open) => !open)} aria-expanded={createOpen} aria-controls="new-subscription-panel">{createOpen ? <X size={16} /> : <Plus size={16} />} {createOpen ? "Close form" : "Add subscription"}</Button>
          </div>
        </div>
      </section>

      {createOpen ? (
        <section id="new-subscription-panel" className="panel-shadow mt-4 rounded-media border border-line/60 bg-panel/75 p-5 sm:p-6" aria-labelledby="new-subscription-title">
          <div className="mb-5 flex items-start gap-3">
            <div className="grid size-10 shrink-0 place-items-center rounded-control bg-accent/10 text-accent"><Plus size={17} /></div>
            <div><h2 id="new-subscription-title" className="text-[16px] font-semibold tracking-[-0.025em]">Add a channel or playlist</h2><p className="mt-1 text-[11px] leading-5 text-muted">Choose where new entries go and how much each background check may queue.</p></div>
          </div>
          {presets.length === 0 ? (
            <div className="flex flex-wrap items-center justify-between gap-3 rounded-panel border border-amber-400/25 bg-amber-400/[0.06] p-4 text-[13px]">
              <span>Create a preset before subscribing to a channel or playlist.</span>
              <Button variant="secondary" onClick={() => setView("presets")}>Open presets <ChevronRight size={14} /></Button>
            </div>
          ) : (
            <div className="grid gap-5 xl:grid-cols-[minmax(0,1.35fr)_minmax(280px,0.65fr)]">
              <div className="grid content-start gap-4 rounded-panel bg-canvas/35 p-4 sm:grid-cols-2">
                <Field label="Name"><Input value={name} onChange={(event) => setName(event.target.value)} placeholder="Documentary channel" /></Field>
                <Field label="Channel or playlist URL"><Input value={url} onChange={(event) => setUrl(event.target.value)} placeholder="https://..." spellCheck={false} /></Field>
                <Field label="Preset"><Select value={presetId} onChange={(event) => setPresetId(event.target.value)}>{presets.map((preset) => <option key={preset.id} value={preset.id}>{preset.name}</option>)}</Select></Field>
                <Field label="Destination"><div className="flex gap-2"><Input value={destination} onChange={(event) => setDestination(event.target.value)} spellCheck={false} /><Button variant="secondary" className="shrink-0 px-3" onClick={() => void chooseDestination()} aria-label="Choose destination"><FolderOpen size={16} /></Button></div></Field>
              </div>
              <div className="rounded-panel border border-line/45 bg-elevated/25 p-4">
                <p className="mb-4 text-[10px] font-semibold tracking-[0.08em] text-muted">Check behavior</p>
                <div className="grid gap-4">
                  <Field label="Check interval"><Select value={intervalMinutes} onChange={(event) => setIntervalMinutes(Number(event.target.value))}><option value={15}>Every 15 minutes</option><option value={60}>Every hour</option><option value={360}>Every 6 hours</option><option value={720}>Every 12 hours</option><option value={1440}>Daily</option><option value={10080}>Weekly</option></Select></Field>
                  <Field label="Recent entries per check" hint="Bounded to avoid unexpectedly queueing an entire channel."><Select value={maxEntries} onChange={(event) => setMaxEntries(Number(event.target.value))}>{[1, 3, 5, 10, 15, 20].map((value) => <option key={value} value={value}>{value}</option>)}</Select></Field>
                </div>
                <Button className="mt-5 h-10 w-full" disabled={saving || !name.trim() || !url.trim() || !presetId || !destination} onClick={() => void createSubscription()}><Rss size={15} /> {saving ? "Saving…" : "Create subscription"}</Button>
              </div>
            </div>
          )}
        </section>
      ) : null}

      {result ? <div className="mt-4 rounded-control border border-accent/25 bg-accent/[0.06] p-3 text-[12px] text-accent" role="status">{result}</div> : null}

      <section className="mt-6" aria-labelledby="saved-subscriptions-title">
        <div className="mb-3 flex items-end justify-between gap-3">
          <div><p className="text-[10px] font-semibold tracking-[0.08em] text-accent">Watch list</p><h2 id="saved-subscriptions-title" className="mt-1 text-[18px] font-semibold tracking-[-0.03em]">Saved subscriptions</h2></div>
          <p className="number-font text-[10px] text-muted">{subscriptions.length} total</p>
        </div>
        {loading ? <div className="flex min-h-48 items-center justify-center gap-2 rounded-media bg-panel/45 text-sm text-muted" role="status"><RefreshCw className="animate-spin text-accent" size={16} /> Loading subscriptions…</div> : subscriptions.length === 0 ? (
          <EmptyState icon={<Rss size={20} />} title="No subscriptions yet" description="Add a channel or playlist. The scheduler will check it without keeping this page open." />
        ) : (
          <div className="grid gap-2.5">
            {subscriptions.map((subscription) => (
              <article key={subscription.id} className="group grid gap-4 rounded-panel border border-line/45 bg-panel/60 p-4 transition duration-200 hover:-translate-y-px hover:border-line/80 hover:bg-panel/85 hover:shadow-control lg:grid-cols-[44px_minmax(0,1fr)_auto] lg:items-center">
                <div className={`grid size-11 place-items-center rounded-control border ${subscription.enabled ? "border-accent/15 bg-accent/10 text-accent" : "border-line/50 bg-elevated text-muted"}`}><Rss size={18} /></div>
                <div className="min-w-0">
                  <div className="flex min-w-0 flex-wrap items-center gap-2"><h3 className="truncate text-[13px] font-semibold tracking-[-0.015em]">{subscription.name}</h3><span className={`rounded-[6px] px-2 py-1 text-[9px] font-semibold ${subscription.enabled ? "bg-emerald-400/[0.08] text-emerald-300" : "bg-elevated text-muted"}`}>{subscription.enabled ? "Active" : "Paused"}</span></div>
                  <p className="mt-1.5 truncate text-[11px] text-muted" title={subscription.url}>{subscription.url}</p>
                  <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-[10px] text-muted"><span className="flex items-center gap-1"><Clock3 size={11} /> Next {formatRelative(subscription.nextCheckAt)}</span><span>Every {formatInterval(subscription.checkIntervalMinutes)}</span><span>Up to {subscription.maxEntries} entries</span></div>
                  {subscription.lastError ? <p className="mt-2 line-clamp-2 text-[10px] leading-4 text-red-300">{subscription.lastError}</p> : null}
                </div>
                <div className="flex flex-wrap items-center gap-1.5">
                  <Button variant="secondary" className="h-9 px-3 text-xs" disabled={!subscription.enabled || checking !== null} onClick={() => void checkNow(subscription)}><RefreshCw className={checking === subscription.id ? "animate-spin" : ""} size={14} /> Check now</Button>
                  <Button variant="ghost" className="h-9 px-3 text-xs" onClick={() => void toggleSubscription(subscription)}>{subscription.enabled ? <Pause size={14} /> : <Play size={14} />}{subscription.enabled ? "Pause" : "Enable"}</Button>
                  <Button variant="ghost" className="h-9 px-2.5 text-red-300 hover:bg-red-400/10 hover:text-red-200" disabled={checking === subscription.id} aria-label={`Delete ${subscription.name}`} onClick={() => void api.deleteSubscription(subscription.id).then(() => setSubscriptions((items) => items.filter((item) => item.id !== subscription.id))).catch((error) => setError(normalizeError(error)))}><Trash2 size={14} /></Button>
                </div>
              </article>
            ))}
          </div>
        )}
      </section>
    </Page>
  );
}

function SchedulerMetric({ label, value, warning = false }: { label: string; value: number; warning?: boolean }) {
  return <div className="min-w-24 bg-panel/80 px-4 py-3 text-right"><dt className="text-[9px] font-semibold tracking-[0.08em] text-muted">{label}</dt><dd className={`number-font mt-1 text-lg font-semibold ${warning ? "text-amber-300" : "text-ink"}`}>{value}</dd></div>;
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
