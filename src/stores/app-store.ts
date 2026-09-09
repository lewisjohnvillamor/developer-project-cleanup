import { create } from "zustand";
import { type Backend, getBackend } from "@/lib";
import type {
  AppError,
  AppInfo,
  ArtifactOutcome,
  ExportFormat,
  GlobalCache,
  HibernateEvent,
  HibernatePlan,
  HibernateRequest,
  HistoryEntry,
  Project,
  QuarantineBatch,
  ScanEvent,
  ScanRecord,
  ScanSummary,
  Settings,
  WakeEvent,
  WakePlan,
} from "@/types";
import { DEFAULT_SETTINGS } from "@/types";
import { EMPTY_FILTERS, type ProjectFilters, type SortDir, type SortKey } from "@/utils/filters";

export type Page = "overview" | "projects" | "history" | "settings";

export interface Toast {
  id: number;
  kind: "info" | "success" | "error";
  message: string;
}

export interface ScanProgress {
  running: boolean;
  discovered: number;
  scanned: number;
  currentName: string | null;
  warnings: { path: string | null; message: string }[];
  cancelled: boolean;
}

export interface HibernateProjectProgress {
  name: string;
  bytesRecovered: number;
  errorCount: number;
  done: boolean;
  artifacts: { relativePath: string; bytes: number; outcome: ArtifactOutcome | null; bytesRecovered: number }[];
}

export interface HibernateState {
  stage: "idle" | "review" | "running" | "done";
  request: HibernateRequest | null;
  plan: HibernatePlan | null;
  planLoading: boolean;
  error: string | null;
  entryId: string | null;
  completed: number;
  total: number;
  totalRecovered: number;
  currentProjectId: string | null;
  currentArtifact: string | null;
  perProject: Record<string, HibernateProjectProgress>;
  order: string[];
  entry: HistoryEntry | null;
}

export interface WakeState {
  stage: "idle" | "confirm" | "running" | "done";
  projectId: string | null;
  plan: WakePlan | null;
  lines: string[];
  currentStep: number;
  success: boolean | null;
  message: string;
  error: string | null;
}

const IDLE_HIBERNATE: HibernateState = {
  stage: "idle",
  request: null,
  plan: null,
  planLoading: false,
  error: null,
  entryId: null,
  completed: 0,
  total: 0,
  totalRecovered: 0,
  currentProjectId: null,
  currentArtifact: null,
  perProject: {},
  order: [],
  entry: null,
};

const IDLE_WAKE: WakeState = {
  stage: "idle",
  projectId: null,
  plan: null,
  lines: [],
  currentStep: -1,
  success: null,
  message: "",
  error: null,
};

interface AppStore {
  backend: Backend | null;
  ready: boolean;
  info: AppInfo | null;
  page: Page;
  settings: Settings;
  projects: Project[];
  summary: ScanSummary | null;
  scannedAt: string | null;
  scan: ScanProgress;
  selection: Set<string>;
  filters: ProjectFilters;
  sortKey: SortKey;
  sortDir: SortDir;
  drawerProjectId: string | null;
  hibernate: HibernateState;
  wake: WakeState;
  history: HistoryEntry[];
  toasts: Toast[];
  addFoldersOpen: boolean;
  quarantine: QuarantineBatch[];
  caches: GlobalCache[] | null;
  cachesLoading: boolean;
  trend: ScanRecord[];
  cacheEntries: number;
  paletteOpen: boolean;

  init(): Promise<void>;
  setPage(page: Page): void;
  saveSettings(patch: Partial<Settings>): Promise<void>;
  addScanRoots(paths: string[]): Promise<void>;
  removeScanRoot(path: string): Promise<void>;
  setAddFoldersOpen(open: boolean): void;

  startScan(roots?: string[], full?: boolean): Promise<void>;
  cancelScan(): Promise<void>;
  loadTrend(): Promise<void>;
  clearCache(): Promise<void>;

  toggleSelect(id: string): void;
  selectMany(ids: string[]): void;
  deselectMany(ids: string[]): void;
  clearSelection(): void;
  setFilters(patch: Partial<ProjectFilters>): void;
  resetFilters(): void;
  setSort(key: SortKey): void;
  openDrawer(id: string): void;
  closeDrawer(): void;

  setProtected(id: string, flag: boolean): Promise<void>;
  ignoreProject(id: string, days: number | null): Promise<void>;
  unignoreProject(id: string): Promise<void>;
  openFolder(id: string): Promise<void>;
  copyText(text: string, label?: string): Promise<void>;

  reviewHibernate(ids: string[], artifactPaths?: Record<string, string[]>): Promise<void>;
  setIncludeReview(include: boolean): Promise<void>;
  toggleReviewArtifact(projectId: string, artifactPath: string): Promise<void>;
  confirmHibernate(): Promise<void>;
  cancelHibernate(): Promise<void>;
  closeHibernate(): void;

  loadHistory(): Promise<void>;
  restoreEntry(id: string): Promise<void>;

  openWake(id: string): Promise<void>;
  runWake(): Promise<void>;
  cancelWake(): Promise<void>;
  closeWake(): void;

  loadQuarantine(): Promise<void>;
  purgeQuarantine(entryId: string): Promise<void>;
  loadCaches(): Promise<void>;
  exportProjects(format: ExportFormat): Promise<void>;
  copyDiagnostics(): Promise<void>;
  excludeFolder(path: string): Promise<void>;
  setPaletteOpen(open: boolean): void;

  toast(message: string, kind?: Toast["kind"]): void;
  dismissToast(id: number): void;
}

let toastSeq = 0;

function upsertProjects(list: Project[], updated: Project[]): Project[] {
  const map = new Map(list.map((p) => [p.id, p]));
  for (const p of updated) map.set(p.id, p);
  return [...map.values()];
}

function recomputeSummary(summary: ScanSummary | null, projects: Project[]): ScanSummary | null {
  if (!summary) return null;
  return {
    ...summary,
    projectCount: projects.length,
    totalBytes: projects.reduce((s, p) => s + p.totalBytes, 0),
    reclaimableBytes: projects.reduce((s, p) => s + p.reclaimableBytes, 0),
    reviewBytes: projects.reduce((s, p) => s + p.reviewBytes, 0),
  };
}

export const useAppStore = create<AppStore>((set, get) => ({
  backend: null,
  ready: false,
  info: null,
  page: "overview",
  settings: DEFAULT_SETTINGS,
  projects: [],
  summary: null,
  scannedAt: null,
  scan: { running: false, discovered: 0, scanned: 0, currentName: null, warnings: [], cancelled: false },
  selection: new Set(),
  filters: EMPTY_FILTERS,
  sortKey: "reclaimableBytes",
  sortDir: "desc",
  drawerProjectId: null,
  hibernate: IDLE_HIBERNATE,
  wake: IDLE_WAKE,
  history: [],
  toasts: [],
  addFoldersOpen: false,
  quarantine: [],
  caches: null,
  cachesLoading: false,
  trend: [],
  cacheEntries: 0,
  paletteOpen: false,

  async init() {
    const backend = await getBackend();
    const [info, settings, snapshot, history, trend, cacheInfo] = await Promise.all([
      backend.getAppInfo(),
      backend.getSettings(),
      backend.getLastScan(),
      backend.getHistory(),
      backend.getScanTrend().catch(() => ({ records: [] })),
      backend.getCacheInfo().catch(() => ({ entries: 0 })),
    ]);
    set({
      backend,
      info,
      settings,
      projects: snapshot.projects,
      summary: snapshot.summary,
      scannedAt: snapshot.scannedAt,
      history: history.entries,
      trend: trend.records,
      cacheEntries: cacheInfo.entries,
      page: "overview",
      ready: true,
    });

    await backend.onScanEvent((e: ScanEvent) => {
      const s = get();
      switch (e.type) {
        case "started":
          set({
            scan: { running: true, discovered: 0, scanned: 0, currentName: null, warnings: [], cancelled: false },
            projects: [],
            summary: null,
            selection: new Set(),
            drawerProjectId: null,
          });
          break;
        case "discovered":
          set({ scan: { ...s.scan, discovered: e.discovered, currentName: e.name } });
          break;
        case "scanned":
          set({
            projects: upsertProjects(s.projects, [e.project]),
            scan: { ...s.scan, scanned: e.scanned, discovered: e.discovered, currentName: e.project.name },
          });
          break;
        case "warning":
          set({ scan: { ...s.scan, warnings: [...s.scan.warnings, { path: e.path, message: e.message }].slice(-50) } });
          break;
        case "finished":
        case "cancelled": {
          backend.getLastScan().then((snap) => {
            set({
              projects: snap.projects,
              summary: snap.summary,
              scannedAt: snap.scannedAt,
              scan: { ...get().scan, running: false, cancelled: e.type === "cancelled" },
            });
            if (e.type === "cancelled") get().toast("Scan cancelled. Showing partial results.", "info");
            else if (snap.summary && snap.summary.cacheHits > 0) {
              get().toast(
                `Scan finished. ${snap.summary.cacheHits} folder size${snap.summary.cacheHits === 1 ? "" : "s"} reused from the previous scan.`,
                "success",
              );
            }
            get().loadTrend();
            backend
              .getCacheInfo()
              .then((c) => set({ cacheEntries: c.entries }))
              .catch(() => {});
          });
          break;
        }
      }
    });

    // A failed write means the screen and the disk now disagree. Say so
    // rather than letting the next launch quietly contradict this one.
    await backend.onAppError((e: AppError) => {
      get().toast(`${e.operation} failed: ${e.message}. Changes made since the last successful save may be lost.`, "error");
    });

    await backend.onProjectsUpdated((updated) => {
      const s = get();
      const projects = upsertProjects(s.projects, updated);
      set({ projects, summary: recomputeSummary(s.summary, projects) });
    });

    await backend.onHibernateEvent((e: HibernateEvent) => {
      const h = get().hibernate;
      switch (e.type) {
        case "started":
          set({ hibernate: { ...h, stage: "running", entryId: e.entryId, total: e.projectCount, completed: 0, totalRecovered: 0 } });
          break;
        case "projectStarted": {
          const per = { ...h.perProject };
          per[e.projectId] = per[e.projectId] ?? { name: e.name, bytesRecovered: 0, errorCount: 0, done: false, artifacts: [] };
          set({
            hibernate: {
              ...h,
              perProject: per,
              currentProjectId: e.projectId,
              currentArtifact: null,
              order: h.order.includes(e.projectId) ? h.order : [...h.order, e.projectId],
            },
          });
          break;
        }
        case "artifactStarted": {
          const per = { ...h.perProject };
          const pp = per[e.projectId];
          if (pp) {
            per[e.projectId] = { ...pp, artifacts: [...pp.artifacts, { relativePath: e.relativePath, bytes: e.bytes, outcome: null, bytesRecovered: 0 }] };
          }
          set({ hibernate: { ...h, perProject: per, currentArtifact: e.relativePath } });
          break;
        }
        case "artifactFinished": {
          const per = { ...h.perProject };
          const pp = per[e.projectId];
          if (pp) {
            per[e.projectId] = {
              ...pp,
              artifacts: pp.artifacts.map((a) =>
                a.relativePath === e.relativePath && a.outcome === null ? { ...a, outcome: e.outcome, bytesRecovered: e.bytesRecovered } : a,
              ),
            };
          }
          set({ hibernate: { ...h, perProject: per, currentArtifact: null } });
          break;
        }
        case "projectFinished": {
          const per = { ...h.perProject };
          const pp = per[e.projectId] ?? { name: e.name, bytesRecovered: 0, errorCount: 0, done: false, artifacts: [] };
          per[e.projectId] = { ...pp, bytesRecovered: e.bytesRecovered, errorCount: e.errorCount, done: true };
          set({ hibernate: { ...h, perProject: per, completed: e.completed, total: e.total, totalRecovered: e.totalRecovered, currentArtifact: null } });
          break;
        }
        case "finished":
          set({ hibernate: { ...get().hibernate, stage: "done", entry: e.entry, currentProjectId: null, currentArtifact: null }, selection: new Set() });
          get().loadHistory();
          break;
      }
    });

    await backend.onWakeEvent((e: WakeEvent) => {
      const w = get().wake;
      if (w.projectId !== e.projectId) return;
      switch (e.type) {
        case "stepStarted":
          set({ wake: { ...w, currentStep: e.stepIndex, lines: [...w.lines, `$ ${e.display}`] } });
          break;
        case "line":
          set({ wake: { ...w, lines: [...w.lines, e.text].slice(-2000) } });
          break;
        case "stepFinished":
          if (e.exitCode !== 0) set({ wake: { ...w, lines: [...w.lines, `exit status ${e.exitCode}`] } });
          break;
        case "finished":
          set({ wake: { ...w, stage: "done", success: e.success, message: e.message } });
          break;
      }
    });
  },

  setPage(page) {
    set({ page, drawerProjectId: null });
  },

  async saveSettings(patch) {
    const { backend, settings } = get();
    if (!backend) return;
    const next = { ...settings, ...patch };
    set({ settings: next });
    try {
      const saved = await backend.updateSettings(next);
      set({ settings: saved });
    } catch (err) {
      get().toast(`Could not save settings: ${String(err)}`, "error");
    }
  },

  async addScanRoots(paths) {
    const { backend, settings } = get();
    if (!backend) return;
    const roots = [...settings.scanRoots];
    for (const p of paths) {
      try {
        const info = await backend.validateFolder(p);
        if (!roots.includes(info.path)) roots.push(info.path);
        if (info.warning) get().toast(info.warning, "info");
      } catch (err) {
        get().toast(String(err), "error");
      }
    }
    await get().saveSettings({ scanRoots: roots });
  },

  async removeScanRoot(path) {
    const { settings } = get();
    await get().saveSettings({ scanRoots: settings.scanRoots.filter((r) => r !== path) });
  },

  setAddFoldersOpen(open) {
    set({ addFoldersOpen: open });
  },

  async startScan(roots, full = false) {
    const { backend, settings } = get();
    if (!backend) return;
    const targets = roots ?? settings.scanRoots;
    if (!targets.length) {
      set({ addFoldersOpen: true });
      return;
    }
    try {
      await backend.startScan(targets, full);
      set({ page: get().page === "settings" ? "projects" : get().page });
    } catch (err) {
      get().toast(String(err), "error");
    }
  },

  async cancelScan() {
    await get().backend?.cancelScan();
  },

  async loadTrend() {
    const { backend } = get();
    if (!backend) return;
    try {
      const t = await backend.getScanTrend();
      set({ trend: t.records });
    } catch {
      /* optional */
    }
  },

  async clearCache() {
    const { backend } = get();
    if (!backend) return;
    await backend.clearTreeCache();
    set({ cacheEntries: 0 });
    get().toast("Cached folder sizes cleared. The next scan measures everything again.", "info");
  },

  toggleSelect(id) {
    const sel = new Set(get().selection);
    if (sel.has(id)) sel.delete(id);
    else sel.add(id);
    set({ selection: sel });
  },
  selectMany(ids) {
    const sel = new Set(get().selection);
    for (const id of ids) sel.add(id);
    set({ selection: sel });
  },
  deselectMany(ids) {
    const sel = new Set(get().selection);
    for (const id of ids) sel.delete(id);
    set({ selection: sel });
  },
  clearSelection() {
    set({ selection: new Set() });
  },
  setFilters(patch) {
    set({ filters: { ...get().filters, ...patch } });
  },
  resetFilters() {
    set({ filters: EMPTY_FILTERS });
  },
  setSort(key) {
    const { sortKey, sortDir } = get();
    if (sortKey === key) set({ sortDir: sortDir === "asc" ? "desc" : "asc" });
    else set({ sortKey: key, sortDir: key === "name" || key === "stack" || key === "status" ? "asc" : "desc" });
  },
  openDrawer(id) {
    set({ drawerProjectId: id });
  },
  closeDrawer() {
    set({ drawerProjectId: null });
  },

  async setProtected(id, flag) {
    const { backend } = get();
    if (!backend) return;
    try {
      const p = await backend.setProtected(id, flag);
      const sel = new Set(get().selection);
      if (flag) sel.delete(id);
      set({ projects: upsertProjects(get().projects, [p]), selection: sel });
      get().toast(flag ? `${p.name} is now protected` : `${p.name} is no longer protected`, "success");
    } catch (err) {
      get().toast(String(err), "error");
    }
  },

  async ignoreProject(id, days) {
    const { backend } = get();
    if (!backend) return;
    try {
      const p = await backend.ignoreProject(id, days);
      const sel = new Set(get().selection);
      sel.delete(id);
      set({ projects: upsertProjects(get().projects, [p]), selection: sel, drawerProjectId: null });
      get().toast(days === null ? `${p.name} hidden until restored` : `${p.name} hidden for ${days} days`, "info");
    } catch (err) {
      get().toast(String(err), "error");
    }
  },

  async unignoreProject(id) {
    const { backend } = get();
    if (!backend) return;
    try {
      const p = await backend.unignoreProject(id);
      set({ projects: upsertProjects(get().projects, [p]) });
    } catch (err) {
      get().toast(String(err), "error");
    }
  },

  async openFolder(id) {
    try {
      await get().backend?.openProjectFolder(id);
    } catch (err) {
      get().toast(String(err), "error");
    }
  },

  async copyText(text, label = "Path") {
    try {
      await get().backend?.copyText(text);
      get().toast(`${label} copied`, "success");
    } catch (err) {
      get().toast(String(err), "error");
    }
  },

  async reviewHibernate(ids, artifactPaths) {
    const { backend, settings, projects } = get();
    if (!backend) return;
    const eligible = ids.filter((id) => {
      const p = projects.find((x) => x.id === id);
      return p && !p.protected;
    });
    if (!eligible.length) {
      get().toast("Protected projects are never hibernated. Unprotect one first.", "info");
      return;
    }
    const request: HibernateRequest = {
      selection: eligible.map((id) => ({ projectId: id, artifactPaths: artifactPaths?.[id] ?? null })),
      includeReview: settings.includeReviewItems,
    };
    set({ hibernate: { ...IDLE_HIBERNATE, stage: "review", request, planLoading: true } });
    try {
      const plan = await backend.planHibernate(request);
      set({ hibernate: { ...get().hibernate, plan, planLoading: false } });
    } catch (err) {
      set({ hibernate: { ...get().hibernate, planLoading: false, error: String(err) } });
    }
  },

  async setIncludeReview(include) {
    const { backend, hibernate } = get();
    if (!backend || !hibernate.request) return;
    const request = { ...hibernate.request, includeReview: include };
    set({ hibernate: { ...hibernate, request, planLoading: true } });
    try {
      const plan = await backend.planHibernate(request);
      set({ hibernate: { ...get().hibernate, plan, planLoading: false } });
    } catch (err) {
      set({ hibernate: { ...get().hibernate, planLoading: false, error: String(err) } });
    }
  },

  async toggleReviewArtifact(projectId, artifactPath) {
    const { backend, hibernate } = get();
    if (!backend || !hibernate.request || !hibernate.plan) return;
    const planned = hibernate.plan.projects.find((p) => p.id === projectId);
    if (!planned) return;
    const current = new Set(planned.artifacts.map((a) => a.path));
    if (current.has(artifactPath)) current.delete(artifactPath);
    else current.add(artifactPath);
    const selection = hibernate.request.selection.map((s) => (s.projectId === projectId ? { ...s, artifactPaths: [...current] } : s));
    const request = { ...hibernate.request, selection };
    set({ hibernate: { ...hibernate, request, planLoading: true } });
    try {
      const plan = await backend.planHibernate(request);
      set({ hibernate: { ...get().hibernate, plan, planLoading: false } });
    } catch (err) {
      set({ hibernate: { ...get().hibernate, planLoading: false, error: String(err) } });
    }
  },

  async confirmHibernate() {
    const { backend, hibernate } = get();
    if (!backend || !hibernate.request) return;
    set({ hibernate: { ...hibernate, stage: "running", error: null } });
    try {
      await backend.startHibernate(hibernate.request);
    } catch (err) {
      set({ hibernate: { ...get().hibernate, stage: "review", error: String(err) } });
    }
  },

  async cancelHibernate() {
    await get().backend?.cancelHibernate();
  },

  closeHibernate() {
    set({ hibernate: IDLE_HIBERNATE });
  },

  async loadHistory() {
    const { backend } = get();
    if (!backend) return;
    const h = await backend.getHistory();
    set({ history: h.entries });
  },

  async restoreEntry(id) {
    const { backend } = get();
    if (!backend) return;
    try {
      const r = await backend.restoreEntry(id);
      await get().loadHistory();
      if (r.errors.length) get().toast(`Restored ${r.restored} folder(s); ${r.errors.length} failed: ${r.errors[0]}`, "error");
      else get().toast(`Restored ${r.restored} folder(s). Scan again to refresh sizes.`, "success");
    } catch (err) {
      get().toast(String(err), "error");
    }
  },

  async openWake(id) {
    const { backend } = get();
    if (!backend) return;
    try {
      const plan = await backend.getWakePlan(id);
      if (!plan) {
        get().toast("No wake command is known for this project.", "info");
        return;
      }
      set({ wake: { ...IDLE_WAKE, stage: "confirm", projectId: id, plan } });
    } catch (err) {
      get().toast(String(err), "error");
    }
  },

  async runWake() {
    const { backend, wake } = get();
    if (!backend || !wake.projectId) return;
    set({ wake: { ...wake, stage: "running", lines: [], currentStep: -1, error: null } });
    try {
      await backend.startWake(wake.projectId);
    } catch (err) {
      set({ wake: { ...get().wake, stage: "confirm", error: String(err) } });
    }
  },

  async cancelWake() {
    await get().backend?.cancelWake();
  },

  closeWake() {
    set({ wake: IDLE_WAKE });
  },

  async loadQuarantine() {
    const { backend } = get();
    if (!backend) return;
    try {
      set({ quarantine: await backend.listQuarantine() });
    } catch (err) {
      get().toast(String(err), "error");
    }
  },

  async purgeQuarantine(entryId) {
    const { backend } = get();
    if (!backend) return;
    try {
      const bytes = await backend.purgeQuarantineBatch(entryId);
      get().toast(`Quarantine batch removed permanently. ${bytes ? `${Math.round(bytes / 1e6)} MB freed.` : ""}`, "success");
      await Promise.all([get().loadQuarantine(), get().loadHistory()]);
      const info = await backend.getAppInfo();
      set({ info });
    } catch (err) {
      get().toast(String(err), "error");
    }
  },

  async loadCaches() {
    const { backend, cachesLoading } = get();
    if (!backend || cachesLoading) return;
    set({ cachesLoading: true });
    try {
      set({ caches: await backend.getGlobalCaches() });
    } catch (err) {
      get().toast(String(err), "error");
    } finally {
      set({ cachesLoading: false });
    }
  },

  async exportProjects(format) {
    const { backend, projects } = get();
    if (!backend) return;
    if (!projects.length) {
      get().toast("Nothing to export yet. Run a scan first.", "info");
      return;
    }
    try {
      const path = await backend.exportProjects(format);
      if (path) get().toast(`Exported ${projects.length} projects to ${path}`, "success");
    } catch (err) {
      get().toast(String(err), "error");
    }
  },

  async copyDiagnostics() {
    const { backend } = get();
    if (!backend) return;
    try {
      const text = await backend.getDiagnostics();
      await backend.copyText(text);
      get().toast("Diagnostics copied. Paste them into a bug report.", "success");
    } catch (err) {
      get().toast(String(err), "error");
    }
  },

  async excludeFolder(path) {
    const { settings } = get();
    if (settings.ignoredPaths.includes(path)) return;
    await get().saveSettings({ ignoredPaths: [...settings.ignoredPaths, path] });
    const sel = new Set(get().selection);
    const p = get().projects.find((x) => x.path === path);
    if (p) sel.delete(p.id);
    set({ projects: get().projects.filter((x) => x.path !== path), selection: sel, drawerProjectId: null });
    get().toast("Folder excluded from future scans. Manage exclusions in Settings.", "info");
  },

  setPaletteOpen(open) {
    set({ paletteOpen: open });
  },

  toast(message, kind = "info") {
    const id = ++toastSeq;
    set({ toasts: [...get().toasts, { id, kind, message }] });
    setTimeout(() => get().dismissToast(id), kind === "error" ? 8000 : 4000);
  },
  dismissToast(id) {
    set({ toasts: get().toasts.filter((t) => t.id !== id) });
  },
}));
