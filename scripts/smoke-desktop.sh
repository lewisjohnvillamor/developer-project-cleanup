#!/usr/bin/env bash
# Launch the real desktop binary on a virtual display, trigger a scan of a
# synthetic project tree through the UI, and assert the engine's results
# reached the app's data folder. Linux only (Xvfb + xdotool).
#
#   npm run smoke:desktop            # builds the debug binary if needed
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="${SMOKE_DIR:-$(mktemp -d)}"
export DISPLAY="${SMOKE_DISPLAY:-:97}"
export PROJECT_HIBERNATE_DATA_DIR="$WORK/appdata"
mkdir -p "$PROJECT_HIBERNATE_DATA_DIR" "$WORK/Projects"

cleanup() {
  kill ${APP_PID:-} ${XVFB_PID:-} 2>/dev/null || true
  # npx wraps vite in a child process; kill whatever holds the port.
  fuser -k 1420/tcp >/dev/null 2>&1 || kill ${VITE_PID:-} 2>/dev/null || true
}
trap cleanup EXIT

# Synthetic projects: a Node app with node_modules and a Rust crate with target/.
mkdir -p "$WORK/Projects/web/node_modules/pkg" "$WORK/Projects/web/src" "$WORK/Projects/svc/target/debug" "$WORK/Projects/svc/src"
echo '{"name":"web","dependencies":{"react":"18"}}' > "$WORK/Projects/web/package.json"
head -c 300000 /dev/zero > "$WORK/Projects/web/node_modules/pkg/blob"
echo "export {}" > "$WORK/Projects/web/src/index.ts"
printf '[package]\nname = "svc"\nversion = "0.1.0"\n' > "$WORK/Projects/svc/Cargo.toml"
head -c 500000 /dev/zero > "$WORK/Projects/svc/target/debug/svc"
echo "fn main() {}" > "$WORK/Projects/svc/src/main.rs"
echo "{ \"scanRoots\": [\"$WORK/Projects\"] }" > "$PROJECT_HIBERNATE_DATA_DIR/settings.json"

cd "$ROOT"
cargo build -q -p project-hibernate
Xvfb "$DISPLAY" -screen 0 1280x820x24 >/dev/null 2>&1 & XVFB_PID=$!
npx vite --port 1420 --strictPort >"$WORK/vite.log" 2>&1 & VITE_PID=$!
for _ in $(seq 1 60); do curl -sf http://localhost:1420/ >/dev/null && break; sleep 1; done
WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 ./target/debug/project-hibernate >"$WORK/app.log" 2>&1 & APP_PID=$!

# Wait for the window. The webview loads the dev server asynchronously, so
# keep clicking "Scan Projects" (top-right) every few seconds until the
# engine has written results; extra clicks while scanning are ignored.
for _ in $(seq 1 60); do xdotool search --name "Project Hibernate" >/dev/null 2>&1 && break; sleep 1; done
sleep 3
for _ in $(seq 1 30); do
  if [ -f "$PROJECT_HIBERNATE_DATA_DIR/last-scan.json" ] && grep -q '"projectCount": 2' "$PROJECT_HIBERNATE_DATA_DIR/last-scan.json"; then break; fi
  xdotool mousemove 1160 23 click 1
  sleep 3
done
if command -v import >/dev/null; then import -window root "$WORK/after-scan.png" || true; fi

python3 - "$PROJECT_HIBERNATE_DATA_DIR/last-scan.json" <<'PY'
import json, sys
snap = json.load(open(sys.argv[1]))
names = sorted(p["name"] for p in snap["projects"])
assert names == ["svc", "web"], names
kinds = sorted(a["kind"] for p in snap["projects"] for a in p["artifacts"])
assert kinds == ["node_modules", "rust-target"], kinds
assert snap["summary"]["reclaimableBytes"] >= 800000, snap["summary"]
print("desktop smoke ok:", names, kinds, snap["summary"]["reclaimableBytes"], "bytes reclaimable")
PY
