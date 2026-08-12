import { open, save } from "@tauri-apps/plugin-dialog";
import { ArchiveRestore, FileVideo, FolderSearch, Library, LoaderCircle, Save, Search, Tags, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button, Field, Input, Select } from "../../components/ui";
import { formatDuration } from "../../lib/format";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type { LibraryItem } from "../../types";

export function LibraryPage() {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [items, setItems] = useState<LibraryItem[]>([]);
  const [search, setSearch] = useState("");
  const [groupBy, setGroupBy] = useState<"none" | "site" | "tag" | "month">("none");
  const [missingOnly, setMissingOnly] = useState(false);
  const [tags, setTags] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => { void load(); }, []);
  async function load() {
    try { setItems(await api.listLibraryItems()); } catch (error) { setError(normalizeError(error)); }
  }

  const filtered = useMemo(() => {
    const query = search.trim().toLocaleLowerCase();
    return items.filter((item) => (!missingOnly || item.missing) && (!query || `${item.title} ${item.site || ""} ${item.path} ${item.tags.join(" ")}`.toLocaleLowerCase().includes(query)));
  }, [items, missingOnly, search]);
  const groups = useMemo(() => groupItems(filtered, groupBy), [filtered, groupBy]);

  async function importFiles() {
    const paths = await open({ multiple: true, directory: false, title: "Add media files to library" });
    if (!paths) return;
    setBusy(true);
    try {
      await api.importLibraryFiles(paths, tags.split(",").map((tag) => tag.trim()).filter(Boolean));
      await load();
      setNotice({ message: `Added ${paths.length} media file${paths.length === 1 ? "" : "s"}.`, view: "library" });
    } catch (error) { setError(normalizeError(error)); } finally { setBusy(false); }
  }

  async function reconcile() {
    setBusy(true);
    try {
      const result = await api.reconcileLibrary(); await load();
      setNotice({ message: `Checked ${result.checkedCount} files; ${result.missingCount} missing.`, view: "library" });
    } catch (error) { setError(normalizeError(error)); } finally { setBusy(false); }
  }

  async function exportBackup() {
    const path = await save({ title: "Export library metadata", defaultPath: "nice-downloader-library.json", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path) return;
    try { await api.exportLibrary(path); setNotice({ message: "Library metadata exported." }); } catch (error) { setError(normalizeError(error)); }
  }

  async function restoreBackup() {
    const path = await open({ multiple: false, directory: false, title: "Restore library metadata", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path) return;
    try { await api.restoreLibrary(path); await load(); setNotice({ message: "Library metadata restored.", view: "library" }); } catch (error) { setError(normalizeError(error)); }
  }

  return <div className="mx-auto w-full max-w-[1340px] px-8 pb-12 pt-8 xl:px-10">
    <header className="mb-7 flex flex-wrap items-end justify-between gap-4">
      <div><div className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent/90"><span className="h-px w-5 bg-accent/70" /> On this computer</div><h1 className="text-[28px] font-semibold tracking-[-0.035em]">Media library</h1><p className="mt-2 text-[13px] text-muted">Catalog downloads and existing media without moving or deleting the original files.</p></div>
      <div className="flex gap-2"><Button variant="secondary" onClick={() => void reconcile()}><FolderSearch size={16} /> Verify files</Button><Button onClick={() => void importFiles()}>{busy ? <LoaderCircle className="animate-spin" size={16} /> : <Library size={16} />} Add media</Button></div>
    </header>

    <section className="panel-shadow border border-line/60 bg-panel/70 p-4">
      <div className="grid gap-3 md:grid-cols-[minmax(220px,1fr)_180px_minmax(200px,.7fr)_auto] md:items-end">
        <Field label="Search"><div className="relative"><Search className="absolute left-3 top-2.5 text-muted" size={15} /><Input className="pl-9" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Title, site, path, or tag" /></div></Field>
        <Field label="Group"><Select value={groupBy} onChange={(event) => setGroupBy(event.target.value as typeof groupBy)}><option value="none">No grouping</option><option value="site">Site</option><option value="tag">Primary tag</option><option value="month">Month added</option></Select></Field>
        <Field label="Tags for new imports"><Input value={tags} onChange={(event) => setTags(event.target.value)} placeholder="music, archive" /></Field>
        <label className="flex h-10 items-center gap-2 text-xs text-muted"><input type="checkbox" checked={missingOnly} onChange={(event) => setMissingOnly(event.target.checked)} /> Missing only</label>
      </div>
      <div className="mt-3 flex justify-end gap-2 border-t border-line/60 pt-3"><Button variant="secondary" className="h-8 px-3 text-xs" onClick={() => void exportBackup()}><Save size={14} /> Export metadata</Button><Button variant="secondary" className="h-8 px-3 text-xs" onClick={() => void restoreBackup()}><ArchiveRestore size={14} /> Restore</Button></div>
    </section>

    <div className="mt-5 space-y-6">
      {groups.map(([group, groupItems]) => <section key={group}><div className="mb-2 flex items-center gap-2 text-xs font-semibold text-muted"><Tags size={14} />{group}<span className="font-normal">{groupItems.length}</span></div><div className="grid gap-3 xl:grid-cols-2">{groupItems.map((item) => <LibraryCard key={item.id} item={item} onChanged={(changed) => setItems((current) => current.map((value) => value.id === changed.id ? changed : value))} onRemove={() => setItems((current) => current.filter((value) => value.id !== item.id))} />)}</div></section>)}
      {filtered.length === 0 ? <div className="border border-dashed border-line p-12 text-center text-sm text-muted">No media matches the current library filters.</div> : null}
    </div>
  </div>;
}

function LibraryCard({ item, onChanged, onRemove }: { item: LibraryItem; onChanged: (item: LibraryItem) => void; onRemove: () => void }) {
  const setError = useAppStore((state) => state.setError);
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState(item.title);
  const [tags, setTags] = useState(item.tags.join(", "));
  async function update() { try { onChanged(await api.updateLibraryItem({ id: item.id, title, tags: tags.split(",").map((tag) => tag.trim()).filter(Boolean) })); setEditing(false); } catch (error) { setError(normalizeError(error)); } }
  async function remove() { try { await api.removeLibraryItem(item.id); onRemove(); } catch (error) { setError(normalizeError(error)); } }
  return <article className={`panel-shadow flex min-w-0 gap-4 border bg-panel/70 p-4 ${item.missing ? "border-red-500/30" : "border-line/60"}`}>
    <div className="grid h-20 w-28 shrink-0 place-items-center bg-elevated text-muted"><FileVideo size={24} /></div>
    <div className="min-w-0 flex-1">{editing ? <><Input value={title} onChange={(event) => setTitle(event.target.value)} /><Input className="mt-2" value={tags} onChange={(event) => setTags(event.target.value)} placeholder="Comma-separated tags" /></> : <><div className="flex items-start justify-between gap-2"><h3 className="truncate text-sm font-semibold">{item.title}</h3>{item.missing ? <span className="shrink-0 text-[10px] font-semibold text-red-400">MISSING</span> : null}</div><p className="mt-1 truncate text-[11px] text-muted">{item.site || "Local media"}{item.durationSeconds != null ? ` · ${formatDuration(item.durationSeconds)}` : ""}</p><p className="mt-2 truncate text-[11px] text-muted/70" title={item.path}>{item.path}</p><div className="mt-2 flex flex-wrap gap-1">{item.tags.map((tag) => <span key={tag} className="bg-elevated px-1.5 py-0.5 text-[10px] text-muted">{tag}</span>)}</div></>}
      <div className="mt-3 flex gap-2">{editing ? <><Button className="h-7 px-2 text-xs" onClick={() => void update()}>Save</Button><Button variant="secondary" className="h-7 px-2 text-xs" onClick={() => setEditing(false)}>Cancel</Button></> : <Button variant="secondary" className="h-7 px-2 text-xs" onClick={() => setEditing(true)}>Edit metadata</Button>}<Button variant="secondary" className="ml-auto h-7 px-2 text-xs text-red-400" onClick={() => void remove()}><Trash2 size={13} /> Remove</Button></div>
    </div>
  </article>;
}

function groupItems(items: LibraryItem[], groupBy: "none" | "site" | "tag" | "month"): [string, LibraryItem[]][] {
  const groups = new Map<string, LibraryItem[]>();
  for (const item of items) {
    const key = groupBy === "site" ? item.site || "Other" : groupBy === "tag" ? item.tags[0] || "Untagged" : groupBy === "month" ? new Date(item.addedAt).toLocaleDateString(undefined, { month: "long", year: "numeric" }) : "All media";
    groups.set(key, [...(groups.get(key) || []), item]);
  }
  return [...groups.entries()];
}
