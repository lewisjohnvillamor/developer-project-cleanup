# Contributing

Thanks for helping. This project removes folders from people's disks, so the
bar for changes that touch removal is deliberately high, and the bar for
everything else is deliberately low.

## The one rule

> Does this make it easier or safer for a developer to reclaim storage from
> dormant projects?

If a change does not, it probably belongs in a fork. See [ROADMAP.md](ROADMAP.md)
for what is in and out of scope.

## Setup

```bash
git clone https://github.com/lewisjohnvillamor/developer-project-cleanup
cd developer-project-cleanup
npm install
cargo test              # engine + CLI
npm run check           # typecheck, lint, unit tests, production build
npm run dev             # UI in a browser with mock data
npm run tauri dev       # the desktop app (needs the Tauri system libraries on Linux)
```

Linux needs `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`.

## Where things live

| Path | What |
| --- | --- |
| `crates/hibernate-core` | The engine. No UI code. Every removal goes through `cleanup/safety.rs`. |
| `crates/hibernate-cli` | The `hibernate` command. Thin. |
| `src-tauri` | Tauri commands and events. Thin. Logic belongs in the engine. |
| `src` | React UI. `src/lib/backend.ts` is the contract; `mock-backend.ts` fakes it for the browser. |
| `e2e` | Playwright walkthrough against the mock. |

## Adding a cleanup rule or an ecosystem

1. Add the marker file(s) to `crates/hibernate-core/src/projects/stacks.rs` and,
   if detection needs more than a filename, to `detection.rs`.
2. Add the rule(s) to `cleanup/rules.rs` with an honest `explanation` and a
   `restore_hint`. Choose the safety level conservatively:
   - **Safe**: fully regenerated from committed inputs by one command, and
     nobody edits it by hand (`node_modules/`, `target/`).
   - **Review**: usually regeneratable but some projects patch or commit it
     (`.venv/`, `Pods/`, `vendor/`, `deps/`).
   - Never add a rule for anything that could hold user data.
3. Add the wake command in `wake.rs`.
4. Add the TypeScript label in `src/types/index.ts` and a row in the
   built-in rules list in `src/pages/SettingsPage.tsx`.
5. Add a test in `detection.rs` and, if the rule is unusual, in `rules.rs`.

## Changes that touch removal

Anything in `cleanup/` (safety, remove, trash, quarantine, hibernate) needs:

- a test that shows the new behaviour, and one that shows what it refuses;
- no widening of what `validate_artifact` accepts without a written reason in
  the PR;
- a manual run of `cargo run -p hibernate-cli -- hibernate <folder> --dry-run`
  on a folder you care about, described in the PR.

## Style

- Rust: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`.
- TypeScript: `npm run lint` (Biome). `npm run format` fixes what it can.
- Keep commands and the CLI thin; put behaviour in the engine so both share it.
- User-facing text: plain sentences, no exclamation marks, no scary red screens.
  Say what will happen and how to undo it.

## Pull requests

- One topic per PR. Small is fine.
- Fill in the template. If you skipped tests, say why.
- CI runs fmt, clippy, tests on Linux/Windows/macOS, the UI tests, the
  Playwright walkthrough, a desktop smoke test and dependency audits.

## Reporting bugs

Use the bug template and paste the output of **Settings → Copy diagnostics**.
It contains the version, platform, settings and scan warnings, and no file
contents or project names.

By contributing you agree that your contributions are licensed under the MIT
license that covers the project.
