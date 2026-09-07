import { StatusBadge } from "@/components/common/Badge";
import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { GlobalCaches } from "@/components/storage/GlobalCaches";
import { RecentCleanup } from "@/components/storage/RecentCleanup";
import { ReclaimTrend } from "@/components/storage/ReclaimTrend";
import { StorageBreakdown } from "@/components/storage/StorageBreakdown";
import { StorageSummary } from "@/components/storage/StorageSummary";
import { useAppStore } from "@/stores/app-store";
import { bulkEligible, primaryLabel } from "@/utils/filters";
import { formatBytes, formatRelative, pluralize } from "@/utils/format";
import { ArrowRight, FolderPlus, Moon, Search, ShieldCheck, Sparkles } from "lucide-react";

function Welcome() {
  const roots = useAppStore((s) => s.settings.scanRoots);
  const startScan = useAppStore((s) => s.startScan);
  const setAddFoldersOpen = useAppStore((s) => s.setAddFoldersOpen);
  return (
    <div className="flex h-full items-center justify-center p-8">
      <div className="fade-in w-full max-w-xl">
        <div className="mb-6 flex items-center gap-3">
          <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-accent text-accent-fg">
            <Moon size={20} strokeWidth={2.2} />
          </span>
          <div>
            <h2 className="text-[20px] font-semibold tracking-tight text-fg">Keep the project. Remove what can be rebuilt.</h2>
            <p className="text-[13px] text-fg-muted">Reclaim disk space from dormant projects without deleting a single source file.</p>
          </div>
        </div>
        <ol className="mb-6 grid grid-cols-3 gap-3 text-[12.5px]">
          {[
            { icon: <Search size={15} />, title: "Scan", text: "Point at the folder where your projects live. Each repository is detected and measured." },
            {
              icon: <Sparkles size={15} />,
              title: "Understand",
              text: "See what node_modules, target, .venv and build output cost per project, and why it is safe to remove.",
            },
            {
              icon: <ShieldCheck size={15} />,
              title: "Hibernate",
              text: "Review exactly what goes, then remove it to the Trash. Source, Git and lockfiles stay untouched.",
            },
          ].map((s) => (
            <li key={s.title} className="rounded-lg border border-border bg-surface p-3">
              <div className="mb-1 flex items-center gap-1.5 font-semibold text-fg">
                <span className="text-accent">{s.icon}</span> {s.title}
              </div>
              <p className="text-fg-muted">{s.text}</p>
            </li>
          ))}
        </ol>
        <div className="flex items-center gap-2">
          {roots.length ? (
            <>
              <Button variant="primary" size="lg" icon={<Search size={15} />} onClick={() => startScan()}>
                Scan {pluralize(roots.length, "folder")}
              </Button>
              <Button variant="ghost" size="lg" icon={<FolderPlus size={15} />} onClick={() => setAddFoldersOpen(true)}>
                Change folders
              </Button>
            </>
          ) : (
            <Button variant="primary" size="lg" icon={<FolderPlus size={15} />} onClick={() => setAddFoldersOpen(true)}>
              Choose project folders
            </Button>
          )}
        </div>
        <p className="mt-4 text-[11.5px] text-fg-subtle">Local-first. No account, no cloud, no file contents leave this machine.</p>
      </div>
    </div>
  );
}

export function OverviewPage() {
  const projects = useAppStore((s) => s.projects);
  const scanning = useAppStore((s) => s.scan.running);
  const setPage = useAppStore((s) => s.setPage);
  const openDrawer = useAppStore((s) => s.openDrawer);
  const selectMany = useAppStore((s) => s.selectMany);
  const reviewHibernate = useAppStore((s) => s.reviewHibernate);
  const warnings = useAppStore((s) => s.scan.warnings);

  if (!projects.length && !scanning) return <Welcome />;

  const top = [...projects].sort((a, b) => b.reclaimableBytes - a.reclaimableBytes).slice(0, 8);
  const dormant = projects.filter((p) => p.status === "dormant" && bulkEligible(p));
  const dormantBytes = dormant.reduce((s, p) => s + p.reclaimableBytes, 0);

  return (
    <div className="h-full overflow-y-auto p-5">
      <div className="mx-auto flex max-w-6xl flex-col gap-4">
        <StorageSummary />

        {dormant.length > 0 && (
          <div className="flex items-center justify-between rounded-lg border border-accent/30 bg-accent-soft/40 px-4 py-3">
            <div className="text-[13px] text-fg">
              <span className="font-semibold">{pluralize(dormant.length, "dormant project")}</span> could free{" "}
              <span className="font-semibold tabular">{formatBytes(dormantBytes)}</span> without touching any source code.
            </div>
            <Button
              variant="primary"
              size="sm"
              onClick={() => {
                selectMany(dormant.map((p) => p.id));
                reviewHibernate(dormant.map((p) => p.id));
              }}
            >
              Review dormant projects
            </Button>
          </div>
        )}

        <div className="grid grid-cols-[1.4fr_1fr] gap-4">
          <Card
            title="Largest reclaimable projects"
            padded={false}
            action={
              <Button variant="ghost" size="sm" icon={<ArrowRight size={13} />} onClick={() => setPage("projects")}>
                All projects
              </Button>
            }
          >
            <ul className="divide-y divide-border">
              {top.map((p) => {
                const share = top[0] ? p.reclaimableBytes / top[0].reclaimableBytes : 0;
                return (
                  <li key={p.id}>
                    <button
                      type="button"
                      onClick={() => {
                        setPage("projects");
                        openDrawer(p.id);
                      }}
                      className="grid w-full grid-cols-[1fr_auto_auto_auto] items-center gap-x-3 px-4 py-2 text-left text-[12.5px] hover:bg-surface-2"
                    >
                      <div className="min-w-0">
                        <div className="truncate font-medium text-fg">{p.name}</div>
                        <div className="mt-1 h-1 w-full max-w-64 rounded-full bg-surface-3">
                          <div className="h-full rounded-full bg-accent" style={{ width: `${share * 100}%` }} />
                        </div>
                      </div>
                      <span className="w-20 truncate text-fg-muted">{primaryLabel(p)}</span>
                      <span className="w-24 whitespace-nowrap text-right text-[11.5px] text-fg-subtle">{formatRelative(p.lastActivityAt)}</span>
                      <span className="w-20 text-right tabular font-semibold text-fg">{formatBytes(p.reclaimableBytes)}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          </Card>
          <div className="flex flex-col gap-4">
            <StorageBreakdown />
            <ReclaimTrend />
            <RecentCleanup />
          </div>
        </div>

        <GlobalCaches />

        <div className="grid grid-cols-4 gap-3">
          {(["active", "dormant", "hibernated", "protected"] as const).map((status) => {
            const n = projects.filter((p) => p.status === status).length;
            return (
              <button
                key={status}
                type="button"
                onClick={() => {
                  useAppStore.getState().setFilters({ statuses: [status] });
                  setPage("projects");
                }}
                className="flex items-center justify-between rounded-lg border border-border bg-surface px-4 py-2.5 text-left hover:bg-surface-2"
              >
                <StatusBadge status={status} />
                <span className="text-[16px] font-semibold tabular text-fg">{n}</span>
              </button>
            );
          })}
        </div>

        {warnings.length > 0 && (
          <Card title={`Scan warnings · ${warnings.length}`}>
            <ul className="max-h-40 overflow-y-auto font-mono text-[11.5px] text-fg-muted">
              {warnings.map((w, i) => (
                <li key={i} className="truncate">
                  {w.path ? `${w.path}: ` : ""}
                  {w.message}
                </li>
              ))}
            </ul>
          </Card>
        )}
      </div>
    </div>
  );
}
