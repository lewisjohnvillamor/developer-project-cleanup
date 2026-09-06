import { memo } from "react";
import { Archive, ArchiveRestore, ArrowDown, ArrowUp, EyeOff, FolderSearch, Shield } from "lucide-react";
import { Button } from "@/components/common/Button";
import { Checkbox, EmptyState } from "@/components/common/Controls";
import { SafetyBadge, StatusBadge, Badge } from "@/components/common/Badge";
import { useAppStore } from "@/stores/app-store";
import type { Project } from "@/types";
import { STACK_LABELS } from "@/types";
import { bulkEligible, isIgnored, primaryLabel, type SortKey } from "@/utils/filters";
import { formatBytes, formatDays, formatRelative } from "@/utils/format";

const COLUMNS: { key: SortKey | null; label: string; className: string }[] = [
  { key: null, label: "", className: "w-8" },
  { key: "name", label: "Project", className: "" },
  { key: "stack", label: "Stack", className: "w-32" },
  { key: "status", label: "Status", className: "w-28" },
  { key: "lastActive", label: "Last active", className: "w-24 text-right" },
  { key: "totalBytes", label: "Total", className: "w-24 text-right" },
  { key: "reclaimableBytes", label: "Reclaimable", className: "w-28 text-right" },
  { key: "safety", label: "Safety", className: "w-20" },
  { key: null, label: "", className: "w-28" },
];

export function ProjectTable({ projects }: { projects: Project[] }) {
  const sortKey = useAppStore((s) => s.sortKey);
  const sortDir = useAppStore((s) => s.sortDir);
  const setSort = useAppStore((s) => s.setSort);
  const selection = useAppStore((s) => s.selection);
  const selectMany = useAppStore((s) => s.selectMany);
  const deselectMany = useAppStore((s) => s.deselectMany);
  const total = useAppStore((s) => s.projects.length);
  const scanning = useAppStore((s) => s.scan.running);
  const resetFilters = useAppStore((s) => s.resetFilters);
  const startScan = useAppStore((s) => s.startScan);

  const eligible = projects.filter((p) => bulkEligible(p));
  const allSelected = eligible.length > 0 && eligible.every((p) => selection.has(p.id));
  const someSelected = eligible.some((p) => selection.has(p.id));

  if (!projects.length) {
    return total === 0 ? (
      <EmptyState
        icon={<FolderSearch size={22} />}
        title={scanning ? "Scanning…" : "No projects scanned yet"}
        description={scanning ? "Projects appear here as soon as they are measured." : "Add the folder where you keep your repositories and run a scan. Nothing is removed until you review and confirm."}
        action={!scanning ? <Button variant="primary" onClick={() => startScan()}>Scan Projects</Button> : undefined}
      />
    ) : (
      <EmptyState icon={<FolderSearch size={22} />} title="No projects match these filters" description="Try clearing a filter or two." action={<Button onClick={resetFilters}>Clear filters</Button>} />
    );
  }

  return (
    <table className="w-full border-collapse text-[12.5px]">
      <thead className="sticky top-0 z-10 bg-surface shadow-[0_1px_0_var(--border)]">
        <tr>
          {COLUMNS.map((c, i) => (
            <th key={i} className={`h-8 px-3 text-left text-[11px] font-semibold uppercase tracking-wide text-fg-muted ${c.className}`}>
              {i === 0 ? (
                <Checkbox checked={allSelected} indeterminate={!allSelected && someSelected} onChange={(v) => (v ? selectMany(eligible.map((p) => p.id)) : deselectMany(projects.map((p) => p.id)))} title="Select all visible" />
              ) : c.key ? (
                <button type="button" onClick={() => setSort(c.key!)} className={`inline-flex items-center gap-1 hover:text-fg ${sortKey === c.key ? "text-fg" : ""}`}>
                  {c.label}
                  {sortKey === c.key && (sortDir === "asc" ? <ArrowUp size={11} /> : <ArrowDown size={11} />)}
                </button>
              ) : (
                c.label
              )}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {projects.map((p) => (
          <Row key={p.id} project={p} selected={selection.has(p.id)} />
        ))}
      </tbody>
    </table>
  );
}

const Row = memo(function Row({ project: p, selected }: { project: Project; selected: boolean }) {
  const toggleSelect = useAppStore((s) => s.toggleSelect);
  const openDrawer = useAppStore((s) => s.openDrawer);
  const drawerId = useAppStore((s) => s.drawerProjectId);
  const reviewHibernate = useAppStore((s) => s.reviewHibernate);
  const openWake = useAppStore((s) => s.openWake);
  const unignore = useAppStore((s) => s.unignoreProject);
  const ignored = isIgnored(p);
  const eligible = bulkEligible(p);
  const label = primaryLabel(p);
  const extra = p.stacks.length > 1 ? p.stacks.map((s) => STACK_LABELS[s]).join(" + ") : p.frameworks.filter((f) => f !== label).slice(0, 2).join(" · ");

  return (
    <tr
      onClick={() => openDrawer(p.id)}
      className={`group h-10 cursor-pointer border-b border-border transition-colors ${drawerId === p.id ? "bg-accent-soft/60" : selected ? "bg-accent-soft/30" : "hover:bg-surface-2"} ${ignored ? "opacity-60" : ""}`}
    >
      <td className="px-3">
        <Checkbox checked={selected} onChange={() => toggleSelect(p.id)} disabled={!eligible} title={p.protected ? "Protected projects are never bulk-selected" : ignored ? "Ignored" : undefined} />
      </td>
      <td className="max-w-0 px-3">
        <div className="flex items-center gap-2">
          <span className="truncate font-medium text-fg">{p.name}</span>
          {p.protected && <Shield size={12} className="shrink-0 text-danger" />}
          {ignored && (
            <Badge tone="inactive" icon={<EyeOff size={10} />}>
              Ignored
            </Badge>
          )}
          {p.parentPath && <Badge tone="neutral">nested</Badge>}
        </div>
        <div className="truncate font-mono text-[11px] text-fg-subtle">{p.path}</div>
      </td>
      <td className="px-3">
        <div className="text-fg">{label}</div>
        {extra && <div className="truncate text-[11px] text-fg-subtle">{extra}</div>}
      </td>
      <td className="px-3">
        <StatusBadge status={p.status} />
      </td>
      <td className="px-3 text-right tabular text-fg-muted" title={formatRelative(p.lastActivityAt)}>
        {formatDays(p.lastActivityAt)}
      </td>
      <td className="px-3 text-right tabular text-fg-muted">{formatBytes(p.totalBytes)}</td>
      <td className="px-3 text-right tabular">
        <span className={p.reclaimableBytes > 0 ? "font-semibold text-fg" : "text-fg-subtle"}>{formatBytes(p.reclaimableBytes)}</span>
        {p.reviewBytes > 0 && <div className="text-[10.5px] text-review">+{formatBytes(p.reviewBytes)} review</div>}
      </td>
      <td className="px-3">
        <SafetyBadge safety={p.safety} reasons={p.safetyReasons} />
      </td>
      <td className="px-3 text-right" onClick={(e) => e.stopPropagation()}>
        {ignored ? (
          <Button size="sm" variant="ghost" onClick={() => unignore(p.id)}>
            Restore
          </Button>
        ) : p.status === "hibernated" ? (
          <Button size="sm" variant="outline" icon={<ArchiveRestore size={13} />} onClick={() => openWake(p.id)}>
            Wake
          </Button>
        ) : p.protected ? (
          <span className="text-[11px] text-fg-subtle">—</span>
        ) : p.reclaimableBytes + p.reviewBytes > 0 ? (
          <Button size="sm" variant="outline" icon={<Archive size={13} />} onClick={() => reviewHibernate([p.id])} className="opacity-70 group-hover:opacity-100">
            Hibernate
          </Button>
        ) : (
          <span className="text-[11px] text-fg-subtle">Nothing to clean</span>
        )}
      </td>
    </tr>
  );
});
