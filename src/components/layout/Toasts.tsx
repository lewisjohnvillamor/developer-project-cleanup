import { AlertCircle, CheckCircle2, Info, X } from "lucide-react";
import { useAppStore } from "@/stores/app-store";

export function Toasts() {
  const toasts = useAppStore((s) => s.toasts);
  const dismiss = useAppStore((s) => s.dismissToast);
  if (!toasts.length) return null;
  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[60] flex w-80 flex-col gap-2">
      {toasts.map((t) => (
        <div key={t.id} className="pointer-events-auto fade-in flex items-start gap-2 rounded-lg border border-border bg-surface px-3 py-2.5 text-[12.5px] text-fg shadow-panel">
          {t.kind === "success" ? <CheckCircle2 size={15} className="mt-0.5 shrink-0 text-safe" /> : t.kind === "error" ? <AlertCircle size={15} className="mt-0.5 shrink-0 text-danger" /> : <Info size={15} className="mt-0.5 shrink-0 text-info" />}
          <span className="flex-1 selectable">{t.message}</span>
          <button type="button" onClick={() => dismiss(t.id)} className="text-fg-subtle hover:text-fg" aria-label="Dismiss">
            <X size={13} />
          </button>
        </div>
      ))}
    </div>
  );
}
