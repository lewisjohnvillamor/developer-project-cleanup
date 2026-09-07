import { useAppStore } from "@/stores/app-store";
import { primaryLabel } from "@/utils/filters";
import { formatBytes } from "@/utils/format";
import { Archive, Download, FolderOpen, FolderPlus, History, LayoutDashboard, Monitor, Moon, RefreshCw, Search, Settings, Shield, Sun } from "lucide-react";
import { type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { Kbd } from "./Controls";

interface Item {
  id: string;
  label: string;
  hint?: string;
  icon: ReactNode;
  group: "Actions" | "Pages" | "Projects";
  run: () => void;
}

export function CommandPalette() {
  const open = useAppStore((s) => s.paletteOpen);
  const setOpen = useAppStore((s) => s.setPaletteOpen);
  const projects = useAppStore((s) => s.projects);
  const selection = useAppStore((s) => s.selection);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setActive(0);
      setTimeout(() => input.current?.focus(), 0);
    }
  }, [open]);

  const items = useMemo<Item[]>(() => {
    const s = useAppStore.getState();
    const actions: Item[] = [
      { id: "scan", label: "Scan projects", hint: "reuse cached sizes", icon: <Search size={14} />, group: "Actions", run: () => s.startScan() },
      {
        id: "rescan",
        label: "Full rescan",
        hint: "measure everything again",
        icon: <RefreshCw size={14} />,
        group: "Actions",
        run: () => s.startScan(undefined, true),
      },
      { id: "folders", label: "Choose project folders", icon: <FolderPlus size={14} />, group: "Actions", run: () => s.setAddFoldersOpen(true) },
      {
        id: "hib",
        label: "Review & hibernate selection",
        hint: selection.size ? `${selection.size} selected` : "nothing selected",
        icon: <Archive size={14} />,
        group: "Actions",
        run: () => selection.size && s.reviewHibernate([...selection]),
      },
      { id: "export-csv", label: "Export projects as CSV", icon: <Download size={14} />, group: "Actions", run: () => s.exportProjects("csv") },
      { id: "export-json", label: "Export projects as JSON", icon: <Download size={14} />, group: "Actions", run: () => s.exportProjects("json") },
      { id: "theme-light", label: "Theme: light", icon: <Sun size={14} />, group: "Actions", run: () => s.saveSettings({ theme: "light" }) },
      { id: "theme-dark", label: "Theme: dark", icon: <Moon size={14} />, group: "Actions", run: () => s.saveSettings({ theme: "dark" }) },
      { id: "theme-system", label: "Theme: system", icon: <Monitor size={14} />, group: "Actions", run: () => s.saveSettings({ theme: "system" }) },
    ];
    const pages: Item[] = [
      { id: "p-overview", label: "Overview", hint: "1", icon: <LayoutDashboard size={14} />, group: "Pages", run: () => s.setPage("overview") },
      { id: "p-projects", label: "Projects", hint: "2", icon: <FolderOpen size={14} />, group: "Pages", run: () => s.setPage("projects") },
      { id: "p-history", label: "History", hint: "3", icon: <History size={14} />, group: "Pages", run: () => s.setPage("history") },
      { id: "p-settings", label: "Settings", hint: "4", icon: <Settings size={14} />, group: "Pages", run: () => s.setPage("settings") },
    ];
    const projs: Item[] = projects.map((p) => ({
      id: `proj-${p.id}`,
      label: p.name,
      hint: `${primaryLabel(p)} · ${formatBytes(p.reclaimableBytes)}${p.protected ? " · protected" : ""}`,
      icon: p.protected ? <Shield size={14} className="text-danger" /> : <FolderOpen size={14} />,
      group: "Projects",
      run: () => {
        s.setPage("projects");
        s.openDrawer(p.id);
      },
    }));
    return [...actions, ...pages, ...projs];
  }, [projects, selection]);

  const q = query.trim().toLowerCase();
  const filtered = useMemo(() => {
    if (!q) return items.filter((i) => i.group !== "Projects").concat(items.filter((i) => i.group === "Projects").slice(0, 6));
    const score = (i: Item) => {
      const l = i.label.toLowerCase();
      if (l.startsWith(q)) return 0;
      if (l.includes(q)) return 1;
      if (i.hint?.toLowerCase().includes(q)) return 2;
      return -1;
    };
    return items
      .map((i) => [i, score(i)] as const)
      .filter(([, sc]) => sc >= 0)
      .sort((a, b) => a[1] - b[1])
      .map(([i]) => i)
      .slice(0, 40);
  }, [items, q]);

  useEffect(() => setActive(0), [q]);

  if (!open) return null;

  const run = (item: Item) => {
    setOpen(false);
    item.run();
  };

  return (
    <div className="fixed inset-0 z-[70] flex items-start justify-center bg-black/40 pt-[12vh]" onMouseDown={() => setOpen(false)} role="presentation">
      <div
        className="fade-in w-full max-w-lg overflow-hidden rounded-xl border border-border bg-surface shadow-panel"
        onMouseDown={(e) => e.stopPropagation()}
        role="dialog"
        aria-label="Command palette"
      >
        <div className="flex items-center gap-2 border-b border-border px-3">
          <Search size={15} className="text-fg-subtle" />
          <input
            ref={input}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setActive((a) => Math.min(filtered.length - 1, a + 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setActive((a) => Math.max(0, a - 1));
              } else if (e.key === "Enter") {
                const item = filtered[active];
                if (item) run(item);
              } else if (e.key === "Escape") {
                setOpen(false);
              }
            }}
            placeholder="Type a command or project name…"
            className="h-11 flex-1 bg-transparent text-[13.5px] text-fg outline-none placeholder:text-fg-subtle"
            aria-label="Search commands"
          />
          <Kbd>Esc</Kbd>
        </div>
        <div className="max-h-[50vh] overflow-y-auto p-1" role="listbox" tabIndex={-1} aria-label="Results">
          {filtered.length === 0 && <div className="px-3 py-6 text-center text-[12.5px] text-fg-muted">Nothing matches.</div>}
          {filtered.map((item, i) => {
            const showGroup = i === 0 || filtered[i - 1]!.group !== item.group;
            return (
              <div key={item.id}>
                {showGroup && <div className="px-2 pb-1 pt-2 text-[10.5px] font-semibold uppercase tracking-wide text-fg-subtle">{item.group}</div>}
                <button
                  type="button"
                  role="option"
                  aria-selected={i === active}
                  onMouseEnter={() => setActive(i)}
                  onClick={() => run(item)}
                  className={`flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left text-[13px] ${i === active ? "bg-accent-soft text-fg" : "text-fg"}`}
                >
                  <span className="text-fg-muted">{item.icon}</span>
                  <span className="flex-1 truncate">{item.label}</span>
                  {item.hint && <span className="truncate text-[11px] text-fg-subtle">{item.hint}</span>}
                </button>
              </div>
            );
          })}
        </div>
        <div className="flex items-center gap-3 border-t border-border px-3 py-1.5 text-[10.5px] text-fg-subtle">
          <span>
            <Kbd>↑↓</Kbd> navigate
          </span>
          <span>
            <Kbd>↵</Kbd> run
          </span>
          <span>
            <Kbd>/</Kbd> search projects
          </span>
          <span>
            <Kbd>H</Kbd> hibernate selection
          </span>
          <span>
            <Kbd>P</Kbd> protect
          </span>
        </div>
      </div>
    </div>
  );
}
