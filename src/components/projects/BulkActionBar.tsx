import { Archive, ListChecks, X } from "lucide-react";
import { Button } from "@/components/common/Button";
import { useAppStore } from "@/stores/app-store";
import { formatBytes, pluralize } from "@/utils/format";

export function BulkActionBar() {
  const selection = useAppStore((s) => s.selection);
  const projects = useAppStore((s) => s.projects);
  const clearSelection = useAppStore((s) => s.clearSelection);
  const reviewHibernate = useAppStore((s) => s.reviewHibernate);
  const includeReview = useAppStore((s) => s.settings.includeReviewItems);
  if (!selection.size) return null;

  const selected = projects.filter((p) => selection.has(p.id) && !p.protected);
  const safe = selected.reduce((s, p) => s + p.reclaimableBytes, 0);
  const review = selected.reduce((s, p) => s + p.reviewBytes, 0);
  const estimate = includeReview ? safe + review : safe;

  return (
    <div className="fade-in pointer-events-none absolute inset-x-0 bottom-4 z-20 flex justify-center px-4">
      <div className="pointer-events-auto flex items-center gap-4 rounded-xl border border-border bg-surface px-4 py-2.5 shadow-panel">
        <div className="flex items-center gap-2 text-[12.5px]">
          <ListChecks size={15} className="text-accent" />
          <span className="font-medium text-fg">{pluralize(selected.length, "project")} selected</span>
        </div>
        <div className="h-5 w-px bg-border" />
        <div className="text-[12.5px] text-fg-muted">
          Estimated recovery <span className="ml-1 font-semibold tabular text-fg">{formatBytes(estimate)}</span>
          {!includeReview && review > 0 && <span className="ml-1 text-[11px] text-fg-subtle">(+{formatBytes(review)} needs review)</span>}
        </div>
        <div className="h-5 w-px bg-border" />
        <Button variant="ghost" size="sm" onClick={clearSelection} icon={<X size={13} />}>
          Clear
        </Button>
        <Button variant="primary" size="md" icon={<Archive size={14} />} onClick={() => reviewHibernate([...selection])}>
          Review &amp; Hibernate
        </Button>
      </div>
    </div>
  );
}
