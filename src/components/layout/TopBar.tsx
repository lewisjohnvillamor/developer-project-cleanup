import { Button } from "@/components/common/Button";
import { Kbd, Menu, ProgressBar } from "@/components/common/Controls";
import { useAppStore } from "@/stores/app-store";
import { formatRelative } from "@/utils/format";
import { ChevronDown, Command, Download, FolderPlus, Loader2, RefreshCw, Search, X } from "lucide-react";

const TITLES: Record<string, string> = {
  overview: "Overview",
  projects: "Developer Projects",
  history: "History",
  settings: "Settings",
};

export function TopBar() {
  const page = useAppStore((s) => s.page);
  const scan = useAppStore((s) => s.scan);
  const startScan = useAppStore((s) => s.startScan);
  const cancelScan = useAppStore((s) => s.cancelScan);
  const roots = useAppStore((s) => s.settings.scanRoots);
  const scannedAt = useAppStore((s) => s.scannedAt);
  const setAddFoldersOpen = useAppStore((s) => s.setAddFoldersOpen);
  const setPaletteOpen = useAppStore((s) => s.setPaletteOpen);
  const exportProjects = useAppStore((s) => s.exportProjects);
  const hibernating = useAppStore((s) => s.hibernate.stage === "running");
  const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);

  const summaryCacheHits = useAppStore((s) => s.summary?.cacheHits ?? 0);
  const progress = scan.discovered > 0 ? scan.scanned / scan.discovered : null;

  return (
    <header className="flex h-12 shrink-0 items-center justify-between border-b border-border bg-surface px-5">
      <div className="flex min-w-0 items-center gap-3">
        <h1 className="text-[14px] font-semibold text-fg">{TITLES[page]}</h1>
        {scan.running ? (
          <div className="flex items-center gap-2 text-[12px] text-fg-muted">
            <Loader2 size={13} className="animate-spin text-accent" />
            <span className="tabular">{scan.scanned === 0 ? `Found ${scan.discovered} projects…` : `Measured ${scan.scanned} of ${scan.discovered}`}</span>
            <ProgressBar value={progress} className="w-32" />
            {scan.currentName && <span className="max-w-48 truncate font-mono text-[11px] text-fg-subtle">{scan.currentName}</span>}
          </div>
        ) : scannedAt ? (
          <span className="text-[12px] text-fg-subtle">
            Last scan {formatRelative(scannedAt).toLowerCase()}
            {roots.length ? ` · ${roots.length} folder${roots.length === 1 ? "" : "s"}` : ""}
            {summaryCacheHits > 0 ? ` · ${summaryCacheHits} size${summaryCacheHits === 1 ? "" : "s"} reused` : ""}
          </span>
        ) : null}
      </div>
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="sm" icon={<Command size={14} />} onClick={() => setPaletteOpen(true)} title="Command palette">
          <span className="hidden lg:inline">Commands</span> <Kbd>{isMac ? "⌘" : "Ctrl"} K</Kbd>
        </Button>
        <Button variant="ghost" size="sm" icon={<FolderPlus size={14} />} onClick={() => setAddFoldersOpen(true)}>
          Folders
        </Button>
        {scan.running ? (
          <Button variant="outline" size="sm" icon={<X size={14} />} onClick={() => cancelScan()}>
            Cancel scan
          </Button>
        ) : (
          <div className="flex items-stretch">
            <Button
              variant="primary"
              size="sm"
              icon={<Search size={14} />}
              onClick={() => startScan()}
              disabled={hibernating}
              title={hibernating ? "Wait for the cleanup to finish" : "Scan (reuses cached sizes for unchanged folders)"}
              className="rounded-r-none"
            >
              Scan Projects
            </Button>
            <Menu
              align="right"
              trigger={() => (
                <Button
                  variant="primary"
                  size="sm"
                  disabled={hibernating}
                  className="rounded-l-none border-l border-l-white/20 px-1.5"
                  title="More scan options"
                >
                  <ChevronDown size={13} />
                </Button>
              )}
              items={[
                { label: "Full rescan", icon: <RefreshCw size={13} />, hint: "ignore cached sizes", onSelect: () => startScan(undefined, true) },
                { separator: true, label: "" },
                { label: "Export as CSV", icon: <Download size={13} />, onSelect: () => exportProjects("csv") },
                { label: "Export as JSON", icon: <Download size={13} />, onSelect: () => exportProjects("json") },
              ]}
            />
          </div>
        )}
      </div>
    </header>
  );
}
