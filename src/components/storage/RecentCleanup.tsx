import { ArrowRight, History } from "lucide-react";
import { Button } from "@/components/common/Button";
import { Card } from "@/components/common/Card";
import { useAppStore } from "@/stores/app-store";
import { formatBytes, formatDate, pluralize } from "@/utils/format";

export function RecentCleanup() {
  const history = useAppStore((s) => s.history);
  const setPage = useAppStore((s) => s.setPage);
  const latest = history[0];
  const lifetime = history.reduce((s, e) => s + e.totalRecovered, 0);

  return (
    <Card
      title="Recent cleanup"
      action={
        history.length ? (
          <Button variant="ghost" size="sm" onClick={() => setPage("history")} icon={<ArrowRight size={13} />}>
            View history
          </Button>
        ) : undefined
      }
    >
      {latest ? (
        <div className="flex flex-col gap-2">
          <div className="text-[12px] text-fg-muted">{formatDate(latest.finishedAt)}</div>
          <div className="flex items-baseline gap-1.5">
            <span className="text-[22px] font-semibold tabular text-fg">{formatBytes(latest.totalRecovered)}</span>
            <span className="text-[12.5px] text-fg-muted">
              recovered from {pluralize(latest.projectCount, "project")}
              {latest.errorCount ? ` · ${latest.errorCount} error${latest.errorCount === 1 ? "" : "s"}` : ""}
            </span>
          </div>
          <ul className="mt-1 divide-y divide-border">
            {[...latest.projects]
              .sort((a, b) => b.bytesRecovered - a.bytesRecovered)
              .slice(0, 4)
              .map((p) => (
                <li key={p.projectId} className="flex items-center justify-between py-1.5 text-[12.5px]">
                  <span className="truncate text-fg">{p.name}</span>
                  <span className="tabular text-fg-muted">{formatBytes(p.bytesRecovered)}</span>
                </li>
              ))}
          </ul>
          {history.length > 1 && (
            <div className="text-[11.5px] text-fg-subtle">
              {formatBytes(lifetime)} recovered across {pluralize(history.length, "cleanup")}
            </div>
          )}
        </div>
      ) : (
        <div className="flex items-center gap-3 py-2 text-[12.5px] text-fg-muted">
          <History size={16} className="text-fg-subtle" />
          No cleanups yet. Select dormant projects and hibernate them to see results here.
        </div>
      )}
    </Card>
  );
}
