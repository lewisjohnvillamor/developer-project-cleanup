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
- Git-aware safety: anything Git tracks is protected, `.gitignore` confirms a
  folder is generated.
- Windows reparse points detected explicitly in removal and measurement.
- Allocated size instead of apparent size, and "moved" rather than "freed"
  when a removal goes to Trash.
- A log file, and failed writes reported in the window instead of discarded.
- Receipts: one test manifests a hostile fixture before and after a real
  cleanup and fails if anything changed that the plan did not name
  (`crates/hibernate-core/tests/nothing_outside_is_touched.rs`).

## Decided

- **The name stays "Project Hibernate".** The collision with the Hibernate ORM
  was raised and accepted: this is a desktop app for developers, not a Java
  library, and the crate, bundle and package names are all unclaimed. Please
  do not reopen this without a new argument.

## Next (in order)

1. **Re-measure at review time** so the confirmed number is not a cached one.
2. Updater against GitHub releases.
3. Homebrew, winget, Scoop, crates.io.

## Later, maybe

- Docker artifact awareness (read-only report, like the toolchain caches).
- Vulnerability audit for dormant projects (#20). Proposed, undecided: the
  engine has never made a network call, and that is worth more than the
  feature unless the offline path is the default.
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
