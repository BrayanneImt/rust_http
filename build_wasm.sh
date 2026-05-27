#!/bin/bash
# =============================================================
# build_wasm.sh — Compilation Rust → WebAssembly (WASI)
# Projet : http_client_wasm
# Cible  : wasm32-wasi (compatible WAMR embarqué dans Zephyr)
#
# Prérequis :
#   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
#   rustup target add wasm32-wasi
#
# Usage : bash build_wasm.sh
# =============================================================
set -e

echo "=== Build : Rust → WASM (wasm32-wasi) ==="

# Vérification rustup + cible
if ! command -v rustup &>/dev/null; then
    echo "ERREUR: rustup requis. Installer depuis https://rustup.rs"
    exit 1
fi

if ! rustup target list --installed | grep -q "wasm32-wasip1"; then
    echo "[setup] Ajout cible wasm32-wasip1..."
    rustup target add wasm32-wasip1
fi

# Compilation release (optimisé taille pour IoT)
cargo build --target wasm32-wasip1 --release

OUTPUT=target/wasm32-wasip1/release/http_wasm.wasm
DEST=http_rust.wasm
cp "$OUTPUT" "$DEST"

echo ""
echo "[OK] $DEST — $(ls -lh $DEST | awk '{print $5}')"
file "$DEST"
echo ""
echo "Test PC :"
echo "  wasmtime $DEST"
echo ""
echo "Déploiement IoT (WAMR dans Zephyr) : voir README.md"