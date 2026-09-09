import type { ReactNode } from "react";

export function Card({
  title,
  action,
  children,
  className = "",
  padded = true,
}: {
  title?: ReactNode;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
  padded?: boolean;
}) {
  return (
    <section className={`rounded-lg border border-border bg-surface ${className}`}>
      {(title || action) && (
        <header className="flex items-center justify-between border-b border-border px-4 py-2.5">
          <h3 className="text-[12px] font-semibold uppercase tracking-wide text-fg-muted">{title}</h3>
          {action}
        </header>
      )}
      <div className={padded ? "p-4" : ""}>{children}</div>
    </section>
  );
}
