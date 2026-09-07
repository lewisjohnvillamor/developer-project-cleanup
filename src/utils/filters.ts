import { type Project, type ProjectStatus, STACK_LABELS, type Stack } from "@/types";
import { daysSince } from "./format";

export type SortKey = "name" | "stack" | "status" | "lastActive" | "totalBytes" | "reclaimableBytes" | "safety";
export type SortDir = "asc" | "desc";

export interface ProjectFilters {
  search: string;
  stacks: Stack[];
  statuses: ProjectStatus[];
  inactiveDays: number | null; // "inactive for more than N days"
  minReclaimableBytes: number | null;
  minTotalBytes: number | null;
  gitClean: boolean | null; // true = only clean, false = only dirty
  notProtected: boolean;
  showIgnored: boolean;
  onlyHibernated: boolean;
}

export const EMPTY_FILTERS: ProjectFilters = {
  search: "",
  stacks: [],
  statuses: [],
  inactiveDays: null,
  minReclaimableBytes: null,
  minTotalBytes: null,
  gitClean: null,
  notProtected: false,
  showIgnored: false,
  onlyHibernated: false,
};

export function isIgnored(p: Project, now = Date.now()): boolean {
  return p.ignoredUntil !== null && Date.parse(p.ignoredUntil) > now;
}

export function activeFilterCount(f: ProjectFilters): number {
  let n = 0;
  if (f.stacks.length) n++;
  if (f.statuses.length) n++;
  if (f.inactiveDays !== null) n++;
  if (f.minReclaimableBytes !== null) n++;
  if (f.minTotalBytes !== null) n++;
  if (f.gitClean !== null) n++;
  if (f.notProtected) n++;
  if (f.showIgnored) n++;
  if (f.onlyHibernated) n++;
  return n;
}

export function applyFilters(projects: Project[], f: ProjectFilters, now = Date.now()): Project[] {
  const q = f.search.trim().toLowerCase();
  return projects.filter((p) => {
    if (!f.showIgnored && isIgnored(p, now)) return false;
    if (q && !p.name.toLowerCase().includes(q) && !p.path.toLowerCase().includes(q)) return false;
    if (f.stacks.length && !p.stacks.some((s) => f.stacks.includes(s))) return false;
    if (f.statuses.length && !f.statuses.includes(p.status)) return false;
    if (f.inactiveDays !== null) {
      const d = daysSince(p.lastActivityAt, now);
      if (d !== null && d <= f.inactiveDays) return false;
    }
    if (f.minReclaimableBytes !== null && p.reclaimableBytes < f.minReclaimableBytes) return false;
    if (f.minTotalBytes !== null && p.totalBytes < f.minTotalBytes) return false;
    if (f.gitClean !== null) {
      const clean = p.git?.state === "clean" || p.git?.state === "remoteMissing";
      const dirty = p.git?.state === "modified" || p.git?.state === "untracked";
      if (f.gitClean && !clean) return false;
      if (!f.gitClean && !dirty) return false;
    }
    if (f.notProtected && p.protected) return false;
    if (f.onlyHibernated && p.status !== "hibernated") return false;
    return true;
  });
}

const STATUS_ORDER: Record<ProjectStatus, number> = { dormant: 0, active: 1, hibernated: 2, protected: 3 };
const SAFETY_ORDER = { safe: 0, review: 1, protected: 2 } as const;

export function sortProjects(projects: Project[], key: SortKey, dir: SortDir): Project[] {
  const sign = dir === "asc" ? 1 : -1;
  const cmp = (a: Project, b: Project): number => {
    switch (key) {
      case "name":
        return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
      case "stack":
        return primaryLabel(a).localeCompare(primaryLabel(b));
      case "status":
        return STATUS_ORDER[a.status] - STATUS_ORDER[b.status];
      case "lastActive": {
        const ta = a.lastActivityAt ? Date.parse(a.lastActivityAt) : 0;
        const tb = b.lastActivityAt ? Date.parse(b.lastActivityAt) : 0;
        return ta - tb;
      }
      case "totalBytes":
        return a.totalBytes - b.totalBytes;
      case "reclaimableBytes":
        return a.reclaimableBytes - b.reclaimableBytes;
      case "safety":
        return SAFETY_ORDER[a.safety] - SAFETY_ORDER[b.safety];
    }
  };
  return [...projects].sort((a, b) => {
    const r = cmp(a, b) * sign;
    return r !== 0 ? r : a.name.localeCompare(b.name);
  });
}

export function primaryLabel(p: Project): string {
  const fw = p.frameworks.find((f) => f !== "TypeScript");
  if (fw) return fw;
  const first = p.stacks[0];
  if (!first) return "Unknown";
  const labels = STACK_LABELS;
  return labels[first];
}

/** Projects eligible for bulk selection: never protected, never ignored. */
export function bulkEligible(p: Project, now = Date.now()): boolean {
  return !p.protected && !isIgnored(p, now) && p.reclaimableBytes + p.reviewBytes > 0;
}
