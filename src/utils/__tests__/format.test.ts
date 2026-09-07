import { describe, expect, it } from "vitest";
import { daysSince, formatBytes, formatCount, formatDays, formatPercent, formatRelative, pluralize, shortenPath, splitBytes } from "../format";

describe("formatBytes", () => {
  it("uses decimal units like the Rust side", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(999)).toBe("999 B");
    expect(formatBytes(1_000)).toBe("1.00 KB");
    expect(formatBytes(31_000_000)).toBe("31.0 MB");
    expect(formatBytes(890_000_000)).toBe("890 MB");
    expect(formatBytes(1_420_000_000)).toBe("1.42 GB");
    expect(formatBytes(94_600_000_000)).toBe("94.6 GB");
  });
  it("handles bad input", () => {
    expect(formatBytes(-1)).toBe("—");
    expect(formatBytes(Number.NaN)).toBe("—");
  });
  it("splits number and unit", () => {
    expect(splitBytes(94_600_000_000)).toEqual(["94.6", "GB"]);
  });
});

describe("relative time", () => {
  const now = Date.parse("2026-09-07T12:00:00Z");
  const ago = (days: number) => new Date(now - days * 86_400_000).toISOString();
  it("matches the Rust wording", () => {
    expect(formatRelative(ago(0), now)).toBe("Today");
    expect(formatRelative(ago(1), now)).toBe("Yesterday");
    expect(formatRelative(ago(14), now)).toBe("14 days ago");
    expect(formatRelative(ago(67), now)).toBe("2 months ago");
    expect(formatRelative(ago(400), now)).toBe("1 year ago");
    expect(formatRelative(null, now)).toBe("Unknown");
  });
  it("gives compact day counts", () => {
    expect(formatDays(ago(14), now)).toBe("14d");
    expect(formatDays(null, now)).toBe("—");
    expect(daysSince("not a date", now)).toBeNull();
  });
});

describe("misc", () => {
  it("formats counts, percents and plurals", () => {
    expect(formatCount(36412)).toBe("36,412");
    expect(formatPercent(33, 100)).toBe("33%");
    expect(formatPercent(1, 0)).toBe("0%");
    expect(pluralize(1, "project")).toBe("1 project");
    expect(pluralize(18, "project")).toBe("18 projects");
  });
  it("shortens long paths in the middle", () => {
    const p = "/home/dev/Projects/very/deep/folder/structure/BrowserSnaps";
    const short = shortenPath(p, 30);
    expect(short.length).toBeLessThanOrEqual(34);
    expect(short.endsWith("BrowserSnaps")).toBe(true);
    expect(shortenPath("/short", 30)).toBe("/short");
  });
});
