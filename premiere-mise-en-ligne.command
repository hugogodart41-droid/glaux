#!/bin/bash
# Glaux — mise en ligne sur GitHub. À lancer UNE SEULE FOIS, dans le Terminal :
#   cd chemin/vers/Glaux
#   bash premiere-mise-en-ligne.command
#
# Dans l'ordre :
#  1. vérifie git et GitHub CLI (gh), puis te connecte à GitHub (dans le navigateur) ;
#  2. crée la clé qui signe les mises à jour (~/.tauri/glaux.key, protégée par un mot de passe)
#     et écrit sa partie publique dans src-tauri/tauri.conf.json ;
#  3. crée le dépôt public « glaux » sur ton compte et y envoie le code ;
#  4. range la clé privée et son mot de passe dans les secrets du dépôt (jamais dans le code) ;
#  5. propose de publier la première version.
# Peut être relancé sans risque : ce qui est déjà fait est sauté.

set -euo pipefail
cd "$(dirname "$0")"

REPO_NAME="glaux"
KEY="$HOME/.tauri/glaux.key"
CONF="src-tauri/tauri.conf.json"

etape() { printf '\n\033[1m%s\033[0m\n' "$1"; }
stop() {
  printf '\n%s\n' "$1"
  exit 1
}

# ---------- 1. Outils et connexion ----------
etape "1/5 Outils et connexion à GitHub"
[ -f .github/workflows/release.yml ] || stop "Place d'abord les automatisations GitHub (une commande) :
  mkdir -p .github/workflows && mv workflows-github/*.yml .github/workflows/ && rmdir workflows-github"
command -v git >/dev/null || stop "git est introuvable : lance « xcode-select --install », puis relance ce script."
command -v node >/dev/null || stop "Node est introuvable (il sert déjà à compiler Glaux) : vérifie ton installation."
if ! command -v gh >/dev/null; then
  if command -v brew >/dev/null; then
    brew install gh
  else
    stop "GitHub CLI (gh) est introuvable. Installe-le depuis https://cli.github.com (fichier .pkg pour macOS), puis relance ce script."
  fi
fi
gh auth status >/dev/null 2>&1 || gh auth login --hostname github.com --web --git-protocol https
gh auth setup-git
LOGIN="$(gh api user --jq .login)"
echo "Connecté à GitHub : $LOGIN"

# ---------- 2. Clé de signature des mises à jour ----------
etape "2/5 Clé de signature des mises à jour"
if [ ! -f "$KEY" ]; then
  echo "Choisis un mot de passe pour cette clé et garde-le précieusement (gestionnaire de mots de passe)."
  echo "Sans la clé ET son mot de passe, plus aucune mise à jour ne pourra être publiée."
  while true; do
    read -rs -p "Mot de passe : " PW
    echo
    read -rs -p "Encore une fois : " PW2
    echo
    if [ -n "$PW" ] && [ "$PW" = "$PW2" ]; then break; fi
    echo "Les deux saisies diffèrent (ou sont vides) : recommence."
  done
  mkdir -p "$(dirname "$KEY")"
  npx tauri signer generate --ci -w "$KEY" -p "$PW" >/dev/null
  echo "Clé créée : $KEY (à sauvegarder, par exemple sur une clé USB)."
else
  echo "Clé déjà présente : $KEY"
  read -rs -p "Mot de passe de la clé : " PW
  echo
fi

# Clé publique + adresse des mises à jour (avec ton pseudo GitHub) dans tauri.conf.json.
node -e '
const fs = require("fs");
const [conf, pub, login, repo] = process.argv.slice(1);
const c = JSON.parse(fs.readFileSync(conf, "utf8"));
c.plugins.updater.pubkey = fs.readFileSync(pub, "utf8").trim();
c.plugins.updater.endpoints = [`https://github.com/${login}/${repo}/releases/latest/download/latest.json`];
fs.writeFileSync(conf, JSON.stringify(c, null, 2) + "\n");
' "$CONF" "$KEY.pub" "$LOGIN" "$REPO_NAME"
echo "Clé publique et adresse des mises à jour enregistrées dans $CONF."

# ---------- 3. Dépôt GitHub ----------
etape "3/5 Dépôt public « $REPO_NAME »"
[ -d .git ] || git init -b main
# Identité des commits : ton pseudo et l'adresse « noreply » de GitHub (ton e-mail reste privé).
git config user.name >/dev/null || git config user.name "$LOGIN"
git config user.email "$(gh api user --jq '"\(.id)+\(.login)@users.noreply.github.com"')"
VERSION="$(node -p 'require("./package.json").version')"
git add -A
git diff --cached --quiet || git commit -q -m "Glaux $VERSION"
if ! git remote get-url origin >/dev/null 2>&1; then
  if gh repo view "$LOGIN/$REPO_NAME" >/dev/null 2>&1; then
    git remote add origin "https://github.com/$LOGIN/$REPO_NAME.git"
  else
    gh repo create "$REPO_NAME" --public --source=. --remote=origin \
      --description "Euro area macro dashboard for Mac & Windows: ECB data, no account, no tracking."
  fi
fi
git push -u origin main

# ---------- 4. Secrets ----------
etape "4/5 Secrets du dépôt (clé privée et mot de passe)"
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo "$LOGIN/$REPO_NAME" <"$KEY"
printf '%s' "$PW" | gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo "$LOGIN/$REPO_NAME"
unset PW PW2

# ---------- 5. Première version ----------
etape "5/5 Première version"
read -r -p "Publier la v$VERSION maintenant (compilation Mac + Windows sur GitHub, environ 15 min) ? [o/N] " REPONSE
if [[ "$REPONSE" =~ ^[oOyY] ]]; then
  git tag -a "v$VERSION" -m "Glaux $VERSION" 2>/dev/null || true
  git push origin "v$VERSION"
  echo "Compilation en cours : https://github.com/$LOGIN/$REPO_NAME/actions"
  echo "Ensuite : https://github.com/$LOGIN/$REPO_NAME/releases → ouvre le brouillon → « Publish release »."
else
  echo "Plus tard : bash publier.command"
fi
echo
echo "Terminé. Dépôt : https://github.com/$LOGIN/$REPO_NAME"
