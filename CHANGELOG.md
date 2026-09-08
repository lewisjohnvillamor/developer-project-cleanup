# Changelog

## Unreleased

### Fixed
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
