import { Archive, ArchiveRestore, ArrowDown, ArrowUp, EyeOff, FolderSearch, Shield } from "lucide-react";
import { memo, useCallback, useEffect, useRef, useState } from "react";
import { Badge, SafetyBadge, StatusBadge } from "@/components/common/Badge";
import { Button } from "@/components/common/Button";
import { Checkbox, EmptyState } from "@/components/common/Controls";
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

/** Fixed row height (h-10) that the windowing math relies on. */
const ROW_HEIGHT = 40;
const OVERSCAN = 12;

/**
 * Track the visible row window of a scroll container. Returns the index
 * range to render; rows outside it are replaced by spacer rows so the scroll
 * bar and keyboard navigation behave as if everything were rendered.
 */
function useRowWindow(scrollRef: React.RefObject<HTMLElement | null>, count: number) {
  const [range, setRange] = useState<[number, number]>([0, Math.min(count, 60)]);
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    let frame = 0;
    const update = () => {
      frame = 0;
      const top = el.scrollTop;
      const height = el.clientHeight || 800;
      const start = Math.max(0, Math.floor(top / ROW_HEIGHT) - OVERSCAN);
      const end = Math.min(count, Math.ceil((top + height) / ROW_HEIGHT) + OVERSCAN);
      setRange((r) => (r[0] === start && r[1] === end ? r : [start, end]));
    };
    const onScroll = () => {
      if (!frame) frame = requestAnimationFrame(update);
    };
    update();
    el.addEventListener("scroll", onScroll, { passive: true });
    const ro = new ResizeObserver(onScroll);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", onScroll);
      ro.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  }, [scrollRef, count]);
  return range;
}

export function ProjectTable({ projects, scrollRef }: { projects: Project[]; scrollRef: React.RefObject<HTMLDivElement | null> }) {
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
  const bodyRef = useRef<HTMLTableSectionElement>(null);
  const [start, end] = useRowWindow(scrollRef, projects.length);
  const drawerId = useAppStore((s) => s.drawerProjectId);
  const openDrawer = useAppStore((s) => s.openDrawer);
  const toggleSelect = useAppStore((s) => s.toggleSelect);

  // Arrow keys move the focused row, Enter opens it, Space selects it.
  const onKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTableSectionElement>) => {
      const rows = Array.from(bodyRef.current?.querySelectorAll<HTMLTableRowElement>("tr[data-id]") ?? []);
      if (!rows.length) return;
      const active = document.activeElement as HTMLElement | null;
      const idx = active ? rows.indexOf(active as HTMLTableRowElement) : -1;
      if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Home" || e.key === "End") {
        e.preventDefault();
        const next =
          e.key === "Home" ? 0 : e.key === "End" ? rows.length - 1 : e.key === "ArrowDown" ? Math.min(rows.length - 1, idx + 1) : Math.max(0, idx - 1);
        rows[next]?.focus();
        rows[next]?.scrollIntoView({ block: "nearest" });
      } else if (idx >= 0 && (e.key === "Enter" || e.key === " ")) {
        e.preventDefault();
        const id = rows[idx]!.dataset.id!;
        if (e.key === "Enter") openDrawer(id);
        else if (rows[idx]!.dataset.eligible === "1") toggleSelect(id);
      }
    },
    [openDrawer, toggleSelect, projects, scrollRef],
  );

  // Focus a row that was outside the rendered window once it appears.
  const pendingFocus = useRef<string | null>(null);
  useEffect(() => {
    if (!pendingFocus.current) return;
    const row = bodyRef.current?.querySelector<HTMLTableRowElement>(`tr[data-id="${pendingFocus.current}"]`);
    if (row) {
      row.focus();
      pendingFocus.current = null;
    }
  });

  useEffect(() => {
    if (!drawerId) return;
    const idx = projects.findIndex((p) => p.id === drawerId);
    const el = scrollRef.current;
    if (idx >= 0 && el && (idx < start || idx >= end)) el.scrollTop = idx * ROW_HEIGHT - el.clientHeight / 2;
    const row = bodyRef.current?.querySelector<HTMLTableRowElement>(`tr[data-id="${drawerId}"]`);
    row?.scrollIntoView({ block: "nearest" });
  }, [drawerId, projects, scrollRef, start, end]);

  if (!projects.length) {
    return total === 0 ? (
      <EmptyState
        icon={<FolderSearch size={22} />}
        title={scanning ? "Scanning…" : "No projects scanned yet"}
        description={
          scanning
            ? "Projects appear here as soon as they are measured."
            : "Add the folder where you keep your repositories and run a scan. Nothing is removed until you review and confirm."
        }
        action={
          !scanning ? (
            <Button variant="primary" onClick={() => startScan()}>
              Scan Projects
            </Button>
          ) : undefined
        }
      />
    ) : (
      <EmptyState
        icon={<FolderSearch size={22} />}
        title="No projects match these filters"
        description="Try clearing a filter or two."
        action={<Button onClick={resetFilters}>Clear filters</Button>}
      />
    );
  }

  return (
    <table className="w-full border-collapse text-[12.5px]">
      <thead className="sticky top-0 z-10 bg-surface shadow-[0_1px_0_var(--border)]">
        <tr>
          {COLUMNS.map((c, i) => (
            <th key={i} className={`h-8 px-3 text-left text-[11px] font-semibold uppercase tracking-wide text-fg-muted ${c.className}`}>
              {i === 0 ? (
                <Checkbox
                  checked={allSelected}
                  indeterminate={!allSelected && someSelected}
                  onChange={(v) => (v ? selectMany(eligible.map((p) => p.id)) : deselectMany(projects.map((p) => p.id)))}
                  title="Select all visible"
                />
              ) : c.key ? (
                <button
                  type="button"
                  onClick={() => setSort(c.key!)}
                  className={`inline-flex items-center gap-1 hover:text-fg ${sortKey === c.key ? "text-fg" : ""}`}
                >
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
      <tbody ref={bodyRef} onKeyDown={onKeyDown}>
        {start > 0 && (
          <tr>
            <td colSpan={COLUMNS.length} style={{ height: start * ROW_HEIGHT, padding: 0, border: 0 }} />
          </tr>
        )}
        {projects.slice(start, end).map((p, i) => (
          <Row key={p.id} project={p} selected={selection.has(p.id)} first={start + i === 0} />
        ))}
        {end < projects.length && (
          <tr>
            <td colSpan={COLUMNS.length} style={{ height: (projects.length - end) * ROW_HEIGHT, padding: 0, border: 0 }} />
          </tr>
        )}
      </tbody>
    </table>
  );
}

const Row = memo(function Row({ project: p, selected, first }: { project: Project; selected: boolean; first: boolean }) {
  const toggleSelect = useAppStore((s) => s.toggleSelect);
  const openDrawer = useAppStore((s) => s.openDrawer);
  const drawerId = useAppStore((s) => s.drawerProjectId);
  const reviewHibernate = useAppStore((s) => s.reviewHibernate);
  const openWake = useAppStore((s) => s.openWake);
  const unignore = useAppStore((s) => s.unignoreProject);
  const ignored = isIgnored(p);
  const eligible = bulkEligible(p);
  const label = primaryLabel(p);
  const extra =
    p.stacks.length > 1
      ? p.stacks.map((s) => STACK_LABELS[s]).join(" + ")
      : p.frameworks
          .filter((f) => f !== label)
          .slice(0, 2)
          .join(" · ");

  return (
    <tr
      data-id={p.id}
      data-eligible={eligible ? "1" : "0"}
      tabIndex={first || drawerId === p.id ? 0 : -1}
      aria-selected={selected}
      onClick={() => openDrawer(p.id)}
      className={`group h-10 cursor-pointer border-b border-border outline-none transition-colors focus-visible:shadow-[inset_0_0_0_2px_var(--ring)] ${drawerId === p.id ? "bg-accent-soft/60" : selected ? "bg-accent-soft/30" : "hover:bg-surface-2"} ${ignored ? "opacity-60" : ""}`}
    >
      <td className="px-3">
        <Checkbox
          checked={selected}
          onChange={() => toggleSelect(p.id)}
          disabled={!eligible}
          title={p.protected ? "Protected projects are never bulk-selected" : ignored ? "Ignored" : undefined}
        />
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
          <Button
            size="sm"
            variant="outline"
            icon={<Archive size={13} />}
            onClick={() => reviewHibernate([p.id])}
            className="opacity-70 group-hover:opacity-100"
          >
            Hibernate
          </Button>
        ) : (
          <span className="text-[11px] text-fg-subtle">Nothing to clean</span>
        )}
      </td>
    </tr>
  );
});
