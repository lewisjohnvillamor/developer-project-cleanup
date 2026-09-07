import { describe, expect, it } from "vitest";
import type { Project } from "@/types";
import { activeFilterCount, applyFilters, bulkEligible, EMPTY_FILTERS, isIgnored, primaryLabel, sortProjects } from "../filters";

const NOW = Date.parse("2026-09-07T12:00:00Z");
const ago = (days: number) => new Date(NOW - days * 86_400_000).toISOString();

function project(overrides: Partial<Project> & { name: string }): Project {
  return {
    id: overrides.name,
    path: `/p/${overrides.name}`,
    scanRoot: "/p",
    parentPath: null,
    stacks: ["node"],
    frameworks: [],
    workspaceMembers: [],
    scanDurationMs: 0,
    cacheHits: 0,
    packageManager: null,
    totalBytes: 100,
    reclaimableBytes: 50,
    reviewBytes: 0,
    fileCount: 1,
    lastActivityAt: ago(40),
    activity: { sourceModifiedAt: null, gitCommitAt: null, appActivityAt: null },
    git: { isRepo: true, state: "clean", isClean: true, modifiedCount: 0, untrackedCount: 0, remoteConfigured: true, branch: "main", lastCommitAt: null },
    status: "dormant",
    safety: "safe",
    safetyReasons: [],
    artifacts: [],
    protectedEntries: [],
    protected: false,
    ignoredUntil: null,
    hibernation: null,
    scannedAt: ago(0),
    warnings: [],
    ...overrides,
  };
}

const projects = [
  project({ name: "LogParser", stacks: ["rust"], reclaimableBytes: 7_600_000_000, totalBytes: 8_100_000_000, lastActivityAt: ago(67) }),
  project({ name: "BrowserSnaps", frameworks: ["Next.js", "TypeScript"], reclaimableBytes: 890_000_000, totalBytes: 1_200_000_000, lastActivityAt: ago(14), git: { ...project({ name: "x" }).git!, state: "modified", isClean: false, modifiedCount: 2 } }),
  project({ name: "Pointsy", stacks: ["rust"], status: "protected", protected: true, reclaimableBytes: 3_100_000_000, lastActivityAt: ago(4) }),
  project({ name: "hidden", ignoredUntil: ago(-30), reclaimableBytes: 10 }),
  project({ name: "expired-ignore", ignoredUntil: ago(1), reclaimableBytes: 20 }),
];

describe("applyFilters", () => {
  it("hides ignored projects unless asked", () => {
    const names = applyFilters(projects, EMPTY_FILTERS, NOW).map((p) => p.name);
    expect(names).not.toContain("hidden");
    expect(names).toContain("expired-ignore");
    expect(applyFilters(projects, { ...EMPTY_FILTERS, showIgnored: true }, NOW)).toHaveLength(5);
  });
  it("combines search, stack, inactivity, size and git filters", () => {
    expect(applyFilters(projects, { ...EMPTY_FILTERS, search: "log" }, NOW).map((p) => p.name)).toEqual(["LogParser"]);
    expect(applyFilters(projects, { ...EMPTY_FILTERS, stacks: ["rust"] }, NOW)).toHaveLength(2);
    expect(applyFilters(projects, { ...EMPTY_FILTERS, inactiveDays: 30 }, NOW).map((p) => p.name)).toEqual(["LogParser", "expired-ignore"]);
    expect(applyFilters(projects, { ...EMPTY_FILTERS, minReclaimableBytes: 1e9 }, NOW).map((p) => p.name)).toEqual(["LogParser", "Pointsy"]);
    expect(applyFilters(projects, { ...EMPTY_FILTERS, gitClean: false }, NOW).map((p) => p.name)).toEqual(["BrowserSnaps"]);
    expect(applyFilters(projects, { ...EMPTY_FILTERS, notProtected: true, stacks: ["rust"] }, NOW).map((p) => p.name)).toEqual(["LogParser"]);
    expect(applyFilters(projects, { ...EMPTY_FILTERS, statuses: ["protected"] }, NOW).map((p) => p.name)).toEqual(["Pointsy"]);
  });
  it("counts active filters", () => {
    expect(activeFilterCount(EMPTY_FILTERS)).toBe(0);
    expect(activeFilterCount({ ...EMPTY_FILTERS, stacks: ["node"], inactiveDays: 30, notProtected: true })).toBe(3);
  });
});

describe("sortProjects", () => {
  it("sorts by reclaimable desc with name tiebreak", () => {
    const sorted = sortProjects(projects, "reclaimableBytes", "desc").map((p) => p.name);
    expect(sorted.slice(0, 3)).toEqual(["LogParser", "Pointsy", "BrowserSnaps"]);
  });
  it("sorts by name and last active", () => {
    expect(sortProjects(projects, "name", "asc")[0]!.name).toBe("BrowserSnaps");
    expect(sortProjects(projects, "lastActive", "asc")[0]!.name).toBe("LogParser");
  });
});

describe("helpers", () => {
  it("labels by framework then stack", () => {
    expect(primaryLabel(projects[1]!)).toBe("Next.js");
    expect(primaryLabel(projects[0]!)).toBe("Rust");
    expect(primaryLabel(project({ name: "ts", frameworks: ["TypeScript"] }))).toBe("Node");
  });
  it("never bulk-selects protected or ignored projects", () => {
    expect(bulkEligible(projects[0]!, NOW)).toBe(true);
    expect(bulkEligible(projects[2]!, NOW)).toBe(false);
    expect(bulkEligible(projects[3]!, NOW)).toBe(false);
    expect(isIgnored(projects[4]!, NOW)).toBe(false);
  });
});
