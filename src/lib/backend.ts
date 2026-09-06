// The contract between the UI and whatever runs the engine. In the desktop
// app this is Tauri; in a plain browser (npm run dev) it is an in-memory mock
// so the UI can be developed and screenshotted without Rust.

import type {
  AppInfo,
  FolderInfo,
  HibernateEvent,
  HibernatePlan,
  HibernateRequest,
  HistoryStore,
  Project,
  RestoreResult,
  ScanEvent,
  ScanSnapshot,
  Settings,
  WakeEvent,
  WakePlan,
} from "@/types";

export type Unlisten = () => void;

export interface Backend {
  readonly kind: "tauri" | "mock";
  getAppInfo(): Promise<AppInfo>;
  getSettings(): Promise<Settings>;
  updateSettings(settings: Settings): Promise<Settings>;
  pickFolders(): Promise<string[]>;
  validateFolder(path: string): Promise<FolderInfo>;

  getLastScan(): Promise<ScanSnapshot>;
  startScan(roots: string[]): Promise<void>;
  cancelScan(): Promise<void>;
  onScanEvent(handler: (e: ScanEvent) => void): Promise<Unlisten>;

  setProtected(projectId: string, protected_: boolean): Promise<Project>;
  ignoreProject(projectId: string, days: number | null): Promise<Project>;
  unignoreProject(projectId: string): Promise<Project>;
  onProjectsUpdated(handler: (projects: Project[]) => void): Promise<Unlisten>;

  planHibernate(request: HibernateRequest): Promise<HibernatePlan>;
  startHibernate(request: HibernateRequest): Promise<void>;
  cancelHibernate(): Promise<void>;
  onHibernateEvent(handler: (e: HibernateEvent) => void): Promise<Unlisten>;

  getHistory(): Promise<HistoryStore>;
  restoreEntry(entryId: string): Promise<RestoreResult>;

  getWakePlan(projectId: string): Promise<WakePlan | null>;
  startWake(projectId: string): Promise<WakePlan>;
  cancelWake(): Promise<void>;
  onWakeEvent(handler: (e: WakeEvent) => void): Promise<Unlisten>;

  openProjectFolder(projectId: string): Promise<void>;
  copyText(text: string): Promise<void>;
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
