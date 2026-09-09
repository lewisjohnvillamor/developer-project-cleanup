import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  AppInfo,
  CacheInfo,
  FolderInfo,
  GlobalCache,
  HibernateEvent,
  HibernatePlan,
  HistoryStore,
  Project,
  QuarantineBatch,
  RestoreResult,
  RuleMatch,
  ScanEvent,
  ScanSnapshot,
  ScanTrend,
  Settings,
  WakeEvent,
  WakePlan,
} from "@/types";
import type { Backend } from "./backend";

async function on<T>(name: string, handler: (payload: T) => void) {
  return listen<T>(name, (event) => handler(event.payload));
}

export const tauriBackend: Backend = {
  kind: "tauri",
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings) => invoke<Settings>("update_settings", { settings }),
  async pickFolders() {
    const picked = await open({ directory: true, multiple: true, title: "Choose project folders" });
    if (!picked) return [];
    return Array.isArray(picked) ? picked : [picked];
  },
  validateFolder: (path) => invoke<FolderInfo>("validate_folder", { path }),

  getLastScan: () => invoke<ScanSnapshot>("get_last_scan"),
  startScan: (roots, full = false) => invoke("start_scan", { roots, full }),
  cancelScan: () => invoke("cancel_scan"),
  onScanEvent: (h) => on<ScanEvent>("scan-event", h),
  getScanTrend: () => invoke<ScanTrend>("get_scan_trend"),
  getCacheInfo: () => invoke<CacheInfo>("get_cache_info"),
  clearTreeCache: () => invoke("clear_tree_cache"),

  setProtected: (projectId, protected_) => invoke<Project>("set_protected", { projectId, protected: protected_ }),
  ignoreProject: (projectId, days) => invoke<Project>("ignore_project", { projectId, days }),
  unignoreProject: (projectId) => invoke<Project>("unignore_project", { projectId }),
  onProjectsUpdated: (h) => on<Project[]>("projects-updated", h),

  planHibernate: (request) => invoke<HibernatePlan>("plan_hibernate", { request }),
  startHibernate: (request) => invoke("start_hibernate", { request }),
  cancelHibernate: () => invoke("cancel_hibernate"),
  onHibernateEvent: (h) => on<HibernateEvent>("hibernate-event", h),

  getHistory: () => invoke<HistoryStore>("get_history"),
  restoreEntry: (entryId) => invoke<RestoreResult>("restore_entry", { entryId }),

  getWakePlan: (projectId) => invoke<WakePlan | null>("get_wake_plan", { projectId }),
  startWake: (projectId) => invoke<WakePlan>("start_wake", { projectId }),
  cancelWake: () => invoke("cancel_wake"),
  onWakeEvent: (h) => on<WakeEvent>("wake-event", h),

  openProjectFolder: (projectId) => invoke("open_project_folder", { projectId }),
  copyText: (text) => invoke("copy_text", { text }),

  getGlobalCaches: () => invoke<GlobalCache[]>("get_global_caches"),
  previewRule: (pattern, ecosystems) => invoke<RuleMatch[]>("preview_rule", { pattern, ecosystems }),
  async exportProjects(format) {
    const path = await save({
      title: "Export projects",
      defaultPath: `projects.${format}`,
      filters: [{ name: format.toUpperCase(), extensions: [format] }],
    });
    if (!path) return null;
    await invoke("export_projects", { path, format });
    return path;
  },
  listQuarantine: () => invoke<QuarantineBatch[]>("list_quarantine"),
  purgeQuarantineBatch: (entryId) => invoke<number>("purge_quarantine_batch", { entryId }),
  getDiagnostics: () => invoke<string>("get_diagnostics"),
};
