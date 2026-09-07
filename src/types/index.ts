// Mirrors of the Rust types in crates/hibernate-core/src/model.rs and friends.
// Field names are camelCase because the Rust side serialises with
// `#[serde(rename_all = "camelCase")]`.

export type Stack =
  | "node"
  | "rust"
  | "python"
  | "go"
  | "maven"
  | "gradle"
  | "dotnet"
  | "cocoapods"
  | "dart"
  | "ruby"
  | "php"
  | "swift"
  | "elixir"
  | "haskell"
  | "zig"
  | "unity"
  | "terraform";

export const STACK_LABELS: Record<Stack, string> = {
  node: "Node",
  rust: "Rust",
  python: "Python",
  go: "Go",
  maven: "Maven",
  gradle: "Gradle",
  dotnet: ".NET",
  cocoapods: "CocoaPods",
  dart: "Dart",
  ruby: "Ruby",
  php: "PHP",
  swift: "Swift",
  elixir: "Elixir",
  haskell: "Haskell",
  zig: "Zig",
  unity: "Unity",
  terraform: "Terraform",
};

export type Safety = "safe" | "review" | "protected";

export type ArtifactCategory =
  | "nodeDependencies"
  | "rustTarget"
  | "buildArtifacts"
  | "pythonEnvironments"
  | "caches"
  | "other";

export const CATEGORY_LABELS: Record<ArtifactCategory, string> = {
  nodeDependencies: "Node dependencies",
  rustTarget: "Rust target",
  buildArtifacts: "Build artifacts",
  pythonEnvironments: "Python environments",
  caches: "Caches",
  other: "Other",
};

export type ProjectStatus = "active" | "dormant" | "hibernated" | "protected";

export type GitState = "clean" | "modified" | "untracked" | "noRepo" | "remoteMissing" | "unknown";

export const GIT_STATE_LABELS: Record<GitState, string> = {
  clean: "Clean",
  modified: "Modified",
  untracked: "Untracked files",
  noRepo: "No Git repository",
  remoteMissing: "Remote missing",
  unknown: "Unknown",
};

export interface GitInfo {
  isRepo: boolean;
  state: GitState;
  isClean: boolean | null;
  modifiedCount: number;
  untrackedCount: number;
  remoteConfigured: boolean | null;
  branch: string | null;
  lastCommitAt: string | null;
}

export interface CleanupArtifact {
  path: string;
  relativePath: string;
  kind: string;
  category: ArtifactCategory;
  bytes: number;
  fileCount: number;
  dirCount: number;
  safety: Safety;
  regeneratable: boolean;
  explanation: string;
  restoreHint: string | null;
}

export interface HibernationRecord {
  hibernatedAt: string;
  previousBytes: number;
  savedBytes: number;
  historyEntryId: string;
}

export interface ActivitySources {
  sourceModifiedAt: string | null;
  gitCommitAt: string | null;
  appActivityAt: string | null;
}

export interface Project {
  id: string;
  name: string;
  path: string;
  scanRoot: string;
  parentPath: string | null;
  stacks: Stack[];
  frameworks: string[];
  workspaceMembers: string[];
  scanDurationMs: number;
  cacheHits: number;
  packageManager: string | null;
  totalBytes: number;
  reclaimableBytes: number;
  reviewBytes: number;
  fileCount: number;
  lastActivityAt: string | null;
  activity: ActivitySources;
  git: GitInfo | null;
  status: ProjectStatus;
  safety: Safety;
  safetyReasons: string[];
  artifacts: CleanupArtifact[];
  protectedEntries: string[];
  protected: boolean;
  ignoredUntil: string | null;
  hibernation: HibernationRecord | null;
  scannedAt: string;
  warnings: string[];
}

export interface CategoryTotal {
  category: ArtifactCategory;
  bytes: number;
  count: number;
}

export interface ScanSummary {
  roots: string[];
  projectCount: number;
  totalBytes: number;
  reclaimableBytes: number;
  reviewBytes: number;
  byCategory: CategoryTotal[];
  durationMs: number;
  warningCount: number;
  finishedAt: string;
  cacheHits: number;
}

export type ScanEvent =
  | { type: "started"; roots: string[] }
  | { type: "discovered"; path: string; name: string; stacks: Stack[]; discovered: number }
  | { type: "scanned"; project: Project; scanned: number; discovered: number }
  | { type: "warning"; path: string | null; message: string }
  | { type: "finished"; summary: ScanSummary }
  | { type: "cancelled"; scanned: number; discovered: number };

export interface ScanSnapshot {
  projects: Project[];
  summary: ScanSummary | null;
  scannedAt: string | null;
}

export type Theme = "system" | "light" | "dark";
export type Disposition = "trash" | "quarantine" | "permanent";

export interface CleanupRule {
  id: string;
  pattern: string;
  ecosystems: Stack[];
  safety: Safety;
  regeneratable: boolean;
  category: ArtifactCategory;
  explanation: string;
  restoreHint: string | null;
  builtin: boolean;
}

export interface Settings {
  theme: Theme;
  rememberFolders: boolean;
  scanRoots: string[];
  maxConcurrency: number;
  followSymlinks: boolean;
  scanHidden: boolean;
  inspectGit: boolean;
  disposition: Disposition;
  quarantineRetentionDays: number;
  includeReviewItems: boolean;
  dormantAfterDays: number;
  ignoredPaths: string[];
  customRules: CleanupRule[];
  protectedPatterns: string[];
  incrementalScans: boolean;
  scheduledScanHours: number;
  notifyThresholdBytes: number;
}

export const DEFAULT_SETTINGS: Settings = {
  theme: "system",
  rememberFolders: true,
  scanRoots: [],
  maxConcurrency: 0,
  followSymlinks: false,
  scanHidden: true,
  inspectGit: true,
  disposition: "trash",
  quarantineRetentionDays: 7,
  includeReviewItems: false,
  dormantAfterDays: 14,
  ignoredPaths: [],
  customRules: [],
  protectedPatterns: [],
  incrementalScans: true,
  scheduledScanHours: 0,
  notifyThresholdBytes: 1_000_000_000,
};

export interface SelectedProject {
  projectId: string;
  artifactPaths?: string[] | null;
}

export interface HibernateRequest {
  selection: SelectedProject[];
  includeReview: boolean;
}

export interface PlannedProject {
  id: string;
  name: string;
  path: string;
  totalBytes: number;
  artifacts: CleanupArtifact[];
  bytes: number;
  skippedReview: CleanupArtifact[];
  warnings: string[];
}

export interface HibernatePlan {
  projects: PlannedProject[];
  totalBytes: number;
  folderCount: number;
  fileCount: number;
  reviewCount: number;
  skippedProtected: string[];
  skippedUnknown: string[];
  disposition: Disposition;
}

export interface FailedPath {
  path: string;
  error: string;
}

export type ArtifactOutcome =
  | { kind: "trashed" }
  | { kind: "quarantined"; quarantinePath: string }
  | { kind: "deleted" }
  | { kind: "partiallyDeleted"; failed: FailedPath[] }
  | { kind: "failed"; error: string }
  | { kind: "skipped"; reason: string }
  | { kind: "restored" };

export function outcomeIsError(o: ArtifactOutcome): boolean {
  return o.kind === "partiallyDeleted" || o.kind === "failed" || o.kind === "skipped";
}

export function describeOutcome(o: ArtifactOutcome): string {
  switch (o.kind) {
    case "trashed":
      return "Moved to Trash";
    case "quarantined":
      return "Quarantined";
    case "deleted":
      return "Deleted";
    case "restored":
      return "Restored";
    case "partiallyDeleted":
      return `${o.failed.length} item${o.failed.length === 1 ? "" : "s"} could not be removed`;
    case "failed":
      return `Failed: ${o.error}`;
    case "skipped":
      return `Skipped: ${o.reason}`;
  }
}

export interface HistoryArtifact {
  path: string;
  relativePath: string;
  kind: string;
  bytes: number;
  outcome: ArtifactOutcome;
}

export interface HistoryProject {
  projectId: string;
  name: string;
  path: string;
  previousBytes: number;
  bytesRecovered: number;
  artifacts: HistoryArtifact[];
  errorCount: number;
}

export interface HistoryEntry {
  id: string;
  startedAt: string;
  finishedAt: string;
  disposition: Disposition;
  projects: HistoryProject[];
  totalRecovered: number;
  projectCount: number;
  errorCount: number;
  cancelled: boolean;
  restoredAt: string | null;
}

export function entryRestorable(e: HistoryEntry, trashRestoreSupported = false): boolean {
  return (
    e.restoredAt === null &&
    e.projects.some((p) => p.artifacts.some((a) => a.outcome.kind === "quarantined" || (trashRestoreSupported && a.outcome.kind === "trashed")))
  );
}

export interface HistoryStore {
  entries: HistoryEntry[];
}

export type HibernateEvent =
  | { type: "started"; entryId: string; projectCount: number; totalBytes: number }
  | { type: "projectStarted"; projectId: string; name: string; index: number }
  | { type: "artifactStarted"; projectId: string; relativePath: string; bytes: number }
  | {
      type: "artifactFinished";
      projectId: string;
      relativePath: string;
      bytesRecovered: number;
      outcome: ArtifactOutcome;
    }
  | {
      type: "projectFinished";
      projectId: string;
      name: string;
      bytesRecovered: number;
      errorCount: number;
      completed: number;
      total: number;
      totalRecovered: number;
    }
  | { type: "finished"; entry: HistoryEntry };

export interface WakeStep {
  program: string;
  args: string[];
  display: string;
}

export interface WakePlan {
  projectId: string;
  cwd: string;
  packageManager: string | null;
  steps: WakeStep[];
  notes: string[];
  missingTools: string[];
}

export type WakeEvent =
  | { type: "stepStarted"; projectId: string; stepIndex: number; display: string }
  | { type: "line"; projectId: string; text: string }
  | { type: "stepFinished"; projectId: string; stepIndex: number; exitCode: number; durationMs: number }
  | { type: "finished"; projectId: string; success: boolean; durationMs: number; message: string };

export interface AppInfo {
  version: string;
  platform: string;
  dataDir: string;
  quarantineDir: string;
  quarantineBytes: number;
  gitAvailable: boolean;
  trashAvailable: boolean;
  trashRestoreSupported: boolean;
  homeDir: string | null;
}

export interface FolderInfo {
  path: string;
  exists: boolean;
  warning: string | null;
}

export interface RestoreResult {
  restored: number;
  errors: string[];
  entry: HistoryEntry;
}

export interface GlobalCache {
  id: string;
  label: string;
  path: string;
  exists: boolean;
  bytes: number;
  fileCount: number;
  cleanCommand: string | null;
  note: string;
}

export interface RuleMatch {
  projectName: string;
  projectPath: string;
  path: string;
  relativePath: string;
  bytes: number;
  fileCount: number;
}

export interface QuarantineBatch {
  entryId: string;
  date: string;
  path: string;
  bytes: number;
  fileCount: number;
  projects: string[];
}

export interface ScanRecord {
  at: string;
  projectCount: number;
  totalBytes: number;
  reclaimableBytes: number;
  reviewBytes: number;
}

export interface ScanTrend {
  records: ScanRecord[];
}

export type ExportFormat = "csv" | "json";

export interface CacheInfo {
  entries: number;
}
