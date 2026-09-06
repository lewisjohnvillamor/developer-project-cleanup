import { useEffect, type ReactNode } from "react";
import { X } from "lucide-react";
import { IconButton } from "./Button";

export function Dialog({
  open,
  onClose,
  title,
  subtitle,
  children,
  footer,
  width = "max-w-xl",
  closable = true,
}: {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  subtitle?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  width?: string;
  closable?: boolean;
}) {
  useEffect(() => {
    if (!open || !closable) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, closable, onClose]);

  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6 backdrop-blur-[1px]" onMouseDown={closable ? onClose : undefined} role="presentation">
      <div
        role="dialog"
        aria-modal="true"
        className={`fade-in flex max-h-[90vh] w-full ${width} flex-col overflow-hidden rounded-xl border border-border bg-surface shadow-panel`}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <header className="flex items-start justify-between gap-4 border-b border-border px-5 py-4">
          <div>
            <h2 className="text-[15px] font-semibold text-fg">{title}</h2>
            {subtitle && <p className="mt-0.5 text-[12px] text-fg-muted">{subtitle}</p>}
          </div>
          {closable && (
            <IconButton title="Close" onClick={onClose}>
              <X size={16} />
            </IconButton>
          )}
        </header>
        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">{children}</div>
        {footer && <footer className="flex items-center justify-end gap-2 border-t border-border bg-surface-2/60 px-5 py-3">{footer}</footer>}
      </div>
    </div>
  );
}
