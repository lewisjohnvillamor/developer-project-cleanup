import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { Backend } from "./backend";
import type {
  AppInfo,
  FolderInfo,
  HibernateEvent,
  HibernatePlan,
  HistoryStore,
  Project,
  RestoreResult,
  ScanEvent,
  ScanSnapshot,
  Settings,
  WakeEvent,
  WakePlan,
} from "@/types";

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
  startScan: (roots) => invoke("start_scan", { roots }),
  cancelScan: () => invoke("cancel_scan"),
  onScanEvent: (h) => on<ScanEvent>("scan-event", h),

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
};
