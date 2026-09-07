// Drives the store through the whole journey against the in-memory mock
// backend: scan → select → review → hibernate → history → wake.
import { beforeAll, describe, expect, it } from "vitest";
import { useAppStore } from "../app-store";

async function until(pred: () => boolean, ms = 20_000) {
  const start = Date.now();
  while (!pred()) {
    if (Date.now() - start > ms) throw new Error("timed out waiting for condition");
    await new Promise((r) => setTimeout(r, 25));
  }
}

describe("app store with the mock backend", () => {
  beforeAll(async () => {
    await useAppStore.getState().init();
  });

  it("initialises in the browser preview", () => {
    const s = useAppStore.getState();
    expect(s.ready).toBe(true);
    expect(s.backend?.kind).toBe("mock");
    expect(s.settings.disposition).toBe("trash");
  });

  it("scans progressively and ends with a summary", async () => {
    const s = useAppStore.getState();
    await s.addScanRoots(["/home/dev/Projects"]);
    expect(useAppStore.getState().settings.scanRoots).toEqual(["/home/dev/Projects"]);
    await s.startScan();
    await until(() => useAppStore.getState().scan.running);
    await until(() => useAppStore.getState().projects.length > 5);
    expect(useAppStore.getState().scan.running).toBe(true);
    await until(() => !useAppStore.getState().scan.running);
    const after = useAppStore.getState();
    expect(after.projects.length).toBe(45);
    expect(after.summary?.reclaimableBytes).toBeGreaterThan(0);
    expect(after.trend.length).toBeGreaterThan(1);
  }, 30_000);

  it("never bulk-selects protected projects and plans without review items", async () => {
    const s = useAppStore.getState();
    const protectedProject = s.projects.find((p) => p.protected)!;
    expect(protectedProject).toBeDefined();
    s.selectMany(s.projects.map((p) => p.id));
    await s.reviewHibernate([...useAppStore.getState().selection]);
    await until(() => useAppStore.getState().hibernate.plan !== null);
    const h = useAppStore.getState().hibernate;
    expect(h.stage).toBe("review");
    // Protected projects are dropped before planning, never merely skipped.
    expect(h.plan!.projects.some((p) => p.id === protectedProject.id)).toBe(false);
    expect(h.request!.selection.some((sel) => sel.projectId === protectedProject.id)).toBe(false);
    expect(h.plan!.reviewCount).toBe(0);
    expect(h.plan!.projects.every((p) => p.artifacts.every((a) => a.safety === "safe"))).toBe(true);
  });

  it("lets a single folder be unticked in the review", async () => {
    const h = useAppStore.getState().hibernate;
    const target = h.plan!.projects.find((p) => p.artifacts.length > 0)!;
    const before = h.plan!.totalBytes;
    const removed = target.artifacts[0]!;
    await useAppStore.getState().toggleReviewArtifact(target.id, removed.path);
    await until(() => !useAppStore.getState().hibernate.planLoading);
    const plan = useAppStore.getState().hibernate.plan!;
    expect(plan.totalBytes).toBe(before - removed.bytes);
    // Put it back.
    await useAppStore.getState().toggleReviewArtifact(target.id, removed.path);
    await until(() => !useAppStore.getState().hibernate.planLoading);
    expect(useAppStore.getState().hibernate.plan!.totalBytes).toBe(before);
  });

  it("runs the cleanup, records history and marks projects hibernated", async () => {
    const s = useAppStore.getState();
    const planned = s.hibernate.plan!.projects.length;
    await s.confirmHibernate();
    await until(() => useAppStore.getState().hibernate.stage === "done", 120_000);
    const done = useAppStore.getState();
    expect(done.hibernate.entry!.projectCount).toBe(planned);
    expect(done.hibernate.entry!.totalRecovered).toBeGreaterThan(0);
    expect(done.selection.size).toBe(0);
    await until(() => useAppStore.getState().history.length === 1);
    const hibernated = useAppStore.getState().projects.filter((p) => p.status === "hibernated");
    expect(hibernated.length).toBeGreaterThan(0);
    done.closeHibernate();
    expect(useAppStore.getState().hibernate.stage).toBe("idle");
  }, 150_000);

  it("wakes a hibernated project and streams output", async () => {
    const s = useAppStore.getState();
    const target = s.projects.find((p) => p.status === "hibernated" && p.stacks.includes("node"))!;
    await s.openWake(target.id);
    expect(useAppStore.getState().wake.stage).toBe("confirm");
    expect(useAppStore.getState().wake.plan!.steps.length).toBeGreaterThan(0);
    await useAppStore.getState().runWake();
    await until(() => useAppStore.getState().wake.stage === "done", 30_000);
    const w = useAppStore.getState().wake;
    expect(w.success).toBe(true);
    expect(w.lines.some((l) => l.startsWith("$ "))).toBe(true);
    expect(useAppStore.getState().projects.find((p) => p.id === target.id)!.status).not.toBe("hibernated");
    useAppStore.getState().closeWake();
  }, 40_000);

  it("protects, ignores and excludes projects", async () => {
    const s = useAppStore.getState();
    const p = s.projects.find((x) => !x.protected && x.status !== "hibernated")!;
    await s.setProtected(p.id, true);
    expect(useAppStore.getState().projects.find((x) => x.id === p.id)!.status).toBe("protected");
    await s.setProtected(p.id, false);
    await s.ignoreProject(p.id, 7);
    expect(useAppStore.getState().projects.find((x) => x.id === p.id)!.ignoredUntil).not.toBeNull();
    await s.unignoreProject(p.id);
    await s.excludeFolder(p.path);
    expect(useAppStore.getState().settings.ignoredPaths).toContain(p.path);
    expect(useAppStore.getState().projects.some((x) => x.id === p.id)).toBe(false);
  });
});
