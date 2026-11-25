#!/bin/bash
# Double-clic : lance Glaux en mode développement (rechargement auto de l'interface).
# Première fois : rends ce fichier exécutable avec  chmod +x dev.command

cd "$(dirname "$0")" || exit 1

# Rend Rust disponible même si le Terminal ne l'a pas encore chargé.
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

# Installe les dépendances JavaScript au premier lancement.
if [ ! -d node_modules ]; then
  npm install || exit 1
fi

npm run tauri dev
