import type { ReactNode } from "react";

export function Page({
  title,
  description,
  actions,
  children,
}: {
  title: string;
  description: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="mx-auto w-full max-w-[1400px] px-8 pb-12 pt-8 xl:px-10">
      <header className="mb-8 flex min-h-16 items-start justify-between gap-8">
        <div className="min-w-0">
          <div className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent/90">
            <span className="h-px w-5 bg-accent/70" /> Local workspace
          </div>
          <h1 className="text-[28px] font-semibold leading-[1.08] tracking-[-0.035em] text-ink text-balance">{title}</h1>
          <p className="mt-2 max-w-[65ch] text-[13px] leading-5 text-muted text-pretty">{description}</p>
        </div>
        {actions ? <div className="shrink-0 pt-3">{actions}</div> : null}
      </header>
      {children}
    </div>
  );
}
