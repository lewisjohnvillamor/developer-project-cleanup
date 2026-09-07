import { type Page, useAppStore } from "@/stores/app-store";
import { FolderOpen, History, LayoutDashboard, Moon, Settings } from "lucide-react";

const ITEMS: { page: Page; label: string; icon: typeof LayoutDashboard }[] = [
  { page: "overview", label: "Overview", icon: LayoutDashboard },
  { page: "projects", label: "Projects", icon: FolderOpen },
  { page: "history", label: "History", icon: History },
  { page: "settings", label: "Settings", icon: Settings },
];

export function Sidebar() {
  const page = useAppStore((s) => s.page);
  const setPage = useAppStore((s) => s.setPage);
  const info = useAppStore((s) => s.info);
  const projectCount = useAppStore((s) => s.projects.length);
  return (
    <aside className="flex w-48 shrink-0 flex-col border-r border-border bg-surface">
      <div className="flex h-12 items-center gap-2 px-4">
        <span className="flex h-6 w-6 items-center justify-center rounded-md bg-accent text-accent-fg">
          <Moon size={13} strokeWidth={2.5} />
        </span>
        <span className="text-[13px] font-semibold tracking-tight text-fg">Project Hibernate</span>
      </div>
      <nav className="flex flex-col gap-0.5 px-2 pt-1">
        {ITEMS.map(({ page: p, label, icon: Icon }) => (
          <button
            key={p}
            type="button"
            onClick={() => setPage(p)}
            aria-current={page === p ? "page" : undefined}
            className={`flex h-8 items-center gap-2.5 rounded-md px-2.5 text-[13px] transition-colors ${
              page === p ? "bg-surface-2 font-medium text-fg" : "text-fg-muted hover:bg-surface-2 hover:text-fg"
            }`}
          >
            <Icon size={15} strokeWidth={page === p ? 2.2 : 1.8} />
            <span className="flex-1 text-left">{label}</span>
            {p === "projects" && projectCount > 0 && <span className="rounded bg-surface-3 px-1.5 text-[10px] tabular text-fg-muted">{projectCount}</span>}
          </button>
        ))}
      </nav>
      <div className="mt-auto px-4 py-3 text-[11px] text-fg-subtle">
        <div>v{info?.version ?? "…"}</div>
        <div className="mt-0.5">Local only. Nothing leaves this machine.</div>
      </div>
    </aside>
  );
}
