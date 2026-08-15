import { Check, Copy, LoaderCircle, Search, Share2, ShieldCheck, Square } from "lucide-react";
import { useEffect, useState } from "react";
import { Button, Input, Select } from "../../components/ui";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type { LibraryItem, LocalShareSession } from "../../types";

export function SharingPage() {
  const setError = useAppStore((state) => state.setError);
  const [session, setSession] = useState<LocalShareSession | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [search, setSearch] = useState("");
  const [items, setItems] = useState<LibraryItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [expiresMinutes, setExpiresMinutes] = useState("30");
  const [copied, setCopied] = useState(false);

  useEffect(() => { void api.getLocalShareStatus().then(setSession).catch((error) => setError(normalizeError(error))); }, [setError]);
  useEffect(() => {
    let disposed = false;
    const timer = window.setTimeout(() => {
      setLoading(true);
      void api.queryLibraryItems({ page: 1, pageSize: 60, search: search.trim() || null, sourceId: null, folderSourceId: null, folderPath: null, folderScope: "tree", mediaKind: null, resolution: null, container: null, duration: null, dateAdded: null, datePublished: null, availability: "available", tags: [], creators: [], series: [], sites: [], rating: null, collectionId: null, favoriteOnly: false, sort: "recent" })
        .then((result) => !disposed && setItems(result.items)).catch((error) => !disposed && setError(normalizeError(error))).finally(() => !disposed && setLoading(false));
    }, 250);
    return () => { disposed = true; window.clearTimeout(timer); };
  }, [search, setError]);

  function toggle(id: string) {
    setSelected((current) => current.includes(id) ? current.filter((value) => value !== id) : current.length < 50 ? [...current, id] : current);
  }
  async function start() {
    if (!selected.length) return;
    setBusy(true);
    try { setSession(await api.startLocalShare(selected, Number(expiresMinutes))); }
    catch (error) { setError(normalizeError(error)); }
    finally { setBusy(false); }
  }
  async function stop() {
    if (!session) return;
    setBusy(true);
    try { await api.stopLocalShare(session.id); setSession(null); }
    catch (error) { setError(normalizeError(error)); }
    finally { setBusy(false); }
  }
  async function copyUrl() {
    if (!session) return;
    try { await navigator.clipboard.writeText(session.url); setCopied(true); window.setTimeout(() => setCopied(false), 2000); }
    catch { setError({ code: "clipboard_unavailable", message: "Could not copy the link. Select and copy it manually." }); }
  }

  return <div className="mx-auto w-full max-w-[1400px] px-8 pb-12 pt-8 xl:px-10"><header className="mb-7"><p className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent/90"><span className="h-px w-5 bg-accent/70" /> Local network</p><h1 className="text-[28px] font-semibold tracking-[-0.035em]">Temporary sharing</h1><p className="mt-2 max-w-3xl text-[13px] leading-5 text-muted">Share selected Library media with devices on your current local network. The random link expires automatically, nothing is uploaded, and the server stops when Nice Downloader exits.</p></header>
    {session ? <section className="panel-shadow border border-accent/25 bg-panel/80 p-6"><div className="flex flex-wrap items-start gap-4"><div className="grid size-11 place-items-center rounded-[10px] bg-accent/10 text-accent"><Share2 size={20} /></div><div className="min-w-0 flex-1"><p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-accent">Sharing is active</p><h2 className="mt-1 text-lg font-semibold">{session.itemCount} item{session.itemCount === 1 ? "" : "s"} available until {new Date(session.expiresAt).toLocaleTimeString()}</h2><div className="mt-4 flex gap-2"><Input className="number-font min-w-0" readOnly value={session.url} onFocus={(event) => event.currentTarget.select()} /><Button variant="secondary" onClick={() => void copyUrl()}>{copied ? <Check size={15} /> : <Copy size={15} />} {copied ? "Copied" : "Copy link"}</Button></div>{!session.reachableOnLan ? <p className="mt-3 border-l-2 border-amber-400 bg-amber-400/5 px-3 py-2 text-xs text-amber-200">No LAN address was detected. The shown loopback link works only on this computer; connect to a network and restart the session.</p> : <p className="mt-3 text-xs text-muted">Open this link on a device connected to the same network. A firewall may ask permission for incoming connections.</p>}</div><Button variant="secondary" disabled={busy} onClick={() => void stop()}>{busy ? <LoaderCircle className="animate-spin" size={14} /> : <Square size={14} />} Stop sharing</Button></div></section> : <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_330px]"><section className="panel-shadow border border-line/60 bg-panel/75 p-5"><div className="flex flex-wrap items-center gap-3"><div><h2 className="text-sm font-semibold">Choose media</h2><p className="mt-1 text-[10px] text-muted">{selected.length}/50 selected · only the selected catalog IDs are authorized</p></div><div className="relative ml-auto w-full max-w-sm"><Search className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted" size={14} /><Input className="pl-9" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search available media" /></div></div>{loading ? <p className="flex items-center gap-2 py-12 text-sm text-muted"><LoaderCircle className="animate-spin" size={16} /> Loading media…</p> : <div className="mt-4 grid gap-2 sm:grid-cols-2 lg:grid-cols-3">{items.map((item) => { const checked = selected.includes(item.id); return <button key={item.id} className={`flex min-w-0 items-center gap-3 rounded-[8px] border p-3 text-left ${checked ? "border-accent/40 bg-accent/10" : "border-line/55 bg-elevated/25 hover:bg-elevated/60"}`} onClick={() => toggle(item.id)}><span className={`grid size-5 shrink-0 place-items-center rounded-[4px] border ${checked ? "border-accent bg-accent text-[#071419]" : "border-line"}`}>{checked ? <Check size={13} /> : null}</span><span className="min-w-0"><strong className="block truncate text-[11px]">{item.title}</strong><span className="mt-1 block text-[9px] text-muted">{item.mediaKind === "image" ? "Image" : "Video"}</span></span></button>; })}</div>}</section><aside className="panel-shadow h-fit border border-line/60 bg-panel/75 p-5"><div className="grid size-10 place-items-center rounded-[9px] bg-accent/10 text-accent"><ShieldCheck size={19} /></div><h2 className="mt-4 text-sm font-semibold">Session limits</h2><label className="mt-4 block text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Expires after<Select className="mt-1" value={expiresMinutes} onChange={(event) => setExpiresMinutes(event.target.value)}><option value="5">5 minutes</option><option value="15">15 minutes</option><option value="30">30 minutes</option><option value="60">1 hour</option><option value="120">2 hours</option></Select></label><ul className="mt-5 space-y-2 text-[11px] leading-5 text-muted"><li>• High-entropy random capability link</li><li>• Read-only byte-range streaming</li><li>• No source paths or Library metadata database exposed</li><li>• Recipients can save bytes their browser receives</li><li>• One active session; memory-only state</li></ul><Button className="mt-5 w-full" disabled={busy || !selected.length} onClick={() => void start()}>{busy ? <LoaderCircle className="animate-spin" size={15} /> : <Share2 size={15} />} Start temporary share</Button></aside></div>}
  </div>;
}
