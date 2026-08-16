import { open } from "@tauri-apps/plugin-dialog";
import {
  ArrowDown,
  ArrowUp,
  CheckCircle2,
  FileCog,
  LoaderCircle,
  Plus,
  RefreshCw,
  Save,
  Trash2,
  XCircle,
} from "lucide-react";
import { useEffect, useState } from "react";
import { Button, Input, Select } from "../../components/ui";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type {
  LibraryCollection,
  PostDownloadRecipe,
  PostDownloadRecipeRun,
  PostDownloadRecipeStep,
} from "../../types";

const NEW_STEP_OPTIONS: Array<{ type: PostDownloadRecipeStep["type"]; label: string }> = [
  { type: "importLibrary", label: "Confirm Library import" },
  { type: "addTags", label: "Add tags" },
  { type: "addToCollection", label: "Add to collection" },
  { type: "transcodeMp4", label: "Create H.264/AAC MP4" },
  { type: "extractAudio", label: "Extract audio" },
  { type: "privacySafeCopy", label: "Create metadata-free privacy copy" },
  { type: "transcribe", label: "Transcribe with whisper.cpp" },
  { type: "semanticIndex", label: "Build semantic transcript index" },
];

export function RecipesPage() {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [recipes, setRecipes] = useState<PostDownloadRecipe[]>([]);
  const [runs, setRuns] = useState<PostDownloadRecipeRun[]>([]);
  const [collections, setCollections] = useState<LibraryCollection[]>([]);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [enabled, setEnabled] = useState(true);
  const [steps, setSteps] = useState<PostDownloadRecipeStep[]>([{ type: "importLibrary" }]);
  const [newStep, setNewStep] = useState<PostDownloadRecipeStep["type"]>("addTags");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);

  async function load() {
    setLoading(true);
    try {
      const [nextRecipes, nextRuns, nextCollections] = await Promise.all([
        api.listPostDownloadRecipes(),
        api.listPostDownloadRecipeRuns(50),
        api.listLibraryCollections(),
      ]);
      setRecipes(nextRecipes);
      setRuns(nextRuns);
      setCollections(nextCollections);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void api.onPostDownloadRecipeRunUpdated((run) => {
      if (disposed) return;
      setRuns((current) => [run, ...current.filter((candidate) => candidate.id !== run.id)].slice(0, 50));
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  function resetEditor() {
    setEditingId(null);
    setName("");
    setEnabled(true);
    setSteps([{ type: "importLibrary" }]);
  }

  function editRecipe(recipe: PostDownloadRecipe) {
    setEditingId(recipe.id);
    setName(recipe.name);
    setEnabled(recipe.enabled);
    setSteps(recipe.steps);
  }

  function appendStep() {
    if (steps.length >= 12) return;
    const step: PostDownloadRecipeStep = newStep === "importLibrary" ? { type: "importLibrary" }
      : newStep === "addTags" ? { type: "addTags", tags: [] }
      : newStep === "addToCollection" ? { type: "addToCollection", collectionId: collections[0]?.id || "" }
      : newStep === "transcodeMp4" ? { type: "transcodeMp4" }
      : newStep === "extractAudio" ? { type: "extractAudio", codec: "aac" }
      : newStep === "privacySafeCopy" ? { type: "privacySafeCopy" }
      : newStep === "transcribe" ? { type: "transcribe", modelPath: "", language: null }
      : { type: "semanticIndex", model: "nomic-embed-text" };
    setSteps((current) => [...current, step]);
  }

  function updateStep(index: number, step: PostDownloadRecipeStep) {
    setSteps((current) => current.map((candidate, candidateIndex) => candidateIndex === index ? step : candidate));
  }

  function moveStep(index: number, delta: -1 | 1) {
    setSteps((current) => {
      const target = index + delta;
      if (target < 0 || target >= current.length) return current;
      const next = [...current];
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
  }

  async function chooseModel(index: number, step: Extract<PostDownloadRecipeStep, { type: "transcribe" }>) {
    const selected = await open({
      title: "Choose a whisper.cpp model",
      directory: false,
      multiple: false,
      filters: [{ name: "whisper.cpp model", extensions: ["bin", "gguf"] }],
    });
    if (typeof selected === "string") updateStep(index, { ...step, modelPath: selected });
  }

  async function saveRecipe() {
    if (!name.trim() || !steps.length) return;
    setBusy(true);
    try {
      const saved = await api.savePostDownloadRecipe({ id: editingId, name: name.trim(), enabled, steps });
      setRecipes((current) => [...current.filter((recipe) => recipe.id !== saved.id), saved].sort((left, right) => left.name.localeCompare(right.name)));
      setNotice({ message: `Saved post-download recipe “${saved.name}”.`, view: "recipes" });
      resetEditor();
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function deleteRecipe(recipe: PostDownloadRecipe) {
    setBusy(true);
    try {
      await api.deletePostDownloadRecipe(recipe.id);
      setRecipes((current) => current.filter((candidate) => candidate.id !== recipe.id));
      if (editingId === recipe.id) resetEditor();
      setNotice({ message: `Deleted recipe “${recipe.name}”.`, view: "recipes" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  return <div className="mx-auto w-full max-w-[1480px] px-8 pb-14 pt-8 xl:px-10">
    <header className="mb-7 flex flex-wrap items-end gap-5"><div className="min-w-0 flex-1"><p className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent/90"><span className="h-px w-5 bg-accent/70" /> Automation</p><h1 className="flex items-center gap-3 text-[28px] font-semibold tracking-[-0.035em]"><FileCog className="text-accent" size={27} /> Post-download recipes</h1><p className="mt-2 max-w-3xl text-[13px] leading-5 text-muted">Build ordered, typed workflows that Rust runs after a verified download. Recipes never execute shell text; every step is validated and recorded.</p></div><Button variant="secondary" disabled={loading || busy} onClick={() => void load()}>{loading ? <LoaderCircle className="animate-spin" size={15} /> : <RefreshCw size={15} />} Refresh</Button></header>

    <div className="grid gap-5 xl:grid-cols-[minmax(0,0.9fr)_minmax(420px,1.1fr)]">
      <section className="panel-shadow border border-line/60 bg-panel/75" aria-labelledby="saved-recipes-title"><header className="flex items-center gap-3 border-b border-line/60 px-5 py-4"><div className="min-w-0 flex-1"><h2 id="saved-recipes-title" className="text-sm font-semibold">Saved recipes</h2><p className="mt-1 text-[10px] text-muted">Choose enabled recipes in New Download.</p></div><Button variant="ghost" className="h-8 text-xs" onClick={resetEditor}><Plus size={13} /> New</Button></header>{recipes.length ? <div className="divide-y divide-line/50">{recipes.map((recipe) => <article key={recipe.id} className={`flex items-center gap-3 px-5 py-4 ${editingId === recipe.id ? "bg-accent/[0.05]" : ""}`}><button className="min-w-0 flex-1 text-left" onClick={() => editRecipe(recipe)}><span className="flex items-center gap-2"><strong className="truncate text-xs font-semibold">{recipe.name}</strong><span className={`rounded-[4px] px-1.5 py-0.5 text-[8px] font-semibold uppercase ${recipe.enabled ? "bg-accent/10 text-accent" : "bg-elevated text-muted"}`}>{recipe.enabled ? "Enabled" : "Disabled"}</span></span><span className="mt-1 block text-[10px] text-muted">{recipe.steps.length} typed step{recipe.steps.length === 1 ? "" : "s"} · {recipe.steps.map(stepLabel).join(" → ")}</span></button><Button variant="ghost" className="h-8 px-2 text-xs" disabled={busy} onClick={() => void deleteRecipe(recipe)}><Trash2 size={13} /></Button></article>)}</div> : <p className="px-5 py-12 text-center text-xs text-muted">No recipes yet. Build one with the editor.</p>}</section>

      <section className="panel-shadow border border-line/60 bg-panel/75" aria-labelledby="recipe-editor-title"><header className="border-b border-line/60 px-5 py-4"><h2 id="recipe-editor-title" className="text-sm font-semibold">{editingId ? "Edit recipe" : "New recipe"}</h2><p className="mt-1 text-[10px] text-muted">Steps run sequentially after media verification and Library import.</p></header><div className="p-5"><div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_150px]"><label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Recipe name<Input className="mt-1 normal-case" value={name} maxLength={80} onChange={(event) => setName(event.target.value)} placeholder="Archive and transcribe" /></label><label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Availability<Select className="mt-1 normal-case" value={enabled ? "enabled" : "disabled"} onChange={(event) => setEnabled(event.target.value === "enabled")}><option value="enabled">Enabled</option><option value="disabled">Disabled</option></Select></label></div>
        <div className="mt-5 space-y-2">{steps.map((step, index) => <StepEditor key={`${step.type}:${index}`} step={step} index={index} collections={collections} onUpdate={(next) => updateStep(index, next)} onChooseModel={(transcribe) => void chooseModel(index, transcribe)} onMove={(delta) => moveStep(index, delta)} onRemove={() => setSteps((current) => current.filter((_, candidateIndex) => candidateIndex !== index))} first={index === 0} last={index === steps.length - 1} />)}</div>
        <div className="mt-3 flex flex-wrap items-center gap-2"><Select className="h-9 min-w-56 flex-1 text-xs" value={newStep} onChange={(event) => setNewStep(event.target.value as PostDownloadRecipeStep["type"])}>{NEW_STEP_OPTIONS.map((option) => <option key={option.type} value={option.type}>{option.label}</option>)}</Select><Button variant="secondary" disabled={steps.length >= 12} onClick={appendStep}><Plus size={14} /> Add step</Button><Button className="ml-auto" disabled={busy || !name.trim() || !steps.length} onClick={() => void saveRecipe()}>{busy ? <LoaderCircle className="animate-spin" size={14} /> : <Save size={14} />} Save recipe</Button></div>
      </div></section>
    </div>

    <section className="panel-shadow mt-6 border border-line/60 bg-panel/75" aria-labelledby="recipe-runs-title"><header className="border-b border-line/60 px-5 py-4"><h2 id="recipe-runs-title" className="text-sm font-semibold">Recent runs</h2><p className="mt-1 text-[10px] text-muted">Idempotent per recipe and download: restarting the app cannot run the same pairing twice.</p></header>{runs.length ? <div className="divide-y divide-line/50">{runs.map((run) => <article key={run.id} className="flex items-center gap-3 px-5 py-3">{run.state === "running" ? <LoaderCircle className="animate-spin text-accent" size={15} /> : run.state === "completed" ? <CheckCircle2 className="text-accent" size={15} /> : <XCircle className="text-red-300" size={15} />}<span className="min-w-0 flex-1"><strong className="block truncate text-xs font-medium">{run.recipeName}</strong><span className="mt-1 block text-[9px] text-muted">Step {run.currentStep}/{run.stepCount} · {new Date(run.updatedAt).toLocaleString()}</span>{run.error ? <span className="mt-1 block text-[9px] text-red-300">{run.error}</span> : null}</span><span className="rounded-[5px] bg-elevated px-2 py-1 text-[9px] font-semibold uppercase text-muted">{run.state}</span></article>)}</div> : <p className="px-5 py-10 text-center text-xs text-muted">No recipe runs yet.</p>}</section>
  </div>;
}

function StepEditor({ step, index, collections, onUpdate, onChooseModel, onMove, onRemove, first, last }: { step: PostDownloadRecipeStep; index: number; collections: LibraryCollection[]; onUpdate: (step: PostDownloadRecipeStep) => void; onChooseModel: (step: Extract<PostDownloadRecipeStep, { type: "transcribe" }>) => void; onMove: (delta: -1 | 1) => void; onRemove: () => void; first: boolean; last: boolean }) {
  return <article className="rounded-[8px] border border-line/55 bg-elevated/25 p-3"><div className="flex items-center gap-2"><span className="number-font grid size-6 shrink-0 place-items-center rounded-[5px] bg-accent/10 text-[9px] font-semibold text-accent">{index + 1}</span><strong className="min-w-0 flex-1 text-[10px] font-semibold">{stepLabel(step)}</strong><button className="grid size-7 place-items-center rounded-[5px] text-muted hover:bg-elevated hover:text-ink disabled:opacity-25" disabled={first} onClick={() => onMove(-1)} aria-label="Move step up"><ArrowUp size={12} /></button><button className="grid size-7 place-items-center rounded-[5px] text-muted hover:bg-elevated hover:text-ink disabled:opacity-25" disabled={last} onClick={() => onMove(1)} aria-label="Move step down"><ArrowDown size={12} /></button><button className="grid size-7 place-items-center rounded-[5px] text-muted hover:bg-red-400/10 hover:text-red-300" onClick={onRemove} aria-label="Remove step"><Trash2 size={12} /></button></div>
    {step.type === "addTags" ? <Input className="mt-2 h-8 text-xs" value={step.tags.join(", ")} onChange={(event) => onUpdate({ ...step, tags: event.target.value.split(",").map((tag) => tag.trim()).filter(Boolean) })} placeholder="archive, favorite" /> : null}
    {step.type === "addToCollection" ? <Select className="mt-2 h-8 text-xs" value={step.collectionId} onChange={(event) => onUpdate({ ...step, collectionId: event.target.value })}><option value="">Choose collection</option>{collections.map((collection) => <option key={collection.id} value={collection.id}>{collection.name}</option>)}</Select> : null}
    {step.type === "extractAudio" ? <Select className="mt-2 h-8 text-xs" value={step.codec} onChange={(event) => onUpdate({ ...step, codec: event.target.value as "mp3" | "aac" | "opus" })}><option value="aac">AAC / M4A</option><option value="mp3">MP3</option><option value="opus">Opus</option></Select> : null}
    {step.type === "transcribe" ? <div className="mt-2 grid gap-2 sm:grid-cols-[minmax(0,1fr)_110px_auto]"><Input className="h-8 text-xs" value={step.modelPath} readOnly placeholder="whisper.cpp model path" /><Input className="h-8 text-xs" value={step.language || ""} onChange={(event) => onUpdate({ ...step, language: event.target.value.trim() || null })} placeholder="auto" /><Button variant="secondary" className="h-8 text-xs" onClick={() => onChooseModel(step)}>Choose</Button></div> : null}
    {step.type === "semanticIndex" ? <Input className="mt-2 h-8 text-xs" value={step.model} onChange={(event) => onUpdate({ ...step, model: event.target.value })} placeholder="nomic-embed-text" /> : null}
  </article>;
}

function stepLabel(step: PostDownloadRecipeStep): string {
  return NEW_STEP_OPTIONS.find((option) => option.type === step.type)?.label || step.type;
}
