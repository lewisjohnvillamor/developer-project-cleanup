# Developer Project Cleanup Manager
## Product, UX, UI, and Technical Specification

> Working concept: a local-first desktop application for developers who keep many side projects and want to reclaim disk space without deleting source code.

---

## 1. Product Summary

Developers often keep dozens or hundreds of projects on disk. Even dormant projects may contain large, fully regeneratable directories such as:

- `node_modules/`
- `.next/`
- `dist/`
- `build/`
- `target/`
- `.venv/`
- `coverage/`
- `.turbo/`
- `.cache/`
- `Pods/`
- framework-specific generated files

The application scans developer project folders, identifies project boundaries and technology stacks, calculates how much storage can safely be reclaimed, and lets the user selectively **Hibernate**, **Wake**, **Protect**, or **Ignore** projects through a desktop UI.

The application does **not** delete source code by default.

---

# 2. Core Product Idea

The product should feel like a **project library manager**, not a generic disk cleaner.

The main question it answers is:

> "Which projects are taking space, what can safely be removed, and how much will I recover?"

Instead of exposing raw folders first, the primary object in the UI is the **project**.

Example:

| Project | Stack | Last Active | Total Size | Reclaimable | Status |
|---|---|---:|---:|---:|---|
| Pointsy | Rust | 4 days ago | 3.8 GB | 3.1 GB | Active |
| BrowserSnaps | Node | 14 days ago | 1.2 GB | 890 MB | Dormant |
| LogParser | Rust | 2 months ago | 8.1 GB | 7.6 GB | Dormant |
| Purple Rally | Next.js | Today | 2.5 GB | 1.9 GB | Protected |

---

# 3. Product Principles

## 3.1 Local-first

The application should work entirely on the user's computer.

No account is required.

No cloud backend is required.

No source code leaves the machine.

No analytics should inspect file contents.

Optional anonymous telemetry may be considered later, but it should never be required for the product to work.

---

## 3.2 Conservative by default

A cleanup tool must earn trust.

The application should prefer:

- Preview before deletion
- Clear explanations
- Reversible actions when possible
- Recycle Bin / Trash or quarantine for initial releases
- Protected paths
- Explicit confirmation for risky operations
- No deletion of source files

---

## 3.3 Project-aware

Do not merely search for `node_modules`.

Understand the project.

Examples:

- `package.json` → JavaScript / TypeScript
- `Cargo.toml` → Rust
- `pyproject.toml` → Python
- `requirements.txt` → Python
- `go.mod` → Go
- `pom.xml` → Maven
- `build.gradle` → Gradle
- `.sln` / `.csproj` → .NET
- `Podfile` → iOS / CocoaPods
- `pubspec.yaml` → Dart / Flutter

A monorepo may contain multiple technology stacks.

---

# 4. Terminology

## Hibernate

Remove safe, regeneratable project artifacts while keeping:

- Source
- Git history
- Assets
- Configuration
- Lockfiles
- Documentation
- User-created data

Example:

```text
BrowserSnaps
1.42 GB before

Hibernate removes:
node_modules/       1.17 GB
.next/               211 MB
coverage/             18 MB

Project after hibernation:
31 MB

Recovered:
1.39 GB
```

---

## Wake

Restore a hibernated project's working dependencies by running the appropriate package/toolchain command.

Examples:

```bash
npm ci
pnpm install --frozen-lockfile
yarn install --immutable
cargo build
pip install -r requirements.txt
bundle install
```

Wake should always show the command before execution.

---

## Protect

Mark a project as excluded from bulk cleanup.

Protected projects should never become selected automatically.

---

## Ignore

Hide a project temporarily or permanently.

Suggested options:

- 7 days
- 30 days
- 90 days
- Until manually restored

---

# 5. Target User

Primary user:

- Developer
- Keeps many side projects
- Uses Windows, macOS, or Linux
- Frequently experiments with frameworks
- Has projects that may sit unused for months
- Does not want to delete source repositories
- Wants quick visibility into wasted disk space

Common folders:

```text
~/Projects
~/Developer
~/Code
~/Documents/Projects
C:\Users\<name>\Downloads\Passion
D:\Projects
```

---

# 6. Primary User Journey

## First Run

1. Launch application.
2. Show a simple welcome screen.
3. User selects one or more project folders.
4. Application scans folders.
5. Dashboard displays:
   - Project count
   - Total size
   - Reclaimable size
   - Potential percentage recovered
6. Projects appear in a sortable table/grid.
7. User selects dormant projects.
8. User previews cleanup.
9. User confirms Hibernate.
10. Application displays progress.
11. Final result shows reclaimed space.

Example result:

> **42.7 GB recovered from 18 projects.**

---

# 7. Main Navigation

Keep navigation minimal.

```text
Overview
Projects
History
Settings
```

Avoid excessive nested navigation.

---

# 8. UI Structure

## 8.1 Main Window

Suggested desktop layout:

```text
┌───────────────────────────────────────────────────────────────┐
│ App Name                                      Scan Projects   │
├──────────────┬────────────────────────────────────────────────┤
│              │                                                │
│ Overview     │   Developer Projects                           │
│ Projects     │                                                │
│ History      │   103 projects                                 │
│ Settings     │   287 GB total                                 │
│              │   94.6 GB reclaimable                          │
│              │                                                │
│              │   Filters / Search                             │
│              │                                                │
│              │   Project Table                                │
│              │                                                │
└──────────────┴────────────────────────────────────────────────┘
```

Sidebar should be narrow and unobtrusive.

---

# 9. Overview Dashboard

The Overview page should answer three questions immediately:

1. How much space are my projects using?
2. How much can I safely recover?
3. Which projects are responsible?

Suggested summary cards:

```text
103
Projects

287 GB
Total Project Size

94.6 GB
Safely Reclaimable

33%
Potential Recovery
```

Optional visualization:

```text
Reclaimable storage by category

Node dependencies     41.2 GB
Rust target           28.7 GB
Build artifacts       11.4 GB
Python environments    8.1 GB
Caches                 5.2 GB
```

Do not make charts the primary interface.

The project list matters more.

---

# 10. Projects Screen

This is the core screen.

Suggested columns:

| Column | Description |
|---|---|
| Checkbox | Select for action |
| Project | Folder/project name |
| Stack | Detected technologies |
| Status | Active / Dormant / Hibernated / Protected |
| Last Active | Last meaningful project activity |
| Total Size | Entire project size |
| Reclaimable | Safe cleanup amount |
| Safety | Green / Yellow |
| Action | Hibernate / Wake |

Suggested table:

```text
☐ Pointsy            Rust         4d     3.8 GB   3.1 GB    Active
☐ Purple Rally       Next.js      0d     2.5 GB   1.9 GB    Protected
☑ BrowserSnaps       Node         14d    1.2 GB   890 MB    Dormant
☑ LogParser          Rust         67d    8.1 GB   7.6 GB    Dormant
```

---

# 11. Search and Filtering

The project list should support fast filtering.

Filters:

- Search project name
- Stack
- Project status
- Last active
- Reclaimable size
- Total size
- Git state
- Protected
- Hibernated

Quick filters:

```text
Inactive > 30 days
Inactive > 90 days
> 500 MB reclaimable
> 1 GB reclaimable
Node
Rust
Python
Git clean
Not protected
```

Filters should be combinable.

---

# 12. Bulk Selection

Bulk selection is important.

Actions:

```text
Select All Visible
Select Dormant
Select > 1 GB
Select Inactive > 30 Days
Deselect Protected
Clear Selection
```

The bulk action bar should show:

```text
18 projects selected

Estimated recovery:
42.7 GB

[Review] [Hibernate]
```

Never automatically include protected projects.

---

# 13. Project Details Drawer

Clicking a project should open a right-side drawer rather than navigating away.

Example:

```text
BrowserSnaps

Path
C:\Users\Lewis\Downloads\Passion\BrowserSnaps

Stack
Node.js
TypeScript
Chrome Extension

Last Active
14 days ago

Git
Clean
Remote configured

Total Size
1.42 GB

Reclaimable
1.39 GB
```

Cleanup breakdown:

```text
node_modules/     1.17 GB     Safe
.next/             211 MB     Safe
coverage/           18 MB     Safe
dist/               12 MB     Safe
```

Protected:

```text
src/
public/
package.json
package-lock.json
.git/
README.md
.env
```

Actions:

```text
Hibernate
Protect Project
Ignore
Open Folder
Copy Path
```

---

# 14. Cleanup Safety Levels

## Green — Regeneratable

Safe by default.

Examples:

```text
node_modules/
.next/
.nuxt/
dist/
build/
target/
coverage/
.vite/
.turbo/
.parcel-cache/
__pycache__/
.pytest_cache/
```

---

## Yellow — Review Recommended

Potentially regeneratable, but behavior may vary.

Examples:

```text
.venv/
venv/
Pods/
vendor/
.gradle/
local package caches
generated SDKs
```

Yellow items should not be bulk-selected until the user opts in.

---

## Red — Protected

Never remove automatically.

Examples:

```text
src/
app/
pages/
public/
assets/
uploads/
data/
.env
.env.*
*.db
*.sqlite
migrations/
.git/
README*
package.json
lockfiles
Cargo.toml
Cargo.lock
```

Users may create additional protected rules.

---

# 15. Hibernate Confirmation

Before cleanup, show a final review.

Example:

```text
Hibernate 18 projects?

Estimated recovery:
42.7 GB

Safe items:
1,267 folders
36,412 files

Review items:
0

Protected files:
untouched

Projects remain usable after reinstalling dependencies.
```

Buttons:

```text
Cancel
Review Files
Hibernate 18 Projects
```

Avoid generic destructive labels such as "DELETE EVERYTHING".

---

# 16. Cleanup Progress UI

Cleanup should be visibly deterministic.

Example:

```text
Hibernate Projects

7 / 18 completed

BrowserSnaps
✓ 1.39 GB recovered

LogParser
✓ 7.61 GB recovered

OldDashboard
Cleaning node_modules...
```

Show:

- Current project
- Current artifact
- Completed projects
- Recovered amount
- Errors
- Cancel button where safe

---

# 17. Completion Screen

Example:

```text
Cleanup Complete

42.7 GB recovered
18 projects hibernated
0 errors

Largest savings:

LogParser          7.61 GB
OldDashboard       5.82 GB
HomeCloud          4.91 GB
```

Actions:

```text
Done
View History
Open Projects
```

---

# 18. Hibernated Projects

A hibernated project remains visible.

Example:

```text
BrowserSnaps

Status:
Hibernated

Current size:
31 MB

Previously:
1.42 GB

Saved:
1.39 GB

[Wake Project]
```

---

# 19. Wake Flow

When the user clicks Wake:

```text
Wake BrowserSnaps

Detected package manager:
npm

Command:

npm ci

This command will run inside:

C:\Users\Lewis\Downloads\Passion\BrowserSnaps
```

Buttons:

```text
Cancel
Run Command
```

Output panel:

```text
Installing dependencies...

added 642 packages

Completed in 31.8 seconds.
```

Do not run arbitrary scripts without clear user visibility.

---

# 20. Activity Detection

"Last Active" should be calculated carefully.

Possible signals:

1. Latest Git commit
2. Latest source file modification
3. Latest meaningful file modification
4. Last time opened through the app

Avoid allowing generated files like `.next/cache` to make an abandoned project appear recently active.

Suggested priority:

```text
last_activity =
max(
    latest source modification,
    latest Git commit,
    application activity record
)
```

Exclude generated directories from activity calculation.

---

# 21. Git Awareness

Git information is useful for trust.

Possible statuses:

```text
Clean
Modified
Untracked files
No Git repository
Remote missing
```

Optional warning:

> This project contains uncommitted changes.

Do not block cleanup if only safe generated artifacts will be removed, but communicate clearly.

---

# 22. Scan Engine

The scanner should:

1. Traverse user-selected roots.
2. Identify project roots.
3. Detect technology.
4. Detect generated artifacts.
5. Calculate sizes.
6. Detect nested projects.
7. Detect Git information.
8. Calculate meaningful last activity.
9. Calculate reclaimable space.
10. Return structured project data to the UI.

Scanning must happen outside the UI thread.

---

# 23. Performance Requirements

The application may scan hundreds of thousands or millions of files.

Requirements:

- Async/background scanning
- Bounded concurrency
- Incremental UI updates
- Ability to cancel scan
- Avoid loading entire directory trees into memory
- Cache previous results
- Re-scan only changed projects where possible

Target:

> First useful project results should appear before the complete scan finishes.

---

# 24. Ignore Rules

Support application-level ignored folders.

Default ignored paths may include:

```text
.git/
.idea/
.vscode/
```

These directories may still be counted toward total size where appropriate, but they should not be treated as reclaimable.

Allow user configuration.

Possible configuration file:

```toml
[scanner]
ignored_paths = [
  "C:\\Windows",
  "C:\\Program Files"
]

[protected_projects]
paths = [
  "C:\\Users\\Lewis\\Downloads\\Passion\\pointsy"
]
```

---

# 25. Recycle Bin / Quarantine

Initial releases should prioritize reversibility.

Preferred hierarchy:

### Option A
Move cleanable artifacts to operating system Trash / Recycle Bin.

### Option B
Use application quarantine.

Example:

```text
AppData/
  quarantine/
    2026-09-07/
      BrowserSnaps/
      LogParser/
```

Quarantine may automatically expire after a configurable period.

Example:

```text
Keep quarantine for:
7 days
```

Permanent deletion can be offered later.

---

# 26. History

History should be lightweight.

Example:

```text
September 7, 2026

18 projects hibernated
42.7 GB recovered

BrowserSnaps      1.39 GB
LogParser         7.61 GB
HomeCloud         4.91 GB
```

Possible actions:

- View details
- Restore from quarantine
- Re-open project folder

---

# 27. Settings

Keep settings minimal.

## General

```text
Theme
System / Light / Dark

Launch behavior
Remember previous folders
```

## Scanning

```text
Maximum scan concurrency
Follow symbolic links: Off
Scan hidden folders: On
```

## Safety

```text
Use Recycle Bin
Use quarantine
Permanent delete

Quarantine retention:
7 days
```

## Rules

```text
Manage cleanup rules
Manage protected paths
Manage ignored folders
```

---

# 28. Design Direction

The UI should feel like a developer tool rather than a consumer cleaner.

Think:

- GitHub Desktop
- Linear
- VS Code
- Raycast
- modern database/admin tools

Avoid:

- Giant gradients
- Excessive glassmorphism
- "AI SaaS" visuals
- Gamified cleaning animations
- Fake urgency
- Red warning screens everywhere
- Cartoon mascots unless intentionally added later

---

# 29. Visual Hierarchy

The strongest visual number should be:

```text
94.6 GB reclaimable
```

Not:

```text
103 projects
```

The user's goal is reclaiming storage.

Secondary emphasis:

- Safety
- Last activity
- Per-project reclaimable space

---

# 30. Color Semantics

Use the user's OS theme by default.

Semantic states:

```text
Green   Safe
Amber   Review
Red     Protected / destructive
Blue    Selected / informational
Gray    Hibernated / inactive
```

Do not rely on color alone.

Always pair colors with icons and labels.

---

# 31. Icon Framework

Use one icon system consistently.

Recommended:

- Lucide

Possible icons:

```text
Folder
FolderOpen
Archive
ArchiveRestore
Shield
Trash2
HardDrive
Database
Package
Clock
GitBranch
Search
Filter
CheckCircle
AlertTriangle
Settings
```

Do not mix icon libraries unless necessary.

---

# 32. Core Components

Suggested React component tree:

```text
AppShell
├── Sidebar
├── TopBar
├── OverviewPage
│   ├── StorageSummary
│   ├── StorageBreakdown
│   └── RecentCleanup
├── ProjectsPage
│   ├── ProjectToolbar
│   ├── ProjectFilters
│   ├── ProjectTable
│   │   └── ProjectRow
│   ├── BulkActionBar
│   └── ProjectDetailsDrawer
├── HistoryPage
└── SettingsPage
```

---

# 33. Technical Stack

Recommended stack:

## Desktop shell

```text
Tauri
```

## UI

```text
React
TypeScript
Vite
```

## Styling

```text
Tailwind CSS
```

Optional component primitives:

```text
Radix UI
```

## Icons

```text
Lucide
```

## Core scanning engine

```text
Rust
```

Useful Rust capabilities:

- Filesystem traversal
- Parallel size calculation
- Git inspection
- Process execution
- OS Trash integration
- Path safety
- Symlink handling

---

# 34. Architecture

```text
┌─────────────────────────────────────┐
│ React + TypeScript                  │
│                                     │
│ Dashboard                           │
│ Project table                       │
│ Filters                             │
│ Settings                            │
└────────────────┬────────────────────┘
                 │
                 │ Tauri commands/events
                 │
┌────────────────▼────────────────────┐
│ Rust Core                           │
│                                     │
│ Scanner                             │
│ Project detector                    │
│ Cleanup rules                       │
│ Size calculator                     │
│ Git inspector                       │
│ Hibernate engine                    │
│ Wake command resolver               │
│ Quarantine / Trash                  │
└────────────────┬────────────────────┘
                 │
                 ▼
          Local Filesystem
```

No backend server is required.

---

# 35. Suggested Data Model

```ts
type Project = {
  id: string
  name: string
  path: string

  stacks: ProjectStack[]

  totalBytes: number
  reclaimableBytes: number

  lastActivityAt?: string

  git?: {
    isRepo: boolean
    isClean?: boolean
    remoteConfigured?: boolean
  }

  status:
    | "active"
    | "dormant"
    | "hibernated"
    | "protected"

  artifacts: CleanupArtifact[]

  protected: boolean
  ignoredUntil?: string
}
```

Artifact:

```ts
type CleanupArtifact = {
  path: string
  kind: string
  bytes: number

  safety:
    | "safe"
    | "review"
    | "protected"

  regeneratable: boolean
}
```

---

# 36. Rust Module Layout

Suggested core:

```text
src-tauri/
└── src/
    ├── main.rs
    ├── commands/
    │   ├── scan.rs
    │   ├── hibernate.rs
    │   ├── wake.rs
    │   └── settings.rs
    │
    ├── scanner/
    │   ├── mod.rs
    │   ├── traversal.rs
    │   ├── size.rs
    │   └── activity.rs
    │
    ├── projects/
    │   ├── mod.rs
    │   ├── detection.rs
    │   └── stacks.rs
    │
    ├── cleanup/
    │   ├── mod.rs
    │   ├── rules.rs
    │   ├── safety.rs
    │   ├── quarantine.rs
    │   └── trash.rs
    │
    ├── git/
    │   └── mod.rs
    │
    └── config/
        └── mod.rs
```

---

# 37. Frontend Folder Structure

```text
src/
├── app/
│   ├── App.tsx
│   └── routes.tsx
│
├── components/
│   ├── layout/
│   ├── projects/
│   ├── storage/
│   └── common/
│
├── pages/
│   ├── OverviewPage.tsx
│   ├── ProjectsPage.tsx
│   ├── HistoryPage.tsx
│   └── SettingsPage.tsx
│
├── hooks/
├── stores/
├── types/
├── utils/
└── styles/
```

---

# 38. Rule System

Cleanup logic should be driven by declarative rules where possible.

Example concept:

```rust
CleanupRule {
    pattern: "node_modules",
    ecosystems: ["node"],
    safety: Safe,
    regeneratable: true,
}
```

Future user rules:

```text
Custom rule

Folder:
.storybook-cache

Safety:
Safe

Applies to:
All projects
```

---

# 39. MVP Scope

Version 0.1 should contain only the essential workflow.

## Required

- Add one or more scan folders
- Detect project roots
- Detect common stacks
- Calculate project size
- Calculate reclaimable size
- Detect common generated directories
- Show project table
- Search
- Filters
- Select/deselect
- Protect project
- Inspect cleanup details
- Hibernate selected projects
- Show progress
- Show reclaimed space
- Persist project protection
- Dark/light/system theme

---

# 40. MVP Technology Support

Start with stacks that give the highest value.

## Node

Detect:

```text
package.json
package-lock.json
yarn.lock
pnpm-lock.yaml
bun.lock
```

Artifacts:

```text
node_modules/
.next/
.nuxt/
dist/
build/
coverage/
.turbo/
.vite/
```

## Rust

Detect:

```text
Cargo.toml
Cargo.lock
```

Artifacts:

```text
target/
```

## Python

Detect:

```text
pyproject.toml
requirements.txt
Pipfile
poetry.lock
```

Artifacts:

```text
.venv/
venv/
__pycache__/
.pytest_cache/
```

This is enough for the first useful release.

---

# 41. Explicit Non-Goals for MVP

Do **not** initially build:

- User accounts
- Cloud synchronization
- SaaS dashboard
- AI assistant
- Docker cleanup
- Operating-system-wide cleaner
- Registry cleaner
- Duplicate file finder
- Photo cleanup
- Browser cache cleanup
- GitHub integration
- Remote project backup
- IDE plugin
- Team workspace
- Billing

These can distract from the core value.

---

# 42. Edge Cases

The implementation must account for:

## Nested projects

Example:

```text
workspace/
├── frontend/
│   └── package.json
└── backend/
    └── Cargo.toml
```

The scanner should recognize both.

---

## Monorepos

Example:

```text
project/
├── package.json
├── apps/
├── packages/
└── node_modules/
```

Do not double-count shared artifacts.

---

## Symbolic links

Do not recursively follow symlinks by default.

Prevent directory cycles.

---

## Locked files

A process may hold a dependency file open.

Cleanup should:

- continue with other files
- show partial failure
- report exactly what could not be removed

---

## Permissions

Never silently elevate privileges.

Display permission errors clearly.

---

## Network drives

Network drives may be extremely slow.

Warn before deep scanning remote paths.

---

## Huge directories

Display progressive results.

Never freeze the entire UI waiting for complete size calculation.

---

# 43. Safety Requirements

Before any deletion:

- Canonicalize path
- Ensure artifact belongs to selected project
- Ensure artifact matches a cleanup rule
- Never operate outside configured scan roots
- Do not follow symlinks unless explicitly enabled
- Confirm protected paths
- Prevent root filesystem deletion
- Prevent drive-root deletion
- Prevent empty-path operations

Examples that must be impossible:

```text
C:\
/
C:\Users
/home
```

---

# 44. Trust UX

Every cleanup candidate should answer:

```text
Why can this be removed?

node_modules/
Installed packages generated from package-lock.json.

Recovery:
1.17 GB

Restore:
npm ci
```

The application should explain decisions instead of asking users to blindly trust it.

---

# 45. Possible Product Names

Do not block development on naming.

Possible directions:

```text
Project Hibernate
DevHibernate
ProjectNap
ColdCode
CodeDormant
Reclaim
DevSweep
Dormant
ProjectShelf
CodeShelf
```

Naming can be decided after the MVP workflow is proven.

---

# 46. Potential Taglines

```text
Keep the code. Lose the weight.
```

```text
Hibernate old projects. Reclaim your disk.
```

```text
Keep 100 side projects. Store only what matters.
```

```text
Your project library, without the dependency baggage.
```

---

# 47. Success Metrics

For a personal/local application, useful metrics are product outcomes rather than engagement.

Examples:

```text
Disk space recovered
Projects safely hibernated
Average cleanup time
Scan duration
Failed cleanup operations
False-positive cleanup rules
```

The most important metric:

> **Zero user-created files accidentally deleted.**

---

# 48. Release Plan

## v0.1

- Windows first
- Node + Rust + Python
- Project scanning
- Reclaimable size
- Selection
- Protection
- Hibernate
- Cleanup history

## v0.2

- Wake projects
- Quarantine restore
- More ecosystems
- Better monorepo detection

## v0.3

- macOS
- Linux
- Custom cleanup rules
- Scheduled local scanning

## Later

Potential optional features:

- Docker artifact awareness
- IDE integration
- GitHub repository status
- Project archival
- Remote cold storage

Only add these after the core workflow proves useful.

---

# 49. Recommended Build Order

## Phase 1 — Scanner CLI

Before building the polished UI, prove:

```text
scan folder
detect projects
calculate sizes
detect reclaimable artifacts
```

Example output:

```text
BrowserSnaps
Total:       1.42 GB
Reclaimable: 1.39 GB

node_modules    1.17 GB
.next           211 MB
coverage         18 MB
```

---

## Phase 2 — Tauri Shell

Build the desktop shell and connect Rust scanner results to React.

---

## Phase 3 — Projects UI

Implement:

- Table
- Search
- Filters
- Sorting
- Detail drawer

---

## Phase 4 — Hibernate

Implement safe deletion / quarantine.

---

## Phase 5 — Trust and Polish

Add:

- Git state
- Explanation UI
- Progress
- History
- Restore
- Better onboarding

---

# 50. Definition of Done for MVP

The MVP is complete when a developer can:

1. Install the desktop application.
2. Select a project folder containing dozens of repositories.
3. See projects appear progressively.
4. See accurate total and reclaimable sizes.
5. Search and filter dormant projects.
6. Select multiple projects.
7. Protect active projects.
8. Preview exactly what will be removed.
9. Hibernate selected projects safely.
10. See how much disk space was recovered.
11. Reopen all source repositories afterward with source files intact.

---

# 51. Guiding Product Rule

When deciding whether to add a feature, ask:

> **Does this make it easier or safer for a developer to reclaim storage from dormant projects?**

If the answer is no, it probably does not belong in the MVP.

---

# Final Direction

Build this as a **local-first developer desktop application with a polished UI**.

Recommended implementation:

```text
Tauri
React
TypeScript
Rust
Tailwind CSS
Lucide Icons
```

The main product experience should revolve around:

```text
SCAN
  ↓
UNDERSTAND
  ↓
SELECT
  ↓
REVIEW
  ↓
HIBERNATE
  ↓
RECOVER SPACE
```

The core promise:

> **Keep the project. Remove what can be rebuilt.**
