set -e

echo "=== Build : Rust -> WASM (wasm32-unknown-unknown) ==="

if ! command -v rustup &>/dev/null; then
    echo "ERREUR: rustup requis."
    exit 1
fi

# Supprimer rust-toolchain.toml s'il existe
if [ -f rust-toolchain.toml ]; then
    rm rust-toolchain.toml
    echo "[info] rust-toolchain.toml supprimé"
fi

# Ajouter la cible wasm32-unknown-unknown si absente
if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
    echo "[setup] Ajout cible wasm32-unknown-unknown..."
    rustup target add wasm32-unknown-unknown
fi

echo "[build] cargo build --release --target wasm32-unknown-unknown..."
cargo build --target wasm32-unknown-unknown --release

OUTPUT=target/wasm32-unknown-unknown/release/http_wasm.wasm
DEST=http_rust.wasm

SIZE_BEFORE=$(stat -c%s "$OUTPUT")
echo "[build] Module brut : ${SIZE_BEFORE} octets ($(( SIZE_BEFORE / 1024 )) KB)"

echo "[opt] wasm-opt -Oz..."
wasm-opt -Oz --enable-bulk-memory --memory-packing "$OUTPUT" -o "$DEST"

SIZE_AFTER=$(stat -c%s "$DEST")
GAIN=$(( (SIZE_BEFORE - SIZE_AFTER) * 100 / SIZE_BEFORE ))
echo "[opt] ${SIZE_BEFORE} → ${SIZE_AFTER} octets (-${GAIN}%)"
echo ""
echo "[OK] $DEST — ${SIZE_AFTER} octets ($(( SIZE_AFTER / 1024 )) KB)"

MAX=$(( 96 * 1024 ))
if [ "$SIZE_AFTER" -gt "$MAX" ]; then
    echo "ERREUR : module trop grand (${SIZE_AFTER} > ${MAX})"
    exit 1
fi
echo "Taille OK pour l'équipement IoT"
echo ""
echo "Déploiement : python3 upload.py"