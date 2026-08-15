import { save } from "@tauri-apps/plugin-dialog";
import {
  ArrowDown,
  ArrowUp,
  Clapperboard,
  Download,
  Film,
  LoaderCircle,
  Plus,
  Save,
  Search,
  Trash2,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button, Field, Input, Textarea } from "../../components/ui";
import { formatDuration } from "../../lib/format";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type {
  LibraryItem,
  LibraryMoment,
  LibraryMontage,
  LibraryMontageSegment,
  SaveLibraryMontageRequest,
} from "../../types";

type MontageDraft = SaveLibraryMontageRequest;

const EMPTY_DRAFT: MontageDraft = {
  name: "Untitled montage",
  notes: "",
  fadeSeconds: 0.25,
  segments: [],
};

export function MontagePage() {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [projects, setProjects] = useState<LibraryMontage[]>([]);
  const [draft, setDraft] = useState<MontageDraft>({ ...EMPTY_DRAFT });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [sourceSearch, setSourceSearch] = useState("");
  const [sources, setSources] = useState<LibraryItem[]>([]);
  const [sourceLoading, setSourceLoading] = useState(false);
  const [selectedSource, setSelectedSource] = useState<LibraryItem | null>(null);
  const [moments, setMoments] = useState<LibraryMoment[]>([]);
  const [sourceMomentsLoading, setSourceMomentsLoading] = useState(false);
  const [segmentItems, setSegmentItems] = useState<Map<string, LibraryItem>>(new Map());
  const [manualLabel, setManualLabel] = useState("");
  const [manualStart, setManualStart] = useState("0");
  const [manualEnd, setManualEnd] = useState("10");

  useEffect(() => {
    let disposed = false;
    void api.listLibraryMontages()
      .then((value) => {
        if (disposed) return;
        setProjects(value);
        if (value[0]) setDraft(copyProject(value[0]));
      })
      .catch((error) => !disposed && setError(normalizeError(error)))
      .finally(() => !disposed && setLoading(false));
    return () => { disposed = true; };
  }, [setError]);

  useEffect(() => {
    let disposed = false;
    const timer = window.setTimeout(() => {
      setSourceLoading(true);
      void api.queryLibraryItems({
        page: 1,
        pageSize: 24,
        search: sourceSearch.trim() || null,
        sourceId: null,
        folderSourceId: null,
        folderPath: null,
        folderScope: "tree",
        mediaKind: "video",
        resolution: null,
        container: null,
        duration: null,
        dateAdded: null,
        datePublished: null,
        availability: "available",
        tags: [],
        creators: [],
        series: [],
        sites: [],
        rating: null,
        collectionId: null,
        favoriteOnly: false,
        sort: "recent",
      }).then((result) => {
        if (!disposed) setSources(result.items);
      }).catch((error) => {
        if (!disposed) setError(normalizeError(error));
      }).finally(() => {
        if (!disposed) setSourceLoading(false);
      });
    }, 250);
    return () => {
      disposed = true;
      window.clearTimeout(timer);
    };
  }, [sourceSearch, setError]);

  useEffect(() => {
    const ids = Array.from(new Set(draft.segments.map((segment) => segment.itemId)));
    if (ids.length === 0) {
      setSegmentItems(new Map());
      return;
    }
    let disposed = false;
    void api.getLibraryItemsByIds(ids).then((items) => {
      if (!disposed) setSegmentItems(new Map(items.map((item) => [item.id, item])));
    }).catch((error) => !disposed && setError(normalizeError(error)));
    return () => { disposed = true; };
  }, [draft.segments, setError]);

  const totalDuration = useMemo(
    () => draft.segments.reduce((total, segment) => total + Math.max(0, segment.endSeconds - segment.startSeconds), 0),
    [draft.segments],
  );
  const activeProject = draft.id ? projects.find((project) => project.id === draft.id) : null;
  const dirty = JSON.stringify(draft) !== JSON.stringify(activeProject ? copyProject(activeProject) : EMPTY_DRAFT);

  function selectProject(project: LibraryMontage) {
    if (dirty && !window.confirm("Discard the unsaved montage edits?")) return;
    setDraft(copyProject(project));
  }

  function createProject() {
    if (dirty && !window.confirm("Discard the unsaved montage edits?")) return;
    setDraft({ ...EMPTY_DRAFT, segments: [] });
  }

  async function chooseSource(item: LibraryItem) {
    setSelectedSource(item);
    setManualLabel(item.title);
    setManualStart("0");
    setManualEnd(String(Math.min(10, item.durationSeconds || 10)));
    setSourceMomentsLoading(true);
    try {
      setMoments(await api.listLibraryMoments(item.id));
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setSourceMomentsLoading(false);
    }
  }

  function addSegment(segment: LibraryMontageSegment) {
    if (draft.segments.length >= 50) {
      setError({ code: "library_montage_limit", message: "A montage can contain at most 50 segments." });
      return;
    }
    setDraft((current) => ({ ...current, segments: [...current.segments, segment] }));
  }

  function addMoment(moment: LibraryMoment) {
    if (moment.endSeconds == null) return;
    addSegment({
      itemId: moment.itemId,
      label: moment.label,
      startSeconds: moment.startSeconds,
      endSeconds: moment.endSeconds,
    });
  }

  function addManualRange() {
    if (!selectedSource) return;
    const startSeconds = Number(manualStart);
    const endSeconds = Number(manualEnd);
    if (!manualLabel.trim() || !Number.isFinite(startSeconds) || !Number.isFinite(endSeconds) || startSeconds < 0 || endSeconds - startSeconds < 0.1) {
      setError({ code: "invalid_library_montage_segment", message: "Enter a label and a clip range of at least 0.1 seconds." });
      return;
    }
    addSegment({ itemId: selectedSource.id, label: manualLabel.trim(), startSeconds, endSeconds });
  }

  function updateSegment(index: number, patch: Partial<LibraryMontageSegment>) {
    setDraft((current) => ({
      ...current,
      segments: current.segments.map((segment, position) => position === index ? { ...segment, ...patch } : segment),
    }));
  }

  function moveSegment(index: number, offset: -1 | 1) {
    const next = index + offset;
    if (next < 0 || next >= draft.segments.length) return;
    setDraft((current) => {
      const segments = [...current.segments];
      [segments[index], segments[next]] = [segments[next], segments[index]];
      return { ...current, segments };
    });
  }

  async function persistDraft(): Promise<LibraryMontage | null> {
    if (!draft.name.trim() || draft.segments.length === 0) return null;
    setSaving(true);
    try {
      const saved = await api.saveLibraryMontage(draft);
      setProjects((current) => [saved, ...current.filter((project) => project.id !== saved.id)]);
      setDraft(copyProject(saved));
      return saved;
    } catch (error) {
      setError(normalizeError(error));
      return null;
    } finally {
      setSaving(false);
    }
  }

  async function removeProject() {
    if (!draft.id || !window.confirm(`Delete the montage project “${draft.name}”? Exported videos are not affected.`)) return;
    try {
      await api.deleteLibraryMontage(draft.id);
      const remaining = projects.filter((project) => project.id !== draft.id);
      setProjects(remaining);
      setDraft(remaining[0] ? copyProject(remaining[0]) : { ...EMPTY_DRAFT, segments: [] });
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  async function exportMontage() {
    if (activeProject?.incomplete) {
      setError({
        code: "library_montage_incomplete",
        message: "A source was removed from this montage. Review the timeline and save it before exporting.",
      });
      return;
    }
    const project = await persistDraft();
    if (!project) return;
    const outputPath = await save({
      title: "Export video montage",
      defaultPath: `${safeFileStem(project.name)}.mp4`,
      filters: [{ name: "MP4 video", extensions: ["mp4"] }],
    });
    if (!outputPath) return;
    setExporting(true);
    try {
      await api.startLibraryMontageExport({ montageId: project.id, outputPath, overwrite: false });
      setNotice({ message: "Montage export started. Track its progress in Transformations.", view: "transformations" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setExporting(false);
    }
  }

  return <div className="mx-auto w-full max-w-[1500px] px-8 pb-12 pt-8 xl:px-10">
    <header className="mb-7 flex flex-wrap items-end gap-4">
      <div className="min-w-0 flex-1"><p className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent/90"><span className="h-px w-5 bg-accent/70" /> Creative tools</p><h1 className="text-[28px] font-semibold tracking-[-0.035em]">Video montage builder</h1><p className="mt-2 max-w-3xl text-[13px] leading-5 text-muted">Arrange saved moments or precise ranges from Library videos, add short fades, and render one normalized MP4 without changing the originals.</p></div>
      <Button variant="secondary" onClick={createProject}><Plus size={15} /> New project</Button>
    </header>

    <div className="grid gap-5 xl:grid-cols-[260px_minmax(0,1fr)]">
      <aside className="panel-shadow min-h-56 border border-line/60 bg-panel/75 p-3" aria-label="Montage projects">
        <p className="px-2 pb-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-muted">Projects · {projects.length}/200</p>
        {loading ? <p className="flex items-center gap-2 px-2 py-4 text-xs text-muted"><LoaderCircle className="animate-spin" size={14} /> Loading projects…</p> : projects.length ? <div className="space-y-1">{projects.map((project) => <button key={project.id} className={`w-full rounded-[7px] px-3 py-2.5 text-left transition ${draft.id === project.id ? "bg-accent/10 text-ink ring-1 ring-accent/30" : "text-muted hover:bg-elevated/65 hover:text-ink"}`} onClick={() => selectProject(project)}><strong className="block truncate text-xs font-semibold">{project.name}</strong><span className={`number-font mt-1 block text-[9px] ${project.incomplete ? "text-amber-400" : ""}`}>{project.incomplete ? `${project.segments.length}/${project.expectedSegmentCount} clips · repair needed` : `${project.segments.length} clips · ${formatDuration(project.segments.reduce((total, segment) => total + segment.endSeconds - segment.startSeconds, 0))}`}</span></button>)}</div> : <p className="px-2 py-5 text-xs leading-5 text-muted">No saved montage projects yet.</p>}
      </aside>

      <main className="min-w-0 space-y-5">
        <section className="panel-shadow border border-line/60 bg-panel/75 p-5">
          {activeProject?.incomplete ? <div className="mb-4 border-l-2 border-amber-400 bg-amber-400/5 px-3 py-2 text-xs leading-5 text-amber-200" role="status">One or more source videos were removed from this project. Review the remaining timeline, add replacements if needed, then save to acknowledge the repair before exporting.</div> : null}
          <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_180px]">
            <Field label="Project name"><Input maxLength={100} value={draft.name} onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))} /></Field>
            <Field label="Fade in/out per clip"><div className="flex items-center gap-2"><Input type="number" min="0" max="5" step="0.05" value={draft.fadeSeconds} onChange={(event) => setDraft((current) => ({ ...current, fadeSeconds: Number(event.target.value) }))} /><span className="text-xs text-muted">sec</span></div></Field>
          </div>
          <div className="mt-4"><Field label="Notes"><Textarea className="min-h-16" maxLength={2000} value={draft.notes} onChange={(event) => setDraft((current) => ({ ...current, notes: event.target.value }))} placeholder="Creative intent, delivery notes, or revision details" /></Field></div>
          <div className="mt-4 flex flex-wrap items-center gap-2 border-t border-line/60 pt-4"><span className="number-font mr-auto text-[10px] text-muted">{draft.segments.length}/50 clips · {formatDuration(totalDuration)} total</span>{draft.id ? <Button variant="ghost" className="text-red-400" onClick={() => void removeProject()}><Trash2 size={14} /> Delete</Button> : null}<Button variant="secondary" disabled={saving || !draft.name.trim() || draft.segments.length === 0} onClick={() => void persistDraft()}>{saving ? <LoaderCircle className="animate-spin" size={15} /> : <Save size={15} />} Save project</Button><Button disabled={saving || exporting || !draft.name.trim() || draft.segments.length === 0} onClick={() => void exportMontage()}>{exporting ? <LoaderCircle className="animate-spin" size={15} /> : <Download size={15} />} Export MP4</Button></div>
        </section>

        <section className="grid gap-5 2xl:grid-cols-[minmax(330px,0.8fr)_minmax(0,1.2fr)]">
          <div className="panel-shadow min-w-0 border border-line/60 bg-panel/75 p-5">
            <h2 className="text-sm font-semibold">Add source clips</h2><p className="mt-1 text-[11px] leading-5 text-muted">Choose a Library video, then add one of its saved ranges or enter exact times.</p>
            <div className="relative mt-4"><Search className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted" size={14} /><Input className="pl-9" value={sourceSearch} onChange={(event) => setSourceSearch(event.target.value)} placeholder="Search Library videos" /></div>
            <div className="mt-2 max-h-44 overflow-y-auto border border-line/55">{sourceLoading ? <p className="flex items-center gap-2 p-3 text-xs text-muted"><LoaderCircle className="animate-spin" size={13} /> Searching…</p> : sources.map((item) => <button key={item.id} className={`flex w-full items-center gap-3 border-b border-line/40 px-3 py-2 text-left last:border-b-0 ${selectedSource?.id === item.id ? "bg-accent/10" : "hover:bg-elevated/55"}`} onClick={() => void chooseSource(item)}><Film className="shrink-0 text-accent" size={14} /><span className="min-w-0 flex-1 truncate text-[11px] font-medium">{item.title}</span><span className="number-font text-[9px] text-muted">{formatDuration(item.durationSeconds)}</span></button>)}</div>

            {selectedSource ? <div className="mt-4 border-t border-line/60 pt-4"><p className="truncate text-xs font-semibold">{selectedSource.title}</p><div className="mt-3 grid grid-cols-[minmax(0,1fr)_90px_90px] gap-2"><Input maxLength={100} value={manualLabel} onChange={(event) => setManualLabel(event.target.value)} placeholder="Clip label" /><Input type="number" min="0" step="0.1" value={manualStart} onChange={(event) => setManualStart(event.target.value)} aria-label="Start seconds" /><Input type="number" min="0.1" step="0.1" value={manualEnd} onChange={(event) => setManualEnd(event.target.value)} aria-label="End seconds" /></div><Button variant="secondary" className="mt-2 w-full" onClick={addManualRange}><Plus size={14} /> Add exact range</Button>
              <div className="mt-4"><p className="mb-2 text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Saved clip ranges</p>{sourceMomentsLoading ? <p className="flex items-center gap-2 text-xs text-muted"><LoaderCircle className="animate-spin" size={13} /> Loading moments…</p> : moments.some((moment) => moment.endSeconds != null) ? <div className="space-y-1">{moments.filter((moment) => moment.endSeconds != null).map((moment) => <button key={moment.id} className="flex w-full items-center gap-2 rounded-[6px] bg-elevated/50 px-3 py-2 text-left hover:bg-elevated" onClick={() => addMoment(moment)}><Plus size={12} className="text-accent" /><span className="min-w-0 flex-1 truncate text-[10px] font-medium">{moment.label}</span><span className="number-font text-[9px] text-muted">{formatDuration(moment.startSeconds)}–{formatDuration(moment.endSeconds)}</span></button>)}</div> : <p className="text-[10px] leading-4 text-muted">No saved clip ranges. Exact times work without creating a moment first.</p>}</div>
            </div> : null}
          </div>

          <div className="panel-shadow min-w-0 border border-line/60 bg-panel/75 p-5">
            <div className="flex items-center gap-3"><div><h2 className="text-sm font-semibold">Timeline</h2><p className="mt-1 text-[11px] text-muted">The exported montage follows this top-to-bottom order.</p></div><Clapperboard className="ml-auto text-accent" size={20} /></div>
            {draft.segments.length ? <ol className="mt-4 space-y-2">{draft.segments.map((segment, index) => { const item = segmentItems.get(segment.itemId); return <li key={`${segment.itemId}-${index}`} className="grid gap-3 border border-line/55 bg-elevated/30 p-3 lg:grid-cols-[32px_minmax(0,1fr)_90px_90px_auto]"><span className="number-font grid size-8 place-items-center rounded-[6px] bg-accent/10 text-[10px] font-semibold text-accent">{index + 1}</span><div className="min-w-0"><Input className="h-8 text-xs" maxLength={100} value={segment.label} onChange={(event) => updateSegment(index, { label: event.target.value })} /><p className="mt-1 truncate text-[9px] text-muted">{item?.title || "Source unavailable — remove or repair this clip"}</p></div><Input className="h-8" type="number" min="0" step="0.1" value={segment.startSeconds} onChange={(event) => updateSegment(index, { startSeconds: Number(event.target.value) })} aria-label={`Clip ${index + 1} start seconds`} /><Input className="h-8" type="number" min="0.1" step="0.1" value={segment.endSeconds} onChange={(event) => updateSegment(index, { endSeconds: Number(event.target.value) })} aria-label={`Clip ${index + 1} end seconds`} /><div className="flex items-center"><button className="grid size-7 place-items-center text-muted hover:text-ink disabled:opacity-25" disabled={index === 0} onClick={() => moveSegment(index, -1)} aria-label={`Move ${segment.label} earlier`}><ArrowUp size={13} /></button><button className="grid size-7 place-items-center text-muted hover:text-ink disabled:opacity-25" disabled={index === draft.segments.length - 1} onClick={() => moveSegment(index, 1)} aria-label={`Move ${segment.label} later`}><ArrowDown size={13} /></button><button className="grid size-7 place-items-center text-muted hover:text-red-400" onClick={() => setDraft((current) => ({ ...current, segments: current.segments.filter((_, position) => position !== index) }))} aria-label={`Remove ${segment.label}`}><Trash2 size={13} /></button></div></li>; })}</ol> : <div className="mt-4 grid min-h-52 place-items-center border border-dashed border-line text-center text-xs leading-5 text-muted"><span><Clapperboard className="mx-auto mb-2 text-accent/60" size={24} />Add saved moments or exact ranges<br />to build the montage timeline.</span></div>}
          </div>
        </section>
      </main>
    </div>
  </div>;
}

function copyProject(project: LibraryMontage): MontageDraft {
  return {
    id: project.id,
    name: project.name,
    notes: project.notes,
    fadeSeconds: project.fadeSeconds,
    segments: project.segments.map((segment) => ({ ...segment })),
  };
}

function safeFileStem(value: string): string {
  const safe = value.normalize("NFKC").replace(/[\\/:*?"<>|\u0000-\u001f]/g, "-").replace(/\s+/g, " ").trim().replace(/[. ]+$/g, "").slice(0, 80);
  return safe || "montage";
}
