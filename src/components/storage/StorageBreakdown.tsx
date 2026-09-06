import { Card } from "@/components/common/Card";
import { useAppStore } from "@/stores/app-store";
import { CATEGORY_LABELS, type ArtifactCategory } from "@/types";
import { formatBytes, formatCount } from "@/utils/format";

const COLORS: Record<ArtifactCategory, string> = {
  nodeDependencies: "bg-[#2563eb]",
  rustTarget: "bg-[#ea580c]",
  buildArtifacts: "bg-[#7c3aed]",
  pythonEnvironments: "bg-[#0891b2]",
  caches: "bg-[#64748b]",
  other: "bg-[#a1a1aa]",
};

export function StorageBreakdown() {
  const summary = useAppStore((s) => s.summary);
  const projects = useAppStore((s) => s.projects);

  let rows = summary?.byCategory ?? [];
  if (!rows.length && projects.length) {
    const map = new Map<ArtifactCategory, { bytes: number; count: number }>();
    for (const p of projects)
      for (const a of p.artifacts) {
        if (a.safety !== "safe") continue;
        const c = map.get(a.category) ?? { bytes: 0, count: 0 };
        c.bytes += a.bytes;
        c.count++;
        map.set(a.category, c);
      }
    rows = [...map.entries()].map(([category, v]) => ({ category, ...v })).sort((a, b) => b.bytes - a.bytes);
  }
  const max = rows[0]?.bytes ?? 1;
  const total = rows.reduce((s, r) => s + r.bytes, 0);

  return (
    <Card title="Reclaimable storage by category">
      {rows.length ? (
        <div className="flex flex-col gap-2.5">
          <div className="flex h-2 w-full overflow-hidden rounded-full bg-surface-3">
            {rows.map((r) => (
              <div key={r.category} className={`${COLORS[r.category]} h-full`} style={{ width: `${(r.bytes / total) * 100}%` }} title={CATEGORY_LABELS[r.category]} />
            ))}
          </div>
          <ul className="mt-1 flex flex-col gap-1.5">
            {rows.map((r) => (
              <li key={r.category} className="grid grid-cols-[10px_1fr_auto_auto] items-center gap-x-3 text-[12.5px]">
                <span className={`h-2.5 w-2.5 rounded-sm ${COLORS[r.category]}`} />
                <span className="text-fg">{CATEGORY_LABELS[r.category]}</span>
                <span className="text-[11px] text-fg-subtle tabular">{formatCount(r.count)} folders</span>
                <span className="w-20 text-right tabular font-medium text-fg">{formatBytes(r.bytes)}</span>
                <span />
                <div className="col-span-3 h-1 rounded-full bg-surface-3">
                  <div className={`${COLORS[r.category]} h-full rounded-full opacity-70`} style={{ width: `${(r.bytes / max) * 100}%` }} />
                </div>
              </li>
            ))}
          </ul>
        </div>
      ) : (
        <p className="text-[12.5px] text-fg-muted">Nothing reclaimable found yet.</p>
      )}
    </Card>
  );
}
