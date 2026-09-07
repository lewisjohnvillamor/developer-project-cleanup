# Roadmap

## The guiding rule

Every feature request is judged by one question:

> **Does this make it easier or safer for a developer to reclaim storage from
> dormant projects?**

If the answer is no, it will be closed with a link to this file, however nice
the idea is. The product is a project library manager for disk space, not a
general cleaner.

## Core promise (never negotiable)

- No source file, Git history, lockfile, configuration or user data is ever
  removed. Rules match generated directories only.
- Nothing is removed without a review the user confirmed.
- Every removal is reversible by default (Trash or quarantine).

## Done

- v0.1: scanning, stacks, reclaimable sizes, filters, selection, protection,
  explanations, review-then-hibernate, history, quarantine restore, wake,
  themes, CLI.
- Incremental scans, scheduled scans, glob-aware monorepos, six more
  ecosystems, rule preview, Trash restore (Windows/Linux), export, trend,
  command palette, keyboard navigation.

## Next (in order)

1. **Git-aware safety.** Treat anything Git tracks as protected, and use
   `.gitignore` as confirmation that a folder is generated.
2. **Windows reparse points.** Explicit junction detection in removal and
   measurement, with tests on a Windows runner.
3. **Allocated size** instead of apparent size (hardlinked pnpm stores,
   compressed folders), and honest wording for Trash ("moved", not "freed").
4. **Re-measure at review time** so the confirmed number is not a cached one.
5. **Log file + surfaced save errors.**
6. Updater against GitHub releases.
7. Homebrew, winget, Scoop, crates.io.

## Later, maybe

- Docker artifact awareness (read-only report, like the toolchain caches).
- Project archival to cold storage, with restore.
- IDE integration ("hibernate this project" from the editor).
- Localization once there is a second language to ship.

## Not planned

Accounts, cloud sync, a SaaS dashboard, AI assistants, OS-wide cleaning,
registry or browser cache cleaning, duplicate finders, team workspaces,
billing, telemetry. These have been declined before and will be again.

## Extras vs core

The toolchain-cache report, the trend chart and the command palette are
conveniences. If they ever conflict with the core flow they lose.
