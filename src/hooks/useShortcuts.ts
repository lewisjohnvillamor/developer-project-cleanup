import { useEffect } from "react";
import { useAppStore } from "@/stores/app-store";
import { applyFilters, bulkEligible } from "@/utils/filters";

function inEditable(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable;
}

/**
 * Global keyboard shortcuts:
 *   Ctrl/Cmd+K  command palette      /        focus search (Projects)
 *   Ctrl/Cmd+A  select all visible   H        review & hibernate selection
 *   P           protect/unprotect the open project
 *   1–4         switch pages         Esc      close drawer / clear selection
 */
export function useShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useAppStore.getState();
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === "k") {
        e.preventDefault();
        s.setPaletteOpen(!s.paletteOpen);
        return;
      }
      if (s.paletteOpen || s.hibernate.stage !== "idle" || s.wake.stage !== "idle" || s.addFoldersOpen) return;
      if (inEditable(e.target)) return;

      if (mod && e.key.toLowerCase() === "a" && s.page === "projects") {
        e.preventDefault();
        const visible = applyFilters(s.projects, s.filters).filter((p) => bulkEligible(p));
        s.selectMany(visible.map((p) => p.id));
        return;
      }
      if (mod) return;
      switch (e.key) {
        case "/":
          if (s.page === "projects") {
            e.preventDefault();
            document.getElementById("project-search")?.focus();
          }
          break;
        case "h":
        case "H":
          if (s.selection.size) {
            e.preventDefault();
            s.reviewHibernate([...s.selection]);
          } else if (s.drawerProjectId) {
            e.preventDefault();
            s.reviewHibernate([s.drawerProjectId]);
          }
          break;
        case "p":
        case "P":
          if (s.drawerProjectId) {
            e.preventDefault();
            const p = s.projects.find((x) => x.id === s.drawerProjectId);
            if (p) s.setProtected(p.id, !p.protected);
          }
          break;
        case "1":
        case "2":
        case "3":
        case "4":
          s.setPage((["overview", "projects", "history", "settings"] as const)[Number(e.key) - 1]!);
          break;
        case "Escape":
          if (s.drawerProjectId) s.closeDrawer();
          else if (s.selection.size) s.clearSelection();
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
