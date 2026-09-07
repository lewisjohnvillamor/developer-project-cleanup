# Releasing

Releases are built by `.github/workflows/release.yml` when a `v*` tag is
pushed. It produces a **draft** GitHub release with:

| Platform | Artifacts |
| --- | --- |
| Windows x64 | `.msi` (WiX) and `.exe` (NSIS) installers |
| macOS arm64 / x64 | `.dmg` and `.app` |
| Linux x64 | `.deb`, `.rpm` and `.AppImage` |

```bash
# 1. bump the version in package.json, src-tauri/tauri.conf.json and Cargo.toml [workspace.package]
# 2. update CHANGELOG.md
git commit -am "Release v0.2.0"
git tag v0.2.0
git push origin main v0.2.0
# 3. review the draft release on GitHub and publish it
```

Unsigned builds work everywhere but show SmartScreen / Gatekeeper warnings.
Signing is optional and configured entirely through repository secrets.

## macOS signing and notarization

1. Create a *Developer ID Application* certificate in your Apple Developer
   account and export it as a `.p12`.
2. Add these repository secrets:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | the `.p12` file, base64-encoded (`base64 -i cert.p12`) |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12` export password |
| `KEYCHAIN_PASSWORD` | any string; used for the temporary CI keychain |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | the Apple ID used for notarization |
| `APPLE_PASSWORD` | an app-specific password for that Apple ID |
| `APPLE_TEAM_ID` | your team id |

`tauri-action` signs, notarizes and staples the bundle when these are set.

## Windows code signing

The workflow is wired for [Azure Trusted Signing](https://tauri.app/distribute/sign/windows/):
set `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID` and add the
`windows.signCommand` block from the Tauri docs to `src-tauri/tauri.conf.json`.
Any other signtool-based flow works the same way through `signCommand`.

## Updater

Not enabled yet. When it is, generate a key pair with `npm run tauri signer generate`,
put the public key in `tauri.conf.json` under `plugins.updater.pubkey` and the private key
in the `TAURI_SIGNING_PRIVATE_KEY` secret; `tauri-action` then emits `latest.json`.

## Before tagging

```bash
npm run check              # typecheck, unit tests, production build
cargo test                 # engine + CLI
cargo clippy --all-targets -- -D warnings
npx vite --port 1420 &  && npm run e2e   # UI walkthrough against the mock
```
