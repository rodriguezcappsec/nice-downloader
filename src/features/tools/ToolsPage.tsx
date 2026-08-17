import { ArrowRight, CheckCircle2, CircleDot, MousePointer2, PlayCircle } from "lucide-react";
import { useAppStore } from "../../stores/app-store";
import { toolNavigation } from "../../lib/navigation";

export function ToolsPage() {
  const setView = useAppStore((state) => state.setView);
  const compareCount = useAppStore((state) => state.compareItemIds.length);
  const imageCompareCount = useAppStore((state) => state.imageCompareItemIds.length);

  function stateFor(id: string): string | null {
    if (id === "compare" && compareCount > 0) return `${compareCount} video${compareCount === 1 ? "" : "s"} selected`;
    if (id === "imageCompare" && imageCompareCount > 0) return `${imageCompareCount} image${imageCompareCount === 1 ? "" : "s"} selected`;
    return null;
  }

  return (
    <div className="workspace-enter mx-auto w-full max-w-[1500px] px-7 pb-16 pt-8 xl:px-11">
      <header className="tools-hero relative overflow-hidden rounded-media border border-line/40 px-7 py-8 lg:px-9">
        <div className="relative max-w-3xl">
          <p className="eyebrow">Private media studio</p>
          <h1 className="mt-3 text-[36px] font-semibold leading-[1.04] tracking-[-0.05em] text-ink">Shape, understand, and care for your collection.</h1>
          <p className="mt-4 max-w-[68ch] text-[14px] leading-6 text-muted">Specialized workflows live here so Home and Library can stay calm. Every operation remains local, explicit, and attached to the media you choose.</p>
        </div>
      </header>

      <section className="mt-9" aria-labelledby="tools-heading">
        <div className="mb-5 flex items-end justify-between gap-5">
          <div>
            <h2 id="tools-heading" className="text-[22px] font-semibold tracking-[-0.035em]">Your media toolkit</h2>
            <p className="mt-1 text-[13px] leading-5 text-muted">Open a workspace now, or select media in Library first when a focused set is useful.</p>
          </div>
          <span className="number-font text-[12px] text-muted">{toolNavigation.length} local tools</span>
        </div>

        <div className="grid gap-3 md:grid-cols-2 2xl:grid-cols-3">
          {toolNavigation.map((tool) => {
            const Icon = tool.icon;
            const currentState = stateFor(tool.id);
            return (
              <article key={tool.id} className="tool-card group flex min-h-[220px] flex-col rounded-panel border border-line/45 bg-panel/55 p-5">
                <div className="flex items-start gap-4">
                  <span className="grid size-11 shrink-0 place-items-center rounded-[12px] border border-accent/15 bg-accent/[0.08] text-accent"><Icon size={20} strokeWidth={1.8} /></span>
                  <div className="min-w-0 flex-1">
                    <h3 className="text-[16px] font-semibold tracking-[-0.025em]">{tool.label}</h3>
                    <p className="mt-2 text-[13px] leading-5 text-muted">{tool.description}</p>
                  </div>
                </div>
                <p className="mt-4 border-l border-line/70 pl-3 text-[12px] leading-5 text-muted/90">{tool.usage}</p>
                <div className="mt-auto flex items-end justify-between gap-4 pt-5">
                  <div className="min-w-0">
                    {currentState ? (
                      <span className="inline-flex items-center gap-1.5 text-[12px] font-medium text-accent"><CircleDot size={13} /> {currentState}</span>
                    ) : (
                      <span className="inline-flex items-center gap-1.5 text-[12px] text-muted">
                        {tool.selection === "required" ? <MousePointer2 size={13} /> : tool.selection === "optional" ? <PlayCircle size={13} /> : <CheckCircle2 size={13} />}
                        {tool.selection === "required" ? "Select media first" : tool.selection === "optional" ? "Selection optional" : "Ready to open"}
                      </span>
                    )}
                  </div>
                  <button className="inline-flex h-10 shrink-0 items-center gap-2 rounded-control px-3 text-[13px] font-semibold text-ink transition hover:bg-elevated focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/75" onClick={() => setView(tool.id)} aria-label={`Open ${tool.label}`}>
                    Open <ArrowRight size={15} />
                  </button>
                </div>
              </article>
            );
          })}
        </div>
      </section>
    </div>
  );
}
