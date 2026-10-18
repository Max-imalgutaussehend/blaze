#!/usr/bin/env bash
# Builds ember + forge as WebAssembly for the playground.
set -euo pipefail
cd "$(dirname "$0")/../.."
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cargo build --release -p blaze-wasm --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/blaze_wasm.wasm site/public/blaze.wasm
