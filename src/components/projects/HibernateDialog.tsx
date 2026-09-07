import { Button } from "@/components/common/Button";
import { Checkbox, ProgressBar, Toggle } from "@/components/common/Controls";
import { Dialog } from "@/components/common/Dialog";
import { useAppStore } from "@/stores/app-store";
import { describeOutcome, outcomeIsError } from "@/types";
import { formatBytes, formatCount, pluralize } from "@/utils/format";
import { AlertTriangle, Archive, Check, ChevronDown, ChevronRight, Loader2, ShieldCheck, X } from "lucide-react";
import { useState } from "react";

const DISPOSITION_TEXT = {
  trash: "Removed folders go to the Recycle Bin / Trash.",
  quarantine: "Removed folders go to quarantine and can be restored from History.",
  permanent: "Removed folders are deleted permanently.",
} as const;

export function HibernateDialog() {
  const h = useAppStore((s) => s.hibernate);
  const close = useAppStore((s) => s.closeHibernate);
  const confirm = useAppStore((s) => s.confirmHibernate);
  const cancel = useAppStore((s) => s.cancelHibernate);
  const setIncludeReview = useAppStore((s) => s.setIncludeReview);
  const toggleReviewArtifact = useAppStore((s) => s.toggleReviewArtifact);
  const saveSettings = useAppStore((s) => s.saveSettings);
  const setPage = useAppStore((s) => s.setPage);
  const [showFiles, setShowFiles] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  if (h.stage === "idle") return null;

  const toggleExpanded = (id: string) => {
    const next = new Set(expanded);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setExpanded(next);
  };

  // ---- Review -------------------------------------------------------------
  if (h.stage === "review") {
    const plan = h.plan;
    const n = plan?.projects.length ?? 0;
    const hasReviewAvailable = plan?.projects.some((p) => p.skippedReview.length > 0) || (plan?.reviewCount ?? 0) > 0;
    return (
      <Dialog
        open
        onClose={close}
        title={plan ? `Hibernate ${pluralize(n, "project")}?` : "Preparing review…"}
        subtitle="Only regeneratable folders are removed. Source, Git history, lockfiles and configuration stay untouched."
        width="max-w-2xl"
        footer={
          <>
            <Button variant="ghost" onClick={close}>
              Cancel
            </Button>
            <Button variant="outline" onClick={() => setShowFiles((v) => !v)} disabled={!plan}>
              {showFiles ? "Hide files" : "Review files"}
            </Button>
            <Button
              variant="primary"
              icon={<Archive size={14} />}
              onClick={confirm}
              disabled={!plan || plan.projects.every((p) => p.artifacts.length === 0) || h.planLoading}
              loading={h.planLoading}
            >
              Hibernate {n ? pluralize(n, "project") : ""}
            </Button>
          </>
        }
      >
        {h.error && <div className="mb-3 rounded-md bg-danger-soft px-3 py-2 text-[12.5px] text-danger">{h.error}</div>}
        {!plan ? (
          <div className="flex items-center gap-2 py-6 text-fg-muted">
            <Loader2 size={16} className="animate-spin" /> Calculating…
          </div>
        ) : (
          <div className="flex flex-col gap-4">
            <div className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-[13px]">
              <span className="text-fg-muted">Estimated recovery</span>
              <span className="text-[20px] font-semibold tabular leading-6 text-accent">{formatBytes(plan.totalBytes)}</span>
              <span className="text-fg-muted">Safe items</span>
              <span className="tabular text-fg">
                {formatCount(plan.folderCount)} folders · {formatCount(plan.fileCount)} files
              </span>
              <span className="text-fg-muted">Review items</span>
              <span className={`tabular ${plan.reviewCount ? "text-review" : "text-fg"}`}>{plan.reviewCount}</span>
              <span className="text-fg-muted">Protected files</span>
              <span className="flex items-center gap-1 text-safe">
                <ShieldCheck size={13} /> untouched
              </span>
              <span className="text-fg-muted">Destination</span>
              <span className={plan.disposition === "permanent" ? "text-danger" : "text-fg"}>{DISPOSITION_TEXT[plan.disposition]}</span>
            </div>

            {plan.skippedProtected.length > 0 && (
              <div className="rounded-md bg-surface-2 px-3 py-2 text-[12px] text-fg-muted">
                Skipped {plan.skippedProtected.length} protected: {plan.skippedProtected.join(", ")}
              </div>
            )}

            {hasReviewAvailable && (
              <div className="rounded-md border border-review/30 bg-review-soft/50 px-3 py-1">
                <Toggle
                  checked={h.request?.includeReview ?? false}
                  onChange={(v) => {
                    setIncludeReview(v);
                    saveSettings({ includeReviewItems: v });
                  }}
                  label={<span className="text-review">Include review items (.venv, Pods, vendor, .gradle)</span>}
                  description="Usually regeneratable, but behaviour varies between projects. Off by default."
                />
              </div>
            )}

            <ul className="divide-y divide-border rounded-md border border-border">
              {plan.projects.map((p) => (
                <li key={p.id} className="px-3 py-2">
                  <div className="flex items-center gap-2">
                    <button type="button" className="text-fg-subtle" onClick={() => toggleExpanded(p.id)} aria-label="Toggle details">
                      {expanded.has(p.id) || showFiles ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                    </button>
                    <span className="flex-1 truncate font-medium text-fg">{p.name}</span>
                    {p.warnings.length > 0 && <AlertTriangle size={13} className="text-review" />}
                    <span className="tabular text-fg">{formatBytes(p.bytes)}</span>
                  </div>
                  {(expanded.has(p.id) || showFiles) && (
                    <div className="fade-in ml-6 mt-1.5 flex flex-col gap-1 text-[12px]">
                      {p.artifacts.map((a) => (
                        <div key={a.path} className="flex items-center gap-2">
                          <Checkbox checked onChange={() => toggleReviewArtifact(p.id, a.path)} disabled={h.planLoading} title="Leave this folder in place" />
                          <span className="flex-1 font-mono text-fg-muted">{a.relativePath}</span>
                          <span className="tabular text-fg-muted">
                            {formatBytes(a.bytes)}
                            {a.safety === "review" && <span className="ml-1.5 text-review">review</span>}
                          </span>
                        </div>
                      ))}
                      {p.skippedReview.map((a) => (
                        <div key={a.path} className="flex items-center gap-2 text-fg-subtle">
                          <Checkbox
                            checked={false}
                            onChange={() => toggleReviewArtifact(p.id, a.path)}
                            disabled={h.planLoading}
                            title="Include this review folder"
                          />
                          <span className="flex-1 font-mono">{a.relativePath}</span>
                          <span className="tabular">
                            {formatBytes(a.bytes)} · <span className="text-review">needs review</span>, left in place
                          </span>
                        </div>
                      ))}
                      {(() => {
                        // Safe folders unticked earlier can be brought back too.
                        const original = useAppStore.getState().projects.find((x) => x.id === p.id);
                        const shown = new Set([...p.artifacts, ...p.skippedReview].map((a) => a.path));
                        const rest = original?.artifacts.filter((a) => !shown.has(a.path) && a.safety !== "protected") ?? [];
                        return rest.map((a) => (
                          <div key={a.path} className="flex items-center gap-2 text-fg-subtle">
                            <Checkbox
                              checked={false}
                              onChange={() => toggleReviewArtifact(p.id, a.path)}
                              disabled={h.planLoading}
                              title="Include this folder"
                            />
                            <span className="flex-1 font-mono">{a.relativePath}</span>
                            <span className="tabular">{formatBytes(a.bytes)} · left in place</span>
                          </div>
                        ));
                      })()}
                      {p.warnings.map((w) => (
                        <div key={w} className="mt-1 flex items-center gap-1 text-review">
                          <AlertTriangle size={12} /> {w}
                        </div>
                      ))}
                      {p.artifacts.length === 0 && <div className="text-fg-subtle">Nothing selected for this project.</div>}
                    </div>
                  )}
                </li>
              ))}
            </ul>
            <p className="text-[12px] text-fg-subtle">
              Expand a project to untick individual folders. Projects remain usable after reinstalling dependencies. Use Wake to bring them back.
            </p>
          </div>
        )}
      </Dialog>
    );
  }

  // ---- Running --------------------------------------------------------------
  if (h.stage === "running") {
    const progress = h.total ? h.completed / h.total : null;
    return (
      <Dialog
        open
        onClose={() => {}}
        closable={false}
        title="Hibernate Projects"
        subtitle={`${h.completed} / ${h.total} completed`}
        width="max-w-xl"
        footer={
          <Button variant="outline" icon={<X size={14} />} onClick={cancel}>
            Cancel after current folder
          </Button>
        }
      >
        <div className="flex flex-col gap-4">
          <div>
            <div className="mb-1.5 flex items-center justify-between text-[12.5px]">
              <span className="text-fg-muted">Recovered so far</span>
              <span className="text-[18px] font-semibold tabular text-safe">{formatBytes(h.totalRecovered)}</span>
            </div>
            <ProgressBar value={progress} tone="safe" />
          </div>
          <ul className="divide-y divide-border rounded-md border border-border">
            {h.order.map((id) => {
              const pp = h.perProject[id];
              if (!pp) return null;
              const current = h.currentProjectId === id;
              return (
                <li key={id} className="px-3 py-2 text-[12.5px]">
                  <div className="flex items-center gap-2">
                    {pp.done ? (
                      pp.errorCount ? (
                        <AlertTriangle size={14} className="text-review" />
                      ) : (
                        <Check size={14} className="text-safe" />
                      )
                    ) : current ? (
                      <Loader2 size={14} className="animate-spin text-accent" />
                    ) : (
                      <span className="h-3.5 w-3.5" />
                    )}
                    <span className="flex-1 truncate font-medium text-fg">{pp.name}</span>
                    {pp.done ? (
                      <span className="tabular text-fg">
                        {formatBytes(pp.bytesRecovered)} recovered{pp.errorCount ? ` · ${pp.errorCount} error${pp.errorCount === 1 ? "" : "s"}` : ""}
                      </span>
                    ) : current && h.currentArtifact ? (
                      <span className="font-mono text-[11.5px] text-fg-muted">Cleaning {h.currentArtifact}…</span>
                    ) : null}
                  </div>
                  {current && pp.artifacts.length > 0 && (
                    <div className="ml-6 mt-1 flex flex-col gap-0.5 text-[11.5px] text-fg-muted">
                      {pp.artifacts.map((a) => (
                        <div key={a.relativePath} className="flex items-center justify-between">
                          <span className="font-mono">{a.relativePath}</span>
                          <span className={a.outcome && outcomeIsError(a.outcome) ? "text-review" : ""}>{a.outcome ? describeOutcome(a.outcome) : "…"}</span>
                        </div>
                      ))}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        </div>
      </Dialog>
    );
  }

  // ---- Done -------------------------------------------------------------------
  const entry = h.entry;
  const failures = entry?.projects.flatMap((p) => p.artifacts.filter((a) => outcomeIsError(a.outcome)).map((a) => ({ project: p.name, a }))) ?? [];
  return (
    <Dialog
      open
      onClose={close}
      title={entry?.cancelled ? "Cleanup cancelled" : "Cleanup Complete"}
      width="max-w-xl"
      footer={
        <>
          <Button
            variant="ghost"
            onClick={() => {
              close();
              setPage("history");
            }}
          >
            View History
          </Button>
          <Button
            variant="ghost"
            onClick={() => {
              close();
              setPage("projects");
            }}
          >
            Open Projects
          </Button>
          <Button variant="primary" onClick={close}>
            Done
          </Button>
        </>
      }
    >
      {entry && (
        <div className="flex flex-col gap-4">
          <div className="flex items-baseline gap-3">
            <span className="text-[34px] font-semibold tabular tracking-tight text-safe">{formatBytes(entry.totalRecovered)}</span>
            <span className="text-[13px] text-fg-muted">recovered</span>
          </div>
          <div className="grid grid-cols-2 gap-2 text-[12.5px]">
            <div className="rounded-md bg-surface-2 px-3 py-2">
              <div className="text-[11px] uppercase tracking-wide text-fg-muted">Projects hibernated</div>
              <div className="text-[16px] font-semibold tabular text-fg">{entry.projectCount}</div>
            </div>
            <div className="rounded-md bg-surface-2 px-3 py-2">
              <div className="text-[11px] uppercase tracking-wide text-fg-muted">Errors</div>
              <div className={`text-[16px] font-semibold tabular ${entry.errorCount ? "text-review" : "text-fg"}`}>{entry.errorCount}</div>
            </div>
          </div>
          {entry.projects.length > 0 && (
            <div>
              <div className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-fg-muted">Largest savings</div>
              <ul className="divide-y divide-border">
                {[...entry.projects]
                  .sort((a, b) => b.bytesRecovered - a.bytesRecovered)
                  .slice(0, 5)
                  .map((p) => (
                    <li key={p.projectId} className="flex items-center justify-between py-1.5 text-[12.5px]">
                      <span className="text-fg">{p.name}</span>
                      <span className="tabular text-fg-muted">{formatBytes(p.bytesRecovered)}</span>
                    </li>
                  ))}
              </ul>
            </div>
          )}
          {failures.length > 0 && (
            <div className="rounded-md border border-review/30 bg-review-soft/40 px-3 py-2 text-[12px]">
              <div className="mb-1 flex items-center gap-1 font-medium text-review">
                <AlertTriangle size={13} /> Could not fully remove
              </div>
              <ul className="flex flex-col gap-1 text-fg-muted">
                {failures.slice(0, 8).map(({ project, a }) => (
                  <li key={a.path}>
                    <span className="text-fg">{project}</span> <span className="font-mono">{a.relativePath}</span> — {describeOutcome(a.outcome)}
                    {a.outcome.kind === "partiallyDeleted" && a.outcome.failed[0] && (
                      <div className="ml-3 truncate font-mono text-[11px] text-fg-subtle">
                        {a.outcome.failed[0].path}: {a.outcome.failed[0].error}
                      </div>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </Dialog>
  );
}
