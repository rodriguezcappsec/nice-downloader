import { CheckCircle2, CircleX, Download, FolderOpen, ShieldCheck } from "lucide-react";
import { Button } from "./ui";
import type { OnboardingStatus } from "../types";

export function OnboardingDialog({ status, onComplete }: { status: OnboardingStatus; onComplete: () => void }) {
  const requiredReady = status.engines.filter((engine) => engine.required).every((engine) => engine.installed);
  return <div className="fixed inset-0 z-50 grid place-items-center bg-black/65 p-6 backdrop-blur-sm" role="dialog" aria-modal="true" aria-labelledby="onboarding-title">
    <div className="panel-shadow w-full max-w-2xl border border-line/80 bg-panel p-7">
      <div className="flex items-center gap-4"><div className="grid size-12 place-items-center rounded-[10px] bg-accent text-[#081426]"><Download size={22} /></div><div><div className="text-[10px] font-semibold uppercase tracking-[0.18em] text-accent">First launch</div><h1 id="onboarding-title" className="mt-1 text-2xl font-semibold tracking-[-0.03em]">Set up Nice Downloader</h1></div></div>
      <p className="mt-5 text-sm leading-6 text-muted">Nice Downloader orchestrates trusted command-line media engines. Downloads and credentials stay on this computer.</p>
      <div className="mt-5 grid gap-3 md:grid-cols-2">
        <section className="border border-line/60 bg-canvas/45 p-4"><h2 className="flex items-center gap-2 text-sm font-semibold"><ShieldCheck size={16} className="text-accent" /> Engine readiness</h2><div className="mt-3 space-y-2">{status.engines.filter((engine) => engine.required || ["ffprobe"].includes(engine.id)).map((engine) => <div key={engine.id} className="flex items-center gap-2 text-xs">{engine.installed ? <CheckCircle2 size={15} className="text-emerald-400" /> : <CircleX size={15} className="text-red-400" />}<span className="font-medium">{engine.displayName}</span><span className="ml-auto text-muted">{engine.installed ? engine.version || "Ready" : "Not found"}</span></div>)}</div>{!requiredReady ? <p className="mt-3 text-xs leading-5 text-amber-300">Install missing required engines through your package manager, then use Detect again in Settings.</p> : null}</section>
        <section className="border border-line/60 bg-canvas/45 p-4"><h2 className="flex items-center gap-2 text-sm font-semibold"><FolderOpen size={16} className="text-accent" /> Default destination</h2><p className="mt-3 break-all font-mono text-xs text-muted">{status.defaultDestination}</p><p className="mt-3 text-xs leading-5 text-muted">You can choose a different destination for every download. The app never executes arbitrary shell strings.</p></section>
      </div>
      <div className="mt-5 border border-line/60 bg-accent/[0.04] p-4 text-xs leading-5 text-muted"><strong className="text-ink">Privacy:</strong> browser cookies are opt-in. Managed cookie files remain in backend-only private storage; cookie values and paths are excluded from the React UI, logs, and diagnostics.</div>
      <div className="mt-6 flex items-center justify-between gap-4"><p className="text-xs text-muted">You can review access, engines, and desktop behavior in Settings.</p><Button onClick={onComplete}>{requiredReady ? "Start downloading" : "Continue to app"}</Button></div>
    </div>
  </div>;
}
