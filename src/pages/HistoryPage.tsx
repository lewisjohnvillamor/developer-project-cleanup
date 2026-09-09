import { AlertTriangle, ArchiveRestore, ChevronDown, ChevronRight, FolderOpen, History, ShieldAlert, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Badge } from "@/components/common/Badge";
import { Button } from "@/components/common/Button";
import { EmptyState } from "@/components/common/Controls";
import { useAppStore } from "@/stores/app-store";
import { describeOutcome, entryRestorable, type HistoryEntry, outcomeIsError } from "@/types";
import { formatBytes, formatDate, formatDateTime, pluralize } from "@/utils/format";

const DISPOSITION_LABEL = { trash: "Trash", quarantine: "Quarantine", permanent: "Permanent" } as const;

function Entry({ entry }: { entry: HistoryEntry }) {
  const [open, setOpen] = useState(false);
  const [details, setDetails] = useState<Set<string>>(new Set());
  const restore = useAppStore((s) => s.restoreEntry);
  const openFolder = useAppStore((s) => s.openFolder);
  const projects = useAppStore((s) => s.projects);
  const [restoring, setRestoring] = useState(false);
  const trashRestore = useAppStore((s) => s.info?.trashRestoreSupported ?? false);
  const restorable = entryRestorable(entry, trashRestore);
  const toggle = (id: string) => {
    const n = new Set(details);
    if (n.has(id)) n.delete(id);
    else n.add(id);
    setDetails(n);
  };

  return (
    <li className="rounded-lg border border-border bg-surface">
      <div className="flex items-center gap-3 px-4 py-3">
        <button type="button" onClick={() => setOpen((v) => !v)} className="text-fg-subtle hover:text-fg" aria-label="Toggle">
          {open ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
        </button>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 text-[13px]">
            <span className="font-semibold text-fg">
              {pluralize(entry.projectCount, "project")} hibernated · <span className="tabular text-safe">{formatBytes(entry.totalRecovered)}</span> recovered
            </span>
            {entry.errorCount > 0 && (
              <Badge tone="review" icon={<AlertTriangle size={11} />}>
                {entry.errorCount} error{entry.errorCount === 1 ? "" : "s"}
              </Badge>
            )}
            {entry.cancelled && <Badge tone="inactive">Cancelled</Badge>}
            {entry.restoredAt && <Badge tone="info">Restored</Badge>}
          </div>
          <div className="text-[11.5px] text-fg-subtle">
            {formatDateTime(entry.finishedAt)} · {DISPOSITION_LABEL[entry.disposition]}
            {restorable ? (entry.disposition === "trash" ? " · restorable from the Trash" : " · restorable from quarantine") : ""}
          </div>
        </div>
        {restorable && (
          <Button
            variant="outline"
            size="sm"
            icon={<ArchiveRestore size={13} />}
            loading={restoring}
            onClick={async () => {
              setRestoring(true);
              await restore(entry.id);
              setRestoring(false);
            }}
          >
            {entry.disposition === "trash" ? "Restore from Trash" : "Restore from quarantine"}
          </Button>
        )}
      </div>
      {open && (
        <ul className="fade-in divide-y divide-border border-t border-border">
          {[...entry.projects]
            .sort((a, b) => b.bytesRecovered - a.bytesRecovered)
            .map((p) => {
              const stillKnown = projects.some((x) => x.id === p.projectId);
              return (
                <li key={p.projectId} className="px-4 py-2 text-[12.5px]">
                  <div className="flex items-center gap-2">
                    <button type="button" onClick={() => toggle(p.projectId)} className="text-fg-subtle hover:text-fg" aria-label="Toggle details">
                      {details.has(p.projectId) ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
                    </button>
                    <span className="font-medium text-fg">{p.name}</span>
                    <span className="truncate font-mono text-[11px] text-fg-subtle">{p.path}</span>
                    <span className="flex-1" />
                    {p.errorCount > 0 && <AlertTriangle size={13} className="text-review" />}
                    <span className="tabular text-fg">{formatBytes(p.bytesRecovered)}</span>
                    {stillKnown && (
                      <Button variant="ghost" size="sm" icon={<FolderOpen size={12} />} onClick={() => openFolder(p.projectId)}>
                        Open
                      </Button>
                    )}
                  </div>
                  {details.has(p.projectId) && (
                    <ul className="ml-6 mt-1 flex flex-col gap-0.5 text-[12px]">
                      {p.artifacts.map((a) => (
                        <li key={a.path} className="flex items-center justify-between gap-3">
                          <span className="font-mono text-fg-muted">{a.relativePath}</span>
                          <span className={`tabular ${outcomeIsError(a.outcome) ? "text-review" : "text-fg-subtle"}`}>
                            {formatBytes(a.bytes)} · {describeOutcome(a.outcome)}
                          </span>
                        </li>
                      ))}
                    </ul>
                  )}
                </li>
              );
            })}
        </ul>
      )}
    </li>
  );
}

function QuarantinePanel() {
  const quarantine = useAppStore((s) => s.quarantine);
  const load = useAppStore((s) => s.loadQuarantine);
  const purge = useAppStore((s) => s.purgeQuarantine);
  const retention = useAppStore((s) => s.settings.quarantineRetentionDays);
  const [confirm, setConfirm] = useState<string | null>(null);
  useEffect(() => {
    load();
  }, [load]);
  if (!quarantine.length) return null;
  const total = quarantine.reduce((s, b) => s + b.bytes, 0);
  return (
    <section>
      <h3 className="mb-2 flex items-center justify-between text-[12px] font-semibold uppercase tracking-wide text-fg-muted">
        <span>Quarantine · {formatBytes(total)}</span>
        <span className="font-normal normal-case tracking-normal text-fg-subtle">
          Batches expire after {retention} day{retention === 1 ? "" : "s"}
        </span>
      </h3>
      <ul className="divide-y divide-border rounded-lg border border-border bg-surface">
        {quarantine.map((b) => (
          <li key={b.entryId} className="flex items-center gap-3 px-4 py-2.5 text-[12.5px]">
            <div className="min-w-0 flex-1">
              <div className="text-fg">{b.projects.length ? b.projects.join(", ") : "(empty)"}</div>
              <div className="truncate font-mono text-[11px] text-fg-subtle" title={b.path}>
                {b.date} · {b.entryId}
              </div>
            </div>
            <span className="tabular text-fg">{formatBytes(b.bytes)}</span>
            {confirm === b.entryId ? (
              <>
                <Button size="sm" variant="ghost" onClick={() => setConfirm(null)}>
                  Keep
                </Button>
                <Button
                  size="sm"
                  variant="danger"
                  icon={<ShieldAlert size={12} />}
                  onClick={() => {
                    setConfirm(null);
                    purge(b.entryId);
                  }}
                >
                  Delete permanently
                </Button>
              </>
            ) : (
              <Button size="sm" variant="ghost" icon={<Trash2 size={12} />} onClick={() => setConfirm(b.entryId)}>
                Purge
              </Button>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

export function HistoryPage() {
  const history = useAppStore((s) => s.history);
  const info = useAppStore((s) => s.info);
  const setPage = useAppStore((s) => s.setPage);
  if (!history.length) {
    return (
      <EmptyState
        icon={<History size={22} />}
        title="No cleanups yet"
        description="Each hibernate run is recorded here with what was removed, where it went and how much space came back."
        action={<Button onClick={() => setPage("projects")}>Go to Projects</Button>}
      />
    );
  }
  const lifetime = history.reduce((s, e) => s + e.totalRecovered, 0);
  const groups = new Map<string, HistoryEntry[]>();
  for (const e of history) {
    const day = formatDate(e.finishedAt);
    groups.set(day, [...(groups.get(day) ?? []), e]);
  }
  return (
    <div className="h-full overflow-y-auto p-5">
      <div className="mx-auto flex max-w-4xl flex-col gap-5">
        <div className="flex items-center justify-between text-[12.5px] text-fg-muted">
          <span>
            <span className="text-[18px] font-semibold tabular text-fg">{formatBytes(lifetime)}</span> recovered across {pluralize(history.length, "cleanup")}
          </span>
          {info && info.quarantineBytes > 0 && (
            <span className="flex items-center gap-1.5">
              <Trash2 size={13} /> {formatBytes(info.quarantineBytes)} in quarantine
            </span>
          )}
        </div>
        <QuarantinePanel />
        {[...groups.entries()].map(([day, entries]) => (
          <section key={day}>
            <h3 className="mb-2 text-[12px] font-semibold uppercase tracking-wide text-fg-muted">{day}</h3>
            <ul className="flex flex-col gap-2">
              {entries.map((e) => (
                <Entry key={e.id} entry={e} />
              ))}
            </ul>
          </section>
        ))}
      </div>
    </div>
  );
}
