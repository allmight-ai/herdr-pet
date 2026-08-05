#!/usr/bin/env bash
# Build step do herdr plugin install/link.
# Compila o release e roda o setup (atalho + PATH) pra ficar usável na hora.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release

# setup: grava keybind no config.toml do user + shim em ~/.local/bin
# (idempotente; seguro rodar várias vezes)
./target/release/herdr-pet setup
