#!/usr/bin/env bash
# Build the sandbox for the web (wasm32 + wasm-bindgen) and optionally serve it.
#   scripts/web.sh            build (release, size-optimised)
#   scripts/web.sh serve      build, then serve on http://localhost:8080 (PORT to override)
#   scripts/web.sh dev        debug build (faster to compile), then serve
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"

profile="web"
[[ "${1:-}" == "dev" ]] && profile="dev"
target_dir="target/wasm32-unknown-unknown/$([[ $profile == dev ]] && echo debug || echo web)"

cargo build -p knight-sandbox --bin sandbox --target wasm32-unknown-unknown --profile "$profile"
wasm-bindgen --target web --no-typescript --out-dir apps/sandbox/web/pkg "$target_dir/sandbox.wasm"
echo "built apps/sandbox/web ($(du -h apps/sandbox/web/pkg/sandbox_bg.wasm | cut -f1) wasm)"

if [[ "${1:-}" == "serve" || "${1:-}" == "dev" ]]; then
  port="${PORT:-8080}"
  echo "serving http://localhost:$port"
  exec python3 -m http.server "$port" --directory apps/sandbox/web
fi
