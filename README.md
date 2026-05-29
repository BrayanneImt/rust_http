# Projet Rust de l'app zephyr pour la creation de socket et envoie de metrique

## Préréquis

```bash
# Installer Rust (si absent)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Ajouter la cible WebAssembly WASI
rustup target add wasm32-wasip1

# Vérifier
rustup target list --installed | grep wasm
# → wasm32-wasip1
```

## Structure du projet

```text
rust_http/
├── src/
│   └── main.rs       ← Application principale (HTTP TCP + LED blinky)
├── Cargo.toml        ← Manifeste du projet
├── build_wasm.sh     ← Script de compilation Rust → WASM
└── upload.py         ← Script d'envoi UART vers l'équipement IoT
```

## Configuration du projet

```bash
#cloner le repo
git clone https://github.com/BrayanneImt/rust_http.git
```

## Paramètres à adapter avant compilation

Dans `src/main.rs`, modifier si nécessaire :

```c
const WIFI_SSID: &str = "wifi_name";   // Nom du hotspot
const WIFI_PSK:  &str = "password";            // Mot de passe
const SERVER_IP: &str = "IP_server";        // IP du PC sur le hotspot
const SERVER_PORT: u16 = 8080;
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

## Déploiement sur l'équipement IoT

```bash
python3 -m venv .venv

# activer l'environnment virtuel
source .venv/bin/activate
# sous windows
source mon_env/bin/activate

# Installer les dependences necessaire
pip install pyserial

# Execution
python3 upload.py
# → Progression de l'upload affichée
# → UPLOAD DONE
```
