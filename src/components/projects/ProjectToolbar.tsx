import { useMemo, useState } from "react";
import { Check, CheckSquare, ChevronDown, Filter, Search, X } from "lucide-react";
import { Button } from "@/components/common/Button";
import { Menu, TextInput, Toggle } from "@/components/common/Controls";
import { useAppStore } from "@/stores/app-store";
import type { Project, ProjectStatus, Stack } from "@/types";
import { STACK_LABELS } from "@/types";
import { activeFilterCount, bulkEligible, EMPTY_FILTERS, type ProjectFilters } from "@/utils/filters";
import { daysSince } from "@/utils/format";

const GB = 1_000_000_000;
const MB = 1_000_000;

interface QuickFilter {
  label: string;
  active: (f: ProjectFilters) => boolean;
  apply: (f: ProjectFilters) => Partial<ProjectFilters>;
}

const QUICK: QuickFilter[] = [
  { label: "Inactive > 30 days", active: (f) => f.inactiveDays === 30, apply: (f) => ({ inactiveDays: f.inactiveDays === 30 ? null : 30 }) },
  { label: "Inactive > 90 days", active: (f) => f.inactiveDays === 90, apply: (f) => ({ inactiveDays: f.inactiveDays === 90 ? null : 90 }) },
  { label: "> 500 MB reclaimable", active: (f) => f.minReclaimableBytes === 500 * MB, apply: (f) => ({ minReclaimableBytes: f.minReclaimableBytes === 500 * MB ? null : 500 * MB }) },
  { label: "> 1 GB reclaimable", active: (f) => f.minReclaimableBytes === GB, apply: (f) => ({ minReclaimableBytes: f.minReclaimableBytes === GB ? null : GB }) },
  { label: "Git clean", active: (f) => f.gitClean === true, apply: (f) => ({ gitClean: f.gitClean === true ? null : true }) },
  { label: "Not protected", active: (f) => f.notProtected, apply: (f) => ({ notProtected: !f.notProtected }) },
];

const STATUSES: ProjectStatus[] = ["active", "dormant", "hibernated", "protected"];

function toggleIn<T>(list: T[], v: T): T[] {
  return list.includes(v) ? list.filter((x) => x !== v) : [...list, v];
}

export function ProjectToolbar({ visible }: { visible: Project[] }) {
  const filters = useAppStore((s) => s.filters);
  const setFilters = useAppStore((s) => s.setFilters);
  const resetFilters = useAppStore((s) => s.resetFilters);
  const projects = useAppStore((s) => s.projects);
  const selection = useAppStore((s) => s.selection);
  const selectMany = useAppStore((s) => s.selectMany);
  const deselectMany = useAppStore((s) => s.deselectMany);
  const clearSelection = useAppStore((s) => s.clearSelection);
  const [filtersOpen, setFiltersOpen] = useState(false);

  const stacksPresent = useMemo(() => {
    const set = new Set<Stack>();
    for (const p of projects) for (const s of p.stacks) set.add(s);
    return [...set].sort();
  }, [projects]);

  const eligible = visible.filter((p) => bulkEligible(p));
  const n = activeFilterCount(filters);

  return (
    <div className="flex flex-col gap-2 border-b border-border bg-surface px-4 py-2.5">
      <div className="flex items-center gap-2">
        <div className="relative w-72">
          <Search size={14} className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-fg-subtle" />
          <TextInput id="project-search" value={filters.search} onChange={(v) => setFilters({ search: v })} placeholder="Search projects or paths  ( / )" className="pl-8" />
          {filters.search && (
            <button type="button" onClick={() => setFilters({ search: "" })} className="absolute right-2 top-1/2 -translate-y-1/2 text-fg-subtle hover:text-fg" aria-label="Clear search">
              <X size={13} />
            </button>
          )}
        </div>
        <Button variant={filtersOpen || n ? "secondary" : "ghost"} size="md" icon={<Filter size={14} />} onClick={() => setFiltersOpen((v) => !v)}>
          Filters{n ? ` · ${n}` : ""}
        </Button>
        {n > 0 && (
          <Button variant="ghost" size="sm" onClick={resetFilters}>
            Clear
          </Button>
        )}
        <div className="flex-1" />
        <Menu
          align="right"
          trigger={(open) => (
            <Button variant={open ? "secondary" : "ghost"} size="md" icon={<CheckSquare size={14} />}>
              Select <ChevronDown size={13} />
            </Button>
          )}
          items={[
            { label: "Select all visible", hint: String(eligible.length), onSelect: () => selectMany(eligible.map((p) => p.id)) },
            { label: "Select dormant", hint: String(eligible.filter((p) => p.status === "dormant").length), onSelect: () => selectMany(eligible.filter((p) => p.status === "dormant").map((p) => p.id)) },
            { label: "Select > 1 GB reclaimable", hint: String(eligible.filter((p) => p.reclaimableBytes > GB).length), onSelect: () => selectMany(eligible.filter((p) => p.reclaimableBytes > GB).map((p) => p.id)) },
            { label: "Select inactive > 30 days", hint: String(eligible.filter((p) => (daysSince(p.lastActivityAt) ?? 9999) > 30).length), onSelect: () => selectMany(eligible.filter((p) => (daysSince(p.lastActivityAt) ?? 9999) > 30).map((p) => p.id)) },
            { separator: true, label: "" },
            { label: "Deselect protected", onSelect: () => deselectMany(projects.filter((p) => p.protected).map((p) => p.id)) },
            { label: "Clear selection", disabled: selection.size === 0, onSelect: clearSelection },
          ]}
        />
      </div>

      <div className="flex flex-wrap items-center gap-1.5">
        {QUICK.map((q) => {
          const on = q.active(filters);
          return (
            <button
              key={q.label}
              type="button"
              onClick={() => setFilters(q.apply(filters))}
              className={`inline-flex h-6 items-center gap-1 rounded-full border px-2.5 text-[11.5px] transition-colors ${on ? "border-accent bg-accent-soft text-accent" : "border-border text-fg-muted hover:bg-surface-2 hover:text-fg"}`}
            >
              {on && <Check size={11} />}
              {q.label}
            </button>
          );
        })}
        {(["node", "rust", "python"] as Stack[]).filter((s) => stacksPresent.includes(s)).map((s) => {
          const on = filters.stacks.includes(s);
          return (
            <button
              key={s}
              type="button"
              onClick={() => setFilters({ stacks: toggleIn(filters.stacks, s) })}
              className={`inline-flex h-6 items-center gap-1 rounded-full border px-2.5 text-[11.5px] transition-colors ${on ? "border-accent bg-accent-soft text-accent" : "border-border text-fg-muted hover:bg-surface-2 hover:text-fg"}`}
            >
              {on && <Check size={11} />}
              {STACK_LABELS[s]}
            </button>
          );
        })}
      </div>

      {filtersOpen && (
        <div className="fade-in grid grid-cols-4 gap-4 rounded-md border border-border bg-surface-2/50 p-3">
          <div>
            <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-fg-muted">Stack</div>
            <div className="flex flex-wrap gap-1">
              {stacksPresent.map((s) => (
                <button key={s} type="button" onClick={() => setFilters({ stacks: toggleIn(filters.stacks, s) })} className={`rounded border px-1.5 py-0.5 text-[11.5px] ${filters.stacks.includes(s) ? "border-accent bg-accent-soft text-accent" : "border-border text-fg-muted hover:text-fg"}`}>
                  {STACK_LABELS[s]}
                </button>
              ))}
            </div>
          </div>
          <div>
            <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-fg-muted">Status</div>
            <div className="flex flex-wrap gap-1">
              {STATUSES.map((s) => (
                <button key={s} type="button" onClick={() => setFilters({ statuses: toggleIn(filters.statuses, s) })} className={`rounded border px-1.5 py-0.5 text-[11.5px] capitalize ${filters.statuses.includes(s) ? "border-accent bg-accent-soft text-accent" : "border-border text-fg-muted hover:text-fg"}`}>
                  {s}
                </button>
              ))}
            </div>
          </div>
          <div>
            <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-fg-muted">Size</div>
            <div className="flex flex-col gap-1 text-[12px]">
              <label className="flex items-center gap-2">
                <span className="w-24 text-fg-muted">Reclaimable ≥</span>
                <select value={filters.minReclaimableBytes ?? ""} onChange={(e) => setFilters({ minReclaimableBytes: e.target.value ? Number(e.target.value) : null })} className="h-6 rounded border border-border bg-surface px-1 text-[12px]">
                  <option value="">any</option>
                  <option value={100 * MB}>100 MB</option>
                  <option value={500 * MB}>500 MB</option>
                  <option value={GB}>1 GB</option>
                  <option value={5 * GB}>5 GB</option>
                </select>
              </label>
              <label className="flex items-center gap-2">
                <span className="w-24 text-fg-muted">Total ≥</span>
                <select value={filters.minTotalBytes ?? ""} onChange={(e) => setFilters({ minTotalBytes: e.target.value ? Number(e.target.value) : null })} className="h-6 rounded border border-border bg-surface px-1 text-[12px]">
                  <option value="">any</option>
                  <option value={100 * MB}>100 MB</option>
                  <option value={GB}>1 GB</option>
                  <option value={5 * GB}>5 GB</option>
                </select>
              </label>
              <label className="flex items-center gap-2">
                <span className="w-24 text-fg-muted">Inactive &gt;</span>
                <select value={filters.inactiveDays ?? ""} onChange={(e) => setFilters({ inactiveDays: e.target.value ? Number(e.target.value) : null })} className="h-6 rounded border border-border bg-surface px-1 text-[12px]">
                  <option value="">any</option>
                  <option value={7}>7 days</option>
                  <option value={30}>30 days</option>
                  <option value={90}>90 days</option>
                  <option value={180}>180 days</option>
                  <option value={365}>1 year</option>
                </select>
              </label>
            </div>
          </div>
          <div>
            <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-fg-muted">Git &amp; visibility</div>
            <div className="flex flex-col text-[12px]">
              <label className="flex items-center gap-2 py-0.5">
                <span className="w-16 text-fg-muted">Git</span>
                <select value={filters.gitClean === null ? "" : filters.gitClean ? "clean" : "dirty"} onChange={(e) => setFilters({ gitClean: e.target.value === "" ? null : e.target.value === "clean" })} className="h-6 rounded border border-border bg-surface px-1 text-[12px]">
                  <option value="">any</option>
                  <option value="clean">clean</option>
                  <option value="dirty">uncommitted changes</option>
                </select>
              </label>
              <Toggle checked={filters.notProtected} onChange={(v) => setFilters({ notProtected: v })} label={<span className="text-[12px] font-normal">Hide protected</span>} />
              <Toggle checked={filters.onlyHibernated} onChange={(v) => setFilters({ onlyHibernated: v })} label={<span className="text-[12px] font-normal">Only hibernated</span>} />
              <Toggle checked={filters.showIgnored} onChange={(v) => setFilters({ showIgnored: v })} label={<span className="text-[12px] font-normal">Show ignored</span>} />
            </div>
          </div>
          <div className="col-span-4 flex justify-end">
            <Button variant="ghost" size="sm" onClick={() => setFilters(EMPTY_FILTERS)}>
              Reset filters
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
