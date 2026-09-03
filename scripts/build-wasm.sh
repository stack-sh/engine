#!/usr/bin/env bash

set -euo pipefail

repository_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
distribution="$repository_root/packages/engine/dist"

if [[ "$distribution" != "$repository_root/packages/engine/dist" ]]; then
  echo "refusing to clean an unexpected distribution path" >&2
  exit 1
fi

wasm_bindgen_version=$(wasm-bindgen --version)
if [[ "$wasm_bindgen_version" != "wasm-bindgen 0.2.127" ]]; then
  echo "wasm-bindgen 0.2.127 is required" >&2
  exit 1
fi

rm -rf "$distribution"
mkdir -p "$distribution"

cd "$repository_root"
cargo +stable build -p stack-engine-wasm --release --target wasm32-unknown-unknown --locked
wasm-bindgen \
  target/wasm32-unknown-unknown/release/stack_engine_wasm.wasm \
  --out-dir "$distribution" \
  --out-name stack_engine \
  --target web \
  --typescript \
  --no-demangle
