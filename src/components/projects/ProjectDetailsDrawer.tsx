import {
  AlertTriangle,
  Archive,
  ArchiveRestore,
  Ban,
  ChevronDown,
  Clock,
  Copy,
  EyeOff,
  FolderOpen,
  GitBranch,
  HelpCircle,
  Layers,
  Shield,
  ShieldOff,
  Timer,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Badge, GitBadge, SafetyBadge, StatusBadge } from "@/components/common/Badge";
import { Button, IconButton } from "@/components/common/Button";
import { Checkbox, Menu } from "@/components/common/Controls";
import { useAppStore } from "@/stores/app-store";
import { type CleanupArtifact, STACK_LABELS } from "@/types";
import { isIgnored } from "@/utils/filters";
import { formatBytes, formatCount, formatDate, formatRelative } from "@/utils/format";

export function ProjectDetailsDrawer() {
  const id = useAppStore((s) => s.drawerProjectId);
  const project = useAppStore((s) => s.projects.find((p) => p.id === id) ?? null);
  const close = useAppStore((s) => s.closeDrawer);
  const setProtected = useAppStore((s) => s.setProtected);
  const ignore = useAppStore((s) => s.ignoreProject);
  const unignore = useAppStore((s) => s.unignoreProject);
  const openFolder = useAppStore((s) => s.openFolder);
  const copyText = useAppStore((s) => s.copyText);
  const reviewHibernate = useAppStore((s) => s.reviewHibernate);
  const openWake = useAppStore((s) => s.openWake);
  const excludeFolder = useAppStore((s) => s.excludeFolder);
  const [chosen, setChosen] = useState<Set<string> | null>(null);
  const [explain, setExplain] = useState<string | null>(null);

  useEffect(() => {
    setChosen(null);
    setExplain(null);
  }, [id]);

  useEffect(() => {
    if (!id) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [id, close]);

  const selectedArtifacts = useMemo(() => {
    if (!project) return [];
    if (chosen) return project.artifacts.filter((a) => chosen.has(a.path));
    return project.artifacts.filter((a) => a.safety === "safe");
  }, [project, chosen]);

  if (!project) return null;
  const p = project;
  const ignored = isIgnored(p);
  const selectedBytes = selectedArtifacts.reduce((s, a) => s + a.bytes, 0);
  const isChosen = (a: CleanupArtifact) => (chosen ? chosen.has(a.path) : a.safety === "safe");
  const toggleArtifact = (a: CleanupArtifact) => {
    const next = new Set(chosen ?? p.artifacts.filter((x) => x.safety === "safe").map((x) => x.path));
    if (next.has(a.path)) next.delete(a.path);
    else next.add(a.path);
    setChosen(next);
  };

  return (
    <aside className="slide-in-right absolute inset-y-0 right-0 z-30 flex w-[440px] flex-col border-l border-border bg-surface shadow-panel">
      <header className="flex items-start justify-between gap-3 border-b border-border px-5 py-4">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h2 className="truncate text-[16px] font-semibold text-fg">{p.name}</h2>
            <StatusBadge status={p.status} />
          </div>
          <div className="mt-1 flex items-center gap-1.5">
            <span className="truncate font-mono text-[11px] text-fg-muted selectable" title={p.path}>
              {p.path}
            </span>
            <IconButton title="Copy path" onClick={() => copyText(p.path)} className="h-5 w-5">
              <Copy size={12} />
            </IconButton>
          </div>
        </div>
        <IconButton title="Close" onClick={close}>
          <X size={16} />
        </IconButton>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
        <dl className="grid grid-cols-2 gap-x-4 gap-y-3 text-[12.5px]">
          <div>
            <dt className="text-[11px] font-semibold uppercase tracking-wide text-fg-muted">Stack</dt>
            <dd className="mt-0.5 flex flex-wrap gap-1">
              {p.stacks.map((s) => (
                <Badge key={s} tone="neutral">
                  {STACK_LABELS[s]}
                </Badge>
              ))}
              {p.frameworks.map((f) => (
                <Badge key={f} tone="accent">
                  {f}
                </Badge>
              ))}
              {p.packageManager && <Badge tone="neutral">{p.packageManager}</Badge>}
            </dd>
          </div>
          <div>
            <dt className="text-[11px] font-semibold uppercase tracking-wide text-fg-muted">Last active</dt>
            <dd className="mt-0.5 text-fg" title={p.lastActivityAt ? formatDate(p.lastActivityAt) : undefined}>
              {formatRelative(p.lastActivityAt)}
              <div className="text-[11px] text-fg-subtle">
                {p.activity.gitCommitAt ? `commit ${formatRelative(p.activity.gitCommitAt).toLowerCase()}` : "no commits"}
                {p.activity.sourceModifiedAt ? ` · edited ${formatRelative(p.activity.sourceModifiedAt).toLowerCase()}` : ""}
              </div>
            </dd>
          </div>
          <div>
            <dt className="text-[11px] font-semibold uppercase tracking-wide text-fg-muted">Git</dt>
            <dd className="mt-0.5 flex flex-wrap gap-1">
              {p.git ? <GitBadge state={p.git.state} branch={p.git.branch} /> : <Badge tone="neutral">Unknown</Badge>}
              {p.git?.isRepo && p.git.remoteConfigured === true && <Badge tone="neutral">Remote configured</Badge>}
            </dd>
          </div>
          <div>
            <dt className="text-[11px] font-semibold uppercase tracking-wide text-fg-muted">Size</dt>
            <dd className="mt-0.5 text-fg">
              <span className="tabular">{formatBytes(p.totalBytes)}</span> total
              <div className="text-[11px] text-fg-subtle">
                <span className="tabular font-semibold text-safe">{formatBytes(p.reclaimableBytes)}</span> safe
                {p.reviewBytes > 0 && (
                  <>
                    {" · "}
                    <span className="tabular text-review">{formatBytes(p.reviewBytes)}</span> review
                  </>
                )}
                {" · "}
                {formatCount(p.fileCount)} files
              </div>
            </dd>
          </div>
        </dl>

        {p.safetyReasons.length > 0 && (
          <div
            className={`mt-4 flex items-start gap-2 rounded-md px-3 py-2 text-[12px] ${p.safety === "review" ? "bg-review-soft text-review" : "bg-surface-2 text-fg-muted"}`}
          >
            {p.safety === "review" ? <AlertTriangle size={14} className="mt-0.5 shrink-0" /> : <SafetyBadge safety={p.safety} compact />}
            <ul>
              {p.safetyReasons.map((r) => (
                <li key={r}>{r}</li>
              ))}
            </ul>
          </div>
        )}

        {p.workspaceMembers.length > 0 && (
          <div className="mt-4 rounded-md border border-border px-3 py-2 text-[12px]">
            <div className="flex items-center gap-1.5 font-medium text-fg">
              <Layers size={13} className="text-fg-muted" /> Workspace · {p.workspaceMembers.length} member{p.workspaceMembers.length === 1 ? "" : "s"}
            </div>
            <div className="mt-1 flex flex-wrap gap-1">
              {p.workspaceMembers.slice(0, 24).map((m) => (
                <span key={m} className="rounded border border-border bg-surface-2 px-1.5 py-0.5 font-mono text-[11px] text-fg-muted">
                  {m}
                </span>
              ))}
              {p.workspaceMembers.length > 24 && <span className="text-fg-subtle">+{p.workspaceMembers.length - 24} more</span>}
            </div>
            <div className="mt-1 text-fg-subtle">Members share this project's artifacts, so nothing is counted twice.</div>
          </div>
        )}

        {p.scanDurationMs > 5000 && (
          <div className="mt-4 flex items-start gap-2 rounded-md bg-surface-2 px-3 py-2 text-[12px] text-fg-muted">
            <Timer size={14} className="mt-0.5 shrink-0" />
            <div>
              Measuring this project took {(p.scanDurationMs / 1000).toFixed(0)}s.
              {p.cacheHits > 0
                ? ` ${p.cacheHits} folder size${p.cacheHits === 1 ? "" : "s"} came from the cache.`
                : " Unchanged folders are reused on the next scan."}
            </div>
          </div>
        )}

        {p.hibernation && (
          <div className="mt-4 rounded-md border border-border bg-surface-2/60 px-3 py-2.5 text-[12px]">
            <div className="flex items-center gap-1.5 font-medium text-fg">
              <Clock size={13} className="text-fg-muted" /> Hibernated {formatRelative(p.hibernation.hibernatedAt).toLowerCase()}
            </div>
            <div className="mt-1 grid grid-cols-3 gap-2 text-fg-muted">
              <div>
                <div className="text-[10.5px] uppercase tracking-wide">Previously</div>
                <div className="tabular text-fg">{formatBytes(p.hibernation.previousBytes)}</div>
              </div>
              <div>
                <div className="text-[10.5px] uppercase tracking-wide">Current</div>
                <div className="tabular text-fg">{formatBytes(p.totalBytes)}</div>
              </div>
              <div>
                <div className="text-[10.5px] uppercase tracking-wide">Saved</div>
                <div className="tabular font-semibold text-safe">{formatBytes(p.hibernation.savedBytes)}</div>
              </div>
            </div>
          </div>
        )}

        <section className="mt-5">
          <div className="mb-1.5 flex items-center justify-between">
            <h3 className="text-[11px] font-semibold uppercase tracking-wide text-fg-muted">Cleanup breakdown</h3>
            {chosen && (
              <button type="button" className="text-[11px] text-accent hover:underline" onClick={() => setChosen(null)}>
                Reset to safe defaults
              </button>
            )}
          </div>
          {p.artifacts.length ? (
            <ul className="divide-y divide-border rounded-md border border-border">
              {p.artifacts.map((a) => (
                <li key={a.path} className="px-3 py-2">
                  <div className="flex items-center gap-2.5">
                    <Checkbox checked={isChosen(a)} onChange={() => toggleArtifact(a)} disabled={p.protected} />
                    <button type="button" className="min-w-0 flex-1 text-left" onClick={() => setExplain(explain === a.path ? null : a.path)}>
                      <span className="font-mono text-[12px] text-fg">{a.relativePath}</span>
                    </button>
                    <span className="tabular text-[12px] text-fg">{formatBytes(a.bytes)}</span>
                    <SafetyBadge safety={a.safety} />
                    <IconButton title="Why can this be removed?" className="h-5 w-5" onClick={() => setExplain(explain === a.path ? null : a.path)}>
                      {explain === a.path ? <ChevronDown size={13} /> : <HelpCircle size={13} />}
                    </IconButton>
                  </div>
                  {explain === a.path && (
                    <div className="fade-in ml-7 mt-2 rounded-md bg-surface-2 px-3 py-2 text-[12px] text-fg-muted">
                      <div className="text-fg">{a.explanation}</div>
                      <div className="mt-1.5 grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5">
                        <span>Recovery</span>
                        <span className="tabular text-fg">
                          {formatBytes(a.bytes)} · {formatCount(a.fileCount)} files · {formatCount(a.dirCount)} folders
                        </span>
                        <span>Restore</span>
                        <span className="font-mono text-[11.5px] text-fg selectable">{a.restoreHint ?? "Regenerated automatically by the toolchain"}</span>
                        {a.trackedByGit && (
                          <>
                            <span>Git</span>
                            <span className="text-danger">Tracked by Git, so this folder is kept whatever its name suggests.</span>
                          </>
                        )}
                        {a.ignoredByGit && !a.trackedByGit && (
                          <>
                            <span>Git</span>
                            <span className="text-safe">Ignored by this repository, which confirms it is generated.</span>
                          </>
                        )}
                        {a.safety === "review" && (
                          <>
                            <span>Note</span>
                            <span>Marked for review: check it before including it in a cleanup.</span>
                          </>
                        )}
                      </div>
                    </div>
                  )}
                </li>
              ))}
            </ul>
          ) : (
            <p className="rounded-md border border-dashed border-border px-3 py-3 text-[12px] text-fg-muted">
              {p.status === "hibernated"
                ? "Everything regeneratable has been removed. Wake the project to reinstall dependencies."
                : "No regeneratable folders found in this project."}
            </p>
          )}
        </section>

        {p.protectedEntries.length > 0 && (
          <section className="mt-5">
            <h3 className="mb-1.5 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wide text-fg-muted">
              <Shield size={11} className="text-danger" /> Protected · never touched
            </h3>
            <div className="flex flex-wrap gap-1">
              {p.protectedEntries.map((e) => (
                <span key={e} className="rounded border border-border bg-surface-2 px-1.5 py-0.5 font-mono text-[11px] text-fg-muted">
                  {e}
                </span>
              ))}
            </div>
          </section>
        )}

        {p.warnings.length > 0 && (
          <section className="mt-5 text-[11.5px] text-fg-subtle">
            {p.warnings.slice(0, 5).map((w) => (
              <div key={w} className="truncate" title={w}>
                {w}
              </div>
            ))}
          </section>
        )}
      </div>

      <footer className="flex flex-col gap-2 border-t border-border bg-surface-2/60 px-5 py-3">
        <div className="flex gap-2">
          {p.status === "hibernated" || (p.artifacts.length === 0 && p.hibernation) ? (
            <Button variant="primary" icon={<ArchiveRestore size={14} />} className="flex-1" onClick={() => openWake(p.id)}>
              Wake Project
            </Button>
          ) : (
            <Button
              variant="primary"
              icon={<Archive size={14} />}
              className="flex-1"
              disabled={p.protected || selectedArtifacts.length === 0}
              title={p.protected ? "Unprotect the project first" : undefined}
              onClick={() => reviewHibernate([p.id], { [p.id]: selectedArtifacts.map((a) => a.path) })}
            >
              Hibernate{selectedBytes > 0 ? ` · ${formatBytes(selectedBytes)}` : ""}
            </Button>
          )}
          <Button
            variant={p.protected ? "secondary" : "outline"}
            icon={p.protected ? <ShieldOff size={14} /> : <Shield size={14} />}
            onClick={() => setProtected(p.id, !p.protected)}
          >
            {p.protected ? "Unprotect" : "Protect"}
          </Button>
        </div>
        <div className="flex gap-2">
          <Button variant="ghost" size="sm" icon={<FolderOpen size={13} />} onClick={() => openFolder(p.id)}>
            Open folder
          </Button>
          <Button variant="ghost" size="sm" icon={<Copy size={13} />} onClick={() => copyText(p.path)}>
            Copy path
          </Button>
          {p.git?.branch && (
            <Button variant="ghost" size="sm" icon={<GitBranch size={13} />} onClick={() => copyText(p.git?.branch ?? "", "Branch")}>
              {p.git.branch}
            </Button>
          )}
          <div className="flex-1" />
          {ignored ? (
            <Button variant="ghost" size="sm" onClick={() => unignore(p.id)}>
              Stop ignoring
            </Button>
          ) : (
            <Menu
              align="right"
              trigger={() => (
                <Button variant="ghost" size="sm" icon={<EyeOff size={13} />}>
                  Ignore <ChevronDown size={12} />
                </Button>
              )}
              items={[
                { label: "Hide for 7 days", onSelect: () => ignore(p.id, 7) },
                { label: "Hide for 30 days", onSelect: () => ignore(p.id, 30) },
                { label: "Hide for 90 days", onSelect: () => ignore(p.id, 90) },
                { separator: true, label: "" },
                { label: "Hide until restored", onSelect: () => ignore(p.id, null) },
                { separator: true, label: "" },
                { label: "Exclude from future scans", icon: <Ban size={13} />, danger: true, onSelect: () => excludeFolder(p.path) },
              ]}
            />
          )}
        </div>
      </footer>
    </aside>
  );
}
