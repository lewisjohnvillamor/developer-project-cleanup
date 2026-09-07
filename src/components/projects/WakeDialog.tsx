import { Button } from "@/components/common/Button";
import { Dialog } from "@/components/common/Dialog";
import { useAppStore } from "@/stores/app-store";
import { AlertTriangle, ArchiveRestore, Check, Loader2, Play, X, XCircle } from "lucide-react";
import { useEffect, useRef } from "react";

export function WakeDialog() {
  const w = useAppStore((s) => s.wake);
  const project = useAppStore((s) => s.projects.find((p) => p.id === s.wake.projectId));
  const close = useAppStore((s) => s.closeWake);
  const run = useAppStore((s) => s.runWake);
  const cancel = useAppStore((s) => s.cancelWake);
  const logRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [w.lines.length]);

  if (w.stage === "idle" || !w.plan) return null;
  const plan = w.plan;

  return (
    <Dialog
      open
      onClose={close}
      closable={w.stage !== "running"}
      title={`Wake ${project?.name ?? ""}`}
      subtitle={w.stage === "confirm" ? "Reinstall dependencies with the project's own toolchain. Commands run only inside the project folder." : undefined}
      width="max-w-2xl"
      footer={
        w.stage === "confirm" ? (
          <>
            <Button variant="ghost" onClick={close}>
              Cancel
            </Button>
            <Button
              variant="primary"
              icon={<Play size={14} />}
              onClick={run}
              disabled={plan.steps.length === 0}
              title={plan.missingTools.length ? "A required tool is missing; the run will fail" : undefined}
            >
              Run Command{plan.steps.length > 1 ? "s" : ""}
            </Button>
          </>
        ) : w.stage === "running" ? (
          <Button variant="outline" icon={<X size={14} />} onClick={cancel}>
            Stop
          </Button>
        ) : (
          <Button variant="primary" onClick={close}>
            Done
          </Button>
        )
      }
    >
      {w.error && <div className="mb-3 rounded-md bg-danger-soft px-3 py-2 text-[12.5px] text-danger">{w.error}</div>}
      <div className="flex flex-col gap-3">
        <div className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-[12.5px]">
          {plan.packageManager && (
            <>
              <span className="text-fg-muted">Detected package manager</span>
              <span className="text-fg">{plan.packageManager}</span>
            </>
          )}
          <span className="text-fg-muted">Command{plan.steps.length > 1 ? "s" : ""}</span>
          <div className="flex flex-col gap-1">
            {plan.steps.map((s, i) => (
              <div key={i} className="flex items-center gap-2">
                {w.stage !== "confirm" &&
                  (w.currentStep > i || w.stage === "done" ? (
                    w.success === false && w.currentStep === i ? (
                      <XCircle size={13} className="text-danger" />
                    ) : (
                      <Check size={13} className="text-safe" />
                    )
                  ) : w.currentStep === i ? (
                    <Loader2 size={13} className="animate-spin text-accent" />
                  ) : (
                    <span className="h-3 w-3" />
                  ))}
                <code className="rounded bg-surface-2 px-2 py-0.5 font-mono text-[12px] text-fg selectable">{s.display}</code>
              </div>
            ))}
          </div>
          <span className="text-fg-muted">Runs inside</span>
          <span className="truncate font-mono text-[12px] text-fg selectable">{plan.cwd}</span>
        </div>
        {plan.missingTools.length > 0 && (
          <div className="flex items-start gap-2 rounded-md border border-review/30 bg-review-soft/50 px-3 py-2 text-[12px] text-review">
            <AlertTriangle size={14} className="mt-0.5 shrink-0" />
            <div>
              <div className="font-medium">
                Not found on PATH:{" "}
                {plan.missingTools.map((t) => (
                  <code key={t} className="mx-0.5 rounded bg-surface px-1 font-mono">
                    {t}
                  </code>
                ))}
              </div>
              <div className="text-fg-muted">Install it, or make sure the app was started from a shell where it is available.</div>
            </div>
          </div>
        )}
        {w.stage === "confirm" && plan.notes.length > 0 && (
          <ul className="text-[11.5px] text-fg-subtle">
            {plan.notes.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        )}
        {w.stage !== "confirm" && (
          <pre
            ref={logRef}
            className="max-h-72 min-h-40 overflow-auto rounded-md border border-border bg-[#0d0d10] p-3 font-mono text-[11.5px] leading-5 text-[#d6d6dc] selectable"
          >
            {w.lines.length ? w.lines.join("\n") : "Starting…"}
          </pre>
        )}
        {w.stage === "done" && (
          <div className={`flex items-center gap-2 rounded-md px-3 py-2 text-[12.5px] ${w.success ? "bg-safe-soft text-safe" : "bg-danger-soft text-danger"}`}>
            {w.success ? <ArchiveRestore size={14} /> : <XCircle size={14} />}
            {w.message}
          </div>
        )}
      </div>
    </Dialog>
  );
}
