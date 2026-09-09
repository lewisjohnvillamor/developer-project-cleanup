# Changelog

## Unreleased

### Fixed
- **Windows junctions could take their target's contents with them.** A junction reports as a directory and does not always report as a symbolic link, so a recursive delete could descend through one and empty whatever it pointed at. Every walk in the engine now treats *any* reparse point as a link: never descended into, unlinked rather than followed. Symbolic links behaved correctly already; this closes the Windows-specific hole, and a test on the Windows runner creates a real junction and asserts its target survives.

### Added
- **Git decides what is generated, not just the folder name.** A folder whose name matches a cleanup rule but that Git tracks holds committed or staged work, so it is now marked protected and never offered for removal, however well the name matches. Projects that commit their `dist/` are the common case. Conversely, a folder the repository ignores is flagged as confirmed generated. Costs two `git` calls per repository, over candidate folders only, and the details drawer explains which applies.

### Changed
- Dependencies refreshed, superseding Dependabot #2-#10: GitHub Actions majors (checkout, setup-node, upload-artifact v7, tauri-action v1), Vite 8, Vitest 5, `@vitejs/plugin-react` 6, Biome 2 and wait-on 9.
  - Vite 8 replaced esbuild with Oxc, so `build.minify` no longer names `"esbuild"`.
  - Biome 2's config was migrated, its CSS parser told about Tailwind 4's at-rules, and its import-ordering assist applied across the frontend.
  - The name stays **Project Hibernate**; the collision with the Hibernate ORM was considered and accepted, and is recorded in ROADMAP.md.
- Dependabot now groups updates by ecosystem and severity, so a cycle opens about three pull requests instead of nine.

### Fixed
- Flaky UI walkthrough: it waited for the text "Estimated recovery", which appears in both the bulk action bar and the hibernate dialog, so the wait passed before the dialog had opened. The wait is now scoped to the dialog, and the walkthrough waits for the command palette to unmount before sending the next shortcut. CI uploads screenshots when the walkthrough fails.
- **Incremental scans never took effect on Windows.** The tree fingerprint hashed directory modification times, which NTFS updates lazily, so a rescan saw a different fingerprint and re-measured everything. It now hashes file metadata only; directories contribute their name, and changes inside them are caught by their own entries. Sizes were always correct, so this was a lost optimisation rather than a correctness bug. Found by the Windows CI job.
- Three scanner tests compared paths built with `/` against `std` output that uses `\` on Windows; they now use the module's `relative_slash` helper.
- Clippy failures on `main` after Rust 1.98 introduced `unnecessary_sort_by` and `manual_checked_ops`; six call sites updated.
- The dependency audit job failed with "Resource not accessible by integration" because `rustsec/audit-check` needs `checks: write`, which fork pull requests never receive. `cargo audit` now runs directly, and the workflow declares least-privilege `contents: read`.

### Changed
- Rust lint and format now run once on a pinned toolchain (`RUST_LINT_VERSION`) instead of three times on rolling stable, so a Rust release cannot turn `main` red on its own. A separate advisory job reports new lints from the latest stable without failing the build.

### Added
- Incremental scans: `node_modules/`, `target/`, `.git/` and other large trees are fingerprinted (two levels of modification times) and their sizes reused when unchanged. "Full rescan" and a cache-clear button bypass it.
- Scheduled background scans with a system notification when the reclaimable total passes a threshold.
- Per-folder ticking in the hibernate review, including opting single review folders in.
- Glob-aware monorepo detection: only declared workspace members (npm/pnpm workspaces, Cargo `members`, Gradle `include`, Maven modules, `.sln` projects, `go.work`) fold into the root; other nested projects stay separate. Members are listed in the details drawer.
- New ecosystems: Swift Package Manager, Elixir/Mix, Haskell (Cabal/Stack), Zig, Unity, Terraform.
- Custom rule preview: see which folders a draft rule would match, with sizes, before saving it.
- Quarantine panel in History with per-batch purge.
- Restore from the OS Trash on Windows and Linux.
- Read-only toolchain cache report (Cargo registry, npm cache, pnpm store, pip, uv, Go, Gradle, Maven, Composer, CocoaPods) with each tool's clean command.
- CSV / JSON export of the project table; "reclaimable over time" trend on the Overview.
- Command palette (Ctrl/Cmd+K) and keyboard shortcuts; full keyboard navigation of the project table, dialogs and menus.
- Wake dialog warns when a required tool is not on PATH.
- "Exclude from future scans" on a project; slow-folder hints; "only copy" warning for repos with no remote and uncommitted work.
- "Copy diagnostics" for bug reports.
- CLI: `--full`, `caches`, `export`, `quarantine list|purge`.
- Frontend unit + store tests (Vitest), a Playwright walkthrough, and a tag-driven release workflow with optional signing.
- Project table is windowed: only visible rows render, so thousands of projects scroll smoothly; keyboard navigation works across the whole list.
- Tests for the Tauri layer (scheduler decisions, post-cleanup projection) and a desktop smoke test that drives the real binary on Xvfb in CI.

### Project
- README with screenshots and a comparison to kondo, npkill and cargo-sweep.
- CONTRIBUTING, SECURITY (private reporting for data-loss bugs), CODE_OF_CONDUCT, ROADMAP with the one guiding rule, issue templates that ask for diagnostics, PR template.
- Supply-chain gates in CI: cargo audit, cargo deny (licenses, yanked crates, sources), npm audit; Dependabot for Cargo, npm and Actions.
- Biome for TypeScript linting and formatting (`npm run lint`, `npm run format`).
- `npm run version:bump <semver>` updates package.json, the lockfile, tauri.conf.json, Cargo.toml and Cargo.lock together.
- Packaging templates for Homebrew (cask + formula), winget and Scoop, crates.io metadata, and docs/DISTRIBUTION.md including updater key setup.

## 0.1.0

Initial MVP: scanning, stack detection, reclaimable sizes, filters, bulk selection,
protection, per-artifact explanations, review-then-hibernate with progress, history
with quarantine restore, wake, light/dark/system theme, CLI.
