import { useMemo } from "react";
import { useAppStore } from "@/stores/app-store";
import { applyFilters, sortProjects } from "@/utils/filters";

/** Filtered + sorted projects for the table, memoised on the inputs. */
export function useProjectList() {
  const projects = useAppStore((s) => s.projects);
  const filters = useAppStore((s) => s.filters);
  const sortKey = useAppStore((s) => s.sortKey);
  const sortDir = useAppStore((s) => s.sortDir);
  return useMemo(() => sortProjects(applyFilters(projects, filters), sortKey, sortDir), [projects, filters, sortKey, sortDir]);
}
