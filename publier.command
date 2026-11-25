#!/bin/bash
# Glaux — publier une nouvelle version (une fois premiere-mise-en-ligne.command passé).
#   cd chemin/vers/Glaux
#   bash publier.command
#
#  1. enregistre les changements du dossier (commit) ;
#  2. passe au numéro de version suivant ;
#  3. envoie tout sur GitHub, qui teste, compile Mac + Windows et prépare un brouillon de version.
# Les utilisateurs ne voient la mise à jour qu'une fois le brouillon publié (« Publish release »).

set -euo pipefail
cd "$(dirname "$0")"

command -v gh >/dev/null || {
  echo "Lance d'abord : bash premiere-mise-en-ligne.command"
  exit 1
}

ACTUELLE="$(node -p 'require("./package.json").version')"
echo "Version actuelle : $ACTUELLE"
echo "  1) correctif   (ex. 0.1.0 → 0.1.1)"
echo "  2) nouveautés  (ex. 0.1.0 → 0.2.0)"
read -r -p "Type de version [1/2] : " TYPE
case "$TYPE" in
  1) NIVEAU="patch" ;;
  2) NIVEAU="minor" ;;
  *)
    echo "Réponse attendue : 1 ou 2. Rien n'est publié."
    exit 1
    ;;
esac

read -r -p "Ce qui change, en une ligne : " MESSAGE
[ -n "$MESSAGE" ] || {
  echo "Message vide : rien n'est publié."
  exit 1
}

git add -A
git diff --cached --quiet || git commit -q -m "$MESSAGE"
# Nouveau numéro dans package.json + commit + étiquette vX.Y.Z (lue par GitHub Actions).
npm version "$NIVEAU" -m "Glaux %s : $MESSAGE"
git push --follow-tags

DEPOT="$(gh repo view --json nameWithOwner --jq .nameWithOwner)"
echo
echo "Glaux $(node -p 'require("./package.json").version') envoyée."
echo "Compilation (environ 15 min) : https://github.com/$DEPOT/actions"
echo "Ensuite : https://github.com/$DEPOT/releases → ouvre le brouillon → « Publish release »."
