// Formatting helpers. Sizes use decimal units to match the Rust side
// (crates/hibernate-core/src/format.rs).

const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"];

export function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n < 0) return "—";
  if (n < 1000) return `${Math.round(n)} B`;
  let value = n;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  const digits = value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(digits)} ${UNITS[unit]}`;
}

/** Split into number and unit for typographic emphasis: ["94.6", "GB"]. */
export function splitBytes(n: number): [string, string] {
  const [num, unit] = formatBytes(n).split(" ");
  return [num ?? "0", unit ?? "B"];
}

export function formatCount(n: number): string {
  return new Intl.NumberFormat().format(n);
}

export function formatPercent(part: number, whole: number): string {
  if (whole <= 0) return "0%";
  return `${Math.round((part / whole) * 100)}%`;
}

export function daysSince(iso: string | null, now = Date.now()): number | null {
  if (!iso) return null;
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return null;
  return Math.max(0, Math.floor((now - t) / 86_400_000));
}

export function formatRelative(iso: string | null, now = Date.now()): string {
  const days = daysSince(iso, now);
  if (days === null) return "Unknown";
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 30) return `${days} days ago`;
  if (days < 365) {
    const months = Math.floor(days / 30);
    return months === 1 ? "1 month ago" : `${months} months ago`;
  }
  const years = Math.floor(days / 365);
  return years === 1 ? "1 year ago" : `${years} years ago`;
}

export function formatDays(iso: string | null, now = Date.now()): string {
  const days = daysSince(iso, now);
  return days === null ? "—" : `${days}d`;
}

export function formatDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" });
}

export function formatDateTime(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)} s`;
  const m = Math.floor(s / 60);
  return `${m} min ${Math.round(s - m * 60)} s`;
}

/** Shorten a long path in the middle, keeping the last segments. */
export function shortenPath(path: string, max = 60): string {
  if (path.length <= max) return path;
  const sep = path.includes("\\") ? "\\" : "/";
  const parts = path.split(sep);
  let tail = "";
  for (let i = parts.length - 1; i >= 0; i--) {
    const next = parts[i] + (tail ? sep + tail : "");
    if (next.length + 4 > max) break;
    tail = next;
  }
  return `${parts[0] ?? ""}${sep}…${sep}${tail}`;
}

export function pluralize(n: number, one: string, many = `${one}s`): string {
  return `${formatCount(n)} ${n === 1 ? one : many}`;
}
