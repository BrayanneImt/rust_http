# Projet Rust de l'app zephyr pour la creation de socket et envoie de metrique

## Préréquis

```bash
#Installer Rust (si absent)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

## Structure du projet

```text
rust_http/
├── src/
│   └── main.rs      ← Application principale
├── Cargo.toml       ← Manifeste du projet
└── build_wasm.sh    ← Script de compilation
```

## Configuration du projet

```bash
#cloner le repo
git clone lien_du_repo

# Ajouter la cible WebAssembly WASI
rustup target add wasm32-wasip1

# Vérifier
rustup target list --installed | grep wasm
# → wasm32-wasip1 resultat
```

## execution du projet pour la generation du fichier wasm

```bash
cd rust_http
bash build_wasm.sh

# Ou manuellement :
cargo build --target wasm32-wasip1 --release

# Copier le résultat
cp target/wasm32-wasip1/release/http_wasm.wasm http_rust.wasm
```

## test sur PC des executable wasm avant deploiement

```bash
# Installer wasmtime
curl https://wasmtime.dev/install.sh -sSf | bash
source ~/.bashrc

# Tester le WASM Rust
wasmtime http_rust.wasm
```
