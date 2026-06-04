#!/bin/bash
# =============================================================
# build_wasm.sh — Compilation Rust → WebAssembly
# Cible : wasm32-unknown-unknown (no_std)
# wasm-opt 116 : ne supporte pas --no-dce ni --export
# → on utilise uniquement cargo build --release (opt-level="s" + lto=true)
# =============================================================
set -e

echo "=== Build : rust_http -> WASM (wasm32-unknown-unknown) ==="

if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
    echo "[setup] Ajout cible wasm32-unknown-unknown..."
    rustup target add wasm32-unknown-unknown
fi

echo "[build] cargo build --release..."
cargo build --target wasm32-unknown-unknown --release

OUTPUT=target/wasm32-unknown-unknown/release/http_wasm.wasm
DEST=http_rust.wasm

cp "$OUTPUT" "$DEST"
SIZE=$(stat -c%s "$DEST")
echo "[OK] $DEST — ${SIZE} octets ($(( SIZE / 1024 )) KB)"

MAX=$(( 32 * 1024 ))
if [ "$SIZE" -gt "$MAX" ]; then
    echo "ATTENTION : module > ${MAX} octets (WASM_MAX_SIZE = 32KB)"
    echo "Augmenter WASM_MAX_SIZE dans zephyr_wamr_runtime/src/main.c"
fi

echo ""
echo "Déploiement : python3 upload.py"