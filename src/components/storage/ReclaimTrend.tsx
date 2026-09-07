import { TrendingDown, TrendingUp } from "lucide-react";
import { Card } from "@/components/common/Card";
import { useAppStore } from "@/stores/app-store";
import { formatBytes, formatDate } from "@/utils/format";

/** Reclaimable space per scan, one point per day. Inline SVG, no library. */
export function ReclaimTrend() {
  const trend = useAppStore((s) => s.trend);
  if (trend.length < 2) return null;
  const points = trend.slice(-60);
  const values = points.map((p) => p.reclaimableBytes);
  const max = Math.max(...values, 1);
  const min = Math.min(...values, 0);
  const w = 100;
  const h = 32;
  const x = (i: number) => (i / (points.length - 1)) * w;
  const y = (v: number) => h - ((v - min) / (max - min || 1)) * (h - 4) - 2;
  const d = points.map((p, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(2)},${y(p.reclaimableBytes).toFixed(2)}`).join(" ");
  const area = `${d} L${w},${h} L0,${h} Z`;
  const first = values[0]!;
  const last = values[values.length - 1]!;
  const delta = last - first;
  const up = delta > 0;

  return (
    <Card title="Reclaimable over time">
      <div className="flex items-baseline justify-between">
        <span className="text-[18px] font-semibold tabular text-fg">{formatBytes(last)}</span>
        <span className={`flex items-center gap-1 text-[12px] ${up ? "text-review" : "text-safe"}`}>
          {up ? <TrendingUp size={13} /> : <TrendingDown size={13} />}
          {delta === 0 ? "no change" : `${up ? "+" : "−"}${formatBytes(Math.abs(delta))}`} since {formatDate(points[0]!.at)}
        </span>
      </div>
      <svg viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" className="mt-2 h-14 w-full" role="img" aria-label={`Reclaimable space over ${points.length} scans`}>
        <path d={area} fill="var(--accent)" opacity="0.12" />
        <path d={d} fill="none" stroke="var(--accent)" strokeWidth="1.5" vectorEffect="non-scaling-stroke" />
      </svg>
      <div className="mt-1 flex justify-between text-[10.5px] text-fg-subtle">
        <span>{formatDate(points[0]!.at)}</span>
        <span>{points.length} scans</span>
        <span>{formatDate(points[points.length - 1]!.at)}</span>
      </div>
    </Card>
  );
}
