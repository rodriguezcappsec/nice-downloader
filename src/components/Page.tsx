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
    <div className="workspace-enter mx-auto w-full max-w-[1440px] px-8 pb-14 pt-10 xl:px-12">
      <header className="mb-10 flex min-h-20 items-start justify-between gap-10">
        <div className="min-w-0">
          <div className="mb-3 flex items-center gap-2.5 text-[10px] font-semibold tracking-[0.08em] text-accent/90">
            <span className="h-px w-7 bg-accent/70" /> Media workspace
          </div>
          <h1 className="text-[34px] font-semibold leading-[1.04] tracking-[-0.045em] text-ink text-balance">{title}</h1>
          <p className="mt-3 max-w-[62ch] text-[13px] leading-5 text-muted text-pretty">{description}</p>
        </div>
        {actions ? <div className="shrink-0 pt-4">{actions}</div> : null}
      </header>
      {children}
    </div>
  );
}
