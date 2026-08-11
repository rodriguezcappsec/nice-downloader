import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode, TextareaHTMLAttributes } from "react";

export function Button({
  className = "",
  variant = "primary",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "ghost" | "danger";
}) {
  const variants = {
    primary: "bg-accent text-[#06251b] hover:brightness-110 border-accent shadow-[0_8px_22px_rgb(var(--accent)/0.16)]",
    secondary: "bg-elevated/85 text-ink hover:bg-elevated hover:border-accent/35 border-line/80 shadow-control",
    ghost: "bg-transparent text-muted hover:text-ink hover:bg-elevated/70 border-transparent",
    danger: "bg-danger/10 text-red-400 hover:bg-danger/20 border-danger/25",
  };
  return (
    <button
      className={`inline-flex h-10 items-center justify-center gap-2 rounded-[8px] border px-4 text-[13px] font-semibold transition-[transform,background-color,border-color,filter,color,box-shadow] duration-200 hover:-translate-y-0.5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/75 focus-visible:ring-offset-2 focus-visible:ring-offset-canvas disabled:pointer-events-none disabled:opacity-45 ${variants[variant]} ${className}`}
      {...props}
    />
  );
}

export function Input({ className = "", ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={`h-11 w-full rounded-[8px] border border-line/80 bg-canvas/70 px-3.5 text-[13px] text-ink shadow-[inset_0_1px_2px_rgb(0_0_0/0.08)] outline-none transition-[border-color,box-shadow,background-color] duration-200 placeholder:text-muted/60 hover:border-muted/50 focus:border-accent/80 focus:bg-panel focus:ring-2 focus:ring-accent/15 ${className}`}
      {...props}
    />
  );
}

export function Textarea({ className = "", ...props }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return (
    <textarea
      className={`min-h-20 w-full resize-y rounded-[8px] border border-line/80 bg-canvas/70 px-3.5 py-3 text-[13px] leading-5 text-ink shadow-[inset_0_1px_2px_rgb(0_0_0/0.08)] outline-none transition-[border-color,box-shadow,background-color] duration-200 placeholder:text-muted/60 hover:border-muted/50 focus:border-accent/80 focus:bg-panel focus:ring-2 focus:ring-accent/15 ${className}`}
      {...props}
    />
  );
}

export function Field({ label, children, hint }: { label: string; children: ReactNode; hint?: string }) {
  return (
    <label className="grid gap-2 text-[13px] font-semibold text-ink">
      <span className="tracking-[-0.01em]">{label}</span>
      {children}
      {hint ? <span className="max-w-[65ch] text-[11px] font-normal leading-[1.45] text-muted text-pretty">{hint}</span> : null}
    </label>
  );
}

export function Select({
  className = "",
  children,
  ...props
}: React.SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select
      className={`h-11 w-full rounded-[8px] border border-line/80 bg-canvas/70 px-3 text-[13px] text-ink shadow-[inset_0_1px_2px_rgb(0_0_0/0.08)] outline-none transition-[border-color,box-shadow,background-color] duration-200 hover:border-muted/50 focus:border-accent/80 focus:bg-panel focus:ring-2 focus:ring-accent/15 ${className}`}
      {...props}
    >
      {children}
    </select>
  );
}

export function Progress({ value, tone = "blue" }: { value: number; tone?: "blue" | "amber" | "green" | "red" }) {
  const colors = { blue: "bg-accent", amber: "bg-amber-400", green: "bg-emerald-400", red: "bg-red-400" };
  return (
    <div className="h-[7px] overflow-hidden rounded-[3px] border border-line/30 bg-canvas/80 shadow-[inset_0_1px_2px_rgb(0_0_0/0.18)]" aria-label={`${value.toFixed(0)}%`}>
      <div
        className={`relative h-full transition-[width] duration-500 ease-out after:absolute after:inset-x-0 after:top-0 after:h-px after:bg-white/30 ${colors[tone]}`}
        style={{ width: `${Math.max(0, Math.min(100, value))}%` }}
      />
    </div>
  );
}

export function EmptyState({ icon, title, description }: { icon: ReactNode; title: string; description: string }) {
  return (
    <div className="relative grid min-h-64 overflow-hidden rounded-[12px] border border-line/60 bg-panel/55 p-8 shadow-[inset_0_1px_0_rgb(255_255_255/0.025)]">
      <div className="absolute -right-12 -top-16 size-48 rounded-full bg-accent/5 blur-3xl" />
      <div className="relative flex max-w-xl items-start gap-5">
        <div className="grid size-12 shrink-0 place-items-center rounded-[10px] border border-line/70 bg-elevated/75 text-accent shadow-control">{icon}</div>
        <div className="pt-0.5">
          <h3 className="text-[15px] font-semibold tracking-[-0.02em] text-ink">{title}</h3>
          <p className="mt-2 max-w-md text-[13px] leading-5 text-muted text-pretty">{description}</p>
          <div className="mt-5 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.14em] text-muted/70"><span className="size-1.5 rounded-full bg-accent/70" /> Waiting for activity</div>
        </div>
      </div>
    </div>
  );
}
