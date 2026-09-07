import { useAppStore } from "@/stores/app-store";
import { formatBytes, formatPercent, splitBytes } from "@/utils/format";
import { Database, FolderOpen, HardDrive, Percent } from "lucide-react";
import type { ReactNode } from "react";

function Stat({
  icon,
  value,
  unit,
  label,
  emphasis,
  hint,
}: { icon: ReactNode; value: string; unit?: string; label: string; emphasis?: boolean; hint?: string }) {
  return (
    <div
      className={`flex flex-col gap-1 rounded-lg border bg-surface px-4 py-3.5 ${emphasis ? "border-accent/40 shadow-[inset_0_0_0_1px_var(--accent-soft)]" : "border-border"}`}
    >
      <div className="flex items-center gap-1.5 text-[11.5px] font-medium uppercase tracking-wide text-fg-muted">
        <span className={emphasis ? "text-accent" : ""}>{icon}</span>
        {label}
      </div>
      <div className="flex items-baseline gap-1">
        <span className={`tabular font-semibold tracking-tight ${emphasis ? "text-[30px] text-accent" : "text-[24px] text-fg"}`}>{value}</span>
        {unit && <span className={`text-[13px] font-medium ${emphasis ? "text-accent" : "text-fg-muted"}`}>{unit}</span>}
      </div>
      {hint && <div className="text-[11.5px] text-fg-subtle">{hint}</div>}
    </div>
  );
}

export function StorageSummary() {
  const summary = useAppStore((s) => s.summary);
  const projects = useAppStore((s) => s.projects);
  const scanning = useAppStore((s) => s.scan.running);

  const total = summary?.totalBytes ?? projects.reduce((s, p) => s + p.totalBytes, 0);
  const reclaimable = summary?.reclaimableBytes ?? projects.reduce((s, p) => s + p.reclaimableBytes, 0);
  const review = summary?.reviewBytes ?? projects.reduce((s, p) => s + p.reviewBytes, 0);
  const count = projects.length;
  const [r, ru] = splitBytes(reclaimable);
  const [t, tu] = splitBytes(total);

  return (
    <div className="grid grid-cols-4 gap-3">
      <Stat
        icon={<Database size={13} />}
        value={r}
        unit={ru}
        label="Safely reclaimable"
        emphasis
        hint={review > 0 ? `+ ${formatBytes(review)} more after review` : "Regeneratable folders only"}
      />
      <Stat icon={<HardDrive size={13} />} value={t} unit={tu} label="Total project size" hint={scanning ? "Still measuring…" : undefined} />
      <Stat icon={<FolderOpen size={13} />} value={String(count)} label="Projects" hint={scanning ? "Appearing as they are measured" : undefined} />
      <Stat
        icon={<Percent size={13} />}
        value={formatPercent(reclaimable, total).replace("%", "")}
        unit="%"
        label="Potential recovery"
        hint="Of total project size"
      />
    </div>
  );
}
