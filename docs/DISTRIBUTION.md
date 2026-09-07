# Distribution

Where the app and CLI can be installed from, and what each channel needs
per release. All templates live in `packaging/`.

| Channel | Status | Per-release work |
| --- | --- | --- |
| GitHub Releases (all platforms) | **live** via `release.yml` | tag `vX.Y.Z` |
| crates.io: `hibernate-cli`, `hibernate-core` | names are free (checked 2026-09) | `cargo publish -p hibernate-core && cargo publish -p hibernate-cli` |
| Homebrew tap (cask + formula) | template | bump version, fill sha256 |
| winget | template | `wingetcreate update … --submit` |
| Scoop bucket | template | bump version, fill sha256 |
| Tauri updater | not enabled | one-time key setup, see below |
| AUR / Flathub / Snap | not started | community-maintained is fine |

## crates.io

```bash
cargo login
cargo publish -p hibernate-core --dry-run
cargo publish -p hibernate-core
cargo publish -p hibernate-cli      # depends on the core version just published
```

`hibernate-cli` currently depends on `hibernate-core` by path; add
`version = "0.1.0"` next to `path = "../hibernate-core"` before publishing so
crates.io resolves it. After publishing, users get the CLI with
`cargo install hibernate-cli` (binary name `hibernate`).

## Homebrew

Create a tap repository `homebrew-tap` with `Casks/project-hibernate.rb` and
`Formula/hibernate-cli.rb` from `packaging/homebrew/`. Users run:

```bash
brew tap <owner>/tap
brew install --cask project-hibernate    # desktop app
brew install hibernate-cli               # CLI, built from source
```

The cask requires a notarized `.dmg` or users see Gatekeeper warnings; see
`docs/RELEASING.md`.

## winget

Windows installers must be code-signed or winget moderation rejects them.
After a signed `.msi` is on the release, run:

```powershell
wingetcreate update ProjectHibernate.ProjectHibernate --urls <msi url> --version X.Y.Z --submit
```

The first submission uses the singleton manifest in `packaging/winget/` split
into the three files `wingetcreate new` produces.

## Scoop

Add `packaging/scoop/project-hibernate.json` to a bucket repository; `checkver`
and `autoupdate` are set so the bucket's `bin/checkver.ps1 -u` can bump it.

## Updater

The Tauri updater checks a `latest.json` that `tauri-action` generates on each
release, and verifies bundles with a minisign key. One-time setup:

```bash
npm run tauri signer generate -- -w ~/.tauri/project-hibernate.key
```

1. Put the **public** key in `src-tauri/tauri.conf.json`:
   ```json
   "plugins": {
     "updater": {
       "pubkey": "<public key>",
       "endpoints": ["https://github.com/lewisjohnvillamor/developer-project-cleanup/releases/latest/download/latest.json"]
     }
   }
   ```
2. Add `TAURI_SIGNING_PRIVATE_KEY` (and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`)
   as repository secrets; `release.yml` already forwards them.
3. Add `tauri-plugin-updater = "2"` to `src-tauri/Cargo.toml`, register it in
   `lib.rs`, grant `updater:default` in `capabilities/default.json`, and add a
   "Check for updates" action (the command palette is the natural place).

Losing the private key means existing installs can never update again; back
it up somewhere that is not the repository.
