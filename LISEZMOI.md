# Glaux

Tableau de bord macro de la zone euro, dans la barre des menus (Mac) et la zone de notification (Windows).
Tauri 2 : cœur en Rust (`src-tauri/`), interface en TypeScript (`src/`).

## Lancer en développement

```bash
cd chemin/vers/Glaux
npm install
npm run tauri dev
```

Ou double-clic sur `dev.command` (après un `chmod +x dev.command` la première fois).
Le premier lancement compile Rust : compte 3 à 5 minutes. Les suivants prennent quelques secondes.

## Tests

```bash
cd src-tauri
cargo test                          # calculs et lecture des CSV (hors ligne)
cargo test -- --ignored --nocapture # téléchargement réel des 10 séries BCE (Internet)
```

## Données

Source : API publique de la BCE (sans clé, sans compte). Les séries sont définies dans `src-tauri/src/ecb.rs` :
les 6 indicateurs affichés, plus les taux à 10 ans de l'Allemagne, de la France, de l'Italie et de l'Espagne
(jeu `IRS`, statistiques de convergence : moyennes mensuelles, publiées environ 3 semaines après la fin du mois).

Calculés par Glaux (`src-tauri/src/store.rs`, à la demande, rien de plus en cache) :
- taux réel = taux de dépôt − inflation ; pente = taux souverain 10 ans − €STR ;
- écarts de taux avec l'Allemagne (France, Italie, Espagne), en points de base, affichés sur une seule ligne
  (« groupe » `spreads` : `src/indicators.ts`) avec trois courbes superposées en vue détail et à l'export.
Quand une nouvelle version suit une série de plus, elle est téléchargée au lancement sans attendre les 6 h.
Cache : `~/Library/Application Support/com.howzen.glaux/cache.json` (Mac), `%APPDATA%\com.howzen.glaux\cache.json` (Windows).
Actualisation automatique si les données ont plus de 6 h (vérification toutes les 15 min), ou via ↻ / ⌘R.
Préférences (alertes activées, rappels déjà envoyés, langue) : `prefs.json`, dans le même dossier que le cache.

## Image exportée

Vue détail : choix du format (retenu d'une fois sur l'autre), « Copier » (⌘C) et « Enregistrer » (⌘E).
- Portrait 1080 × 1350 (LinkedIn, Instagram), paysage 1200 × 675 (LinkedIn, X), story 1080 × 1920 (TikTok, Reels).
  La story garde l'essentiel hors des zones couvertes par l'interface des applications (haut et bas de l'écran).
- Chaque format est une table de positions dans `src/export.ts` (`LAYOUTS`) ; le dessin est commun.
- Enregistrement : `Images/Glaux/Glaux_{indicateur}_{format}_{date}.png` (nom construit côté Rust à partir de
  valeurs connues). Copie : module presse-papiers de Tauri (`copy_png` dans `main.rs`).

## Export Excel

Icône « tableau » dans l'en-tête (à côté de ↻) : toutes les séries dans `Documents/Glaux/Glaux_donnees_{date}.xlsx`,
construit côté Rust à partir du cache (`src-tauri/src/excel.rs`, crate `rust_xlsxwriter`), sans réseau.
- « Quotidien » : taux de dépôt (valeur en vigueur reportée sur chaque date), €STR, taux 10 ans, EUR/USD, pente (pb).
- « Mensuel » : inflation, chômage, taux réel, taux 10 ans DE/FR/IT/ES, écarts avec l'Allemagne (pb).
- « Sources » : clé BCE ou calcul de chaque colonne, unité, date du dernier téléchargement.
Vraies dates Excel, formats de nombre par unité, en-tête figé, filtres ; titres dans la langue de Glaux.

## Langue

Français ou anglais. Par défaut : la langue du système (français si le Mac/PC est en français, anglais sinon).
Bouton « English / Français » en bas du panneau ; le choix est enregistré.
- Interface, formats (`2,50 %` / `2.50%`, `pb` / `bp`), image exportée : `src/i18n.ts` et `src/indicators.ts`.
  Les deux dictionnaires ont le même type : un texte oublié d'un côté empêche la compilation.
- Notifications et menu de l'icône : `src-tauri/src/i18n.rs`, `alerts.rs`, `agenda.rs`.
- Les détails techniques d'erreur (au survol du statut, pour le débogage) restent en français.

## Notifications

Changement du taux de dépôt, nouvelle inflation publiée, rappel le matin d'une décision BCE ou d'une inflation flash.
Bouton « Alertes : oui/non » en bas du panneau (« Alertes : bloquées » si macOS les refuse, raison au survol).

- **Mac** : système natif d'Apple (`src-tauri/src/macos_notify.rs`, UNUserNotificationCenter). Le module de Tauri
  passe par une API dépréciée depuis macOS 11 qui ne délivre plus rien sur les macOS récents.
  Fonctionne dans l'app compilée (Glaux.app, signée « ad hoc ») ; en mode développement, repli sur `osascript`
  (notification au nom de « Éditeur de script »).
- **Windows / Linux** : module de notifications de Tauri.

## Mises à jour automatiques

Module « updater » de Tauri (`src-tauri/src/updates.rs`). Glaux vérifie au lancement puis une fois par jour
le fichier `latest.json` de la dernière version publiée sur GitHub. Si une version plus récente existe,
« Installer la vX.Y.Z » remplace le statut en bas du panneau ; un clic télécharge, vérifie la signature,
installe et relance. Rien ne s'installe sans clic. Pas de vérification en mode développement.

- Adresse et clé publique : `src-tauri/tauri.conf.json` › `plugins.updater` (écrites par `premiere-mise-en-ligne.command`).
- Clé privée : `~/.tauri/glaux.key` sur le Mac (+ mot de passe), copiée dans les secrets du dépôt GitHub.
  **À sauvegarder** : sans elle, plus aucune mise à jour ne peut être publiée.
- `requireSignedVersion` activé : une version signée ne peut pas être resservie sous un autre numéro.

## Publier une version

Sur GitHub (dépôt public, compilation gratuite) : `.github/workflows/release.yml` compile Mac (puce Apple + Intel)
et Windows à chaque étiquette `vX.Y.Z`, et prépare un **brouillon** de version avec les installeurs et `latest.json`.
Les utilisateurs ne voient rien tant que le brouillon n'est pas publié.

```bash
cd chemin/vers/Glaux
bash premiere-mise-en-ligne.command   # une seule fois : GitHub, clé de signature, dépôt, secrets
bash publier.command                  # ensuite, à chaque version
```

Puis : onglet Releases du dépôt › brouillon › vérifier › « Publish release ».
`.github/workflows/tests.yml` lance les tests (Mac + Windows) à chaque envoi.

Signature Mac : « ad hoc » (`signingIdentity: "-"` dans `tauri.conf.json`) ; premier lancement expliqué dans
`README.md`. Signature Developer ID et notarisation : secrets `APPLE_*` à ajouter dans `release.yml`
(voir la documentation de tauri-action).
Version de l'app : `package.json` (une seule source ; `publier.command` l'augmente avec `npm version`).

## Structure

| Fichier | Rôle |
|---|---|
| `src-tauri/src/main.rs` | Icône de la barre des menus, panneau, commandes appelées par l'interface, fil d'actualisation |
| `src-tauri/src/ecb.rs` | Les 6 séries BCE, téléchargement et lecture du CSV |
| `src-tauri/src/data.rs` | Calculs : dernière valeur, variation sur 1 mois, mini-courbe, historique, taux réel, pente |
| `src-tauri/src/agenda.rs` | Dates officielles des décisions BCE et des inflations flash (**à mettre à jour chaque année**, liens dans le fichier) et rappels du jour J |
| `src-tauri/src/macos_notify.rs` | Notifications Mac (UNUserNotificationCenter, repli osascript en mode dev) |
| `src-tauri/src/alerts.rs` | Textes des notifications : changement de taux BCE, nouvelle inflation |
| `src-tauri/src/i18n.rs` | Langue (système ou choix), noms des mois et nombres en FR / EN |
| `src-tauri/src/excel.rs` | Export Excel des historiques (feuilles quotidienne, mensuelle, sources) |
| `src-tauri/src/updates.rs` | Mises à jour automatiques : vérification quotidienne, installation au clic |
| `src-tauri/src/store.rs` | Cache local (lecture, écriture, actualisation) et indicateurs calculés à la demande |
| `src-tauri/tauri.conf.json` | Fenêtre, identifiant de l'app, icônes, sécurité, mises à jour |
| `src-tauri/tauri.release.conf.json` | En plus pour les versions publiées : DMG, installeur Windows, fichiers de mise à jour |
| `.github/workflows/` | GitHub Actions : tests à chaque envoi, compilation et brouillon de version à chaque étiquette |
| `premiere-mise-en-ligne.command`, `publier.command` | Mise en ligne (une fois) et publication d'une version |
| `README.md` | Page publique du dépôt (anglais + résumé en français) : installation, confidentialité |
| `src-tauri/capabilities/default.json` | Ce que l'interface a le droit de demander au cœur Rust |
| `src-tauri/Info.plist` | Mac : pas d'icône dans le Dock |
| `src-tauri/icons/` | Icônes générées depuis `app-icon.png` (`npm run tauri icon app-icon.png`) |
| `index.html`, `src/styles.css` | Structure et style du panneau (liste + vue détail) |
| `src/main.ts` | Interface : liste, vue détail, boutons, raccourcis |
| `src/chart.ts` | Courbes sans bibliothèque : mini-courbes, courbe de la vue détail, courbe de l'image |
| `src/export.ts` | Image exportée : portrait 1080 × 1350, paysage 1200 × 675, story 1080 × 1920 (une table de positions par format), enregistrée dans Images › Glaux ou copiée |
| `src/agenda.ts` | Bandeau d'agenda : compte à rebours (les dates viennent de Rust) |
| `src/indicators.ts` | Nom, précision et textes d'explication de chaque indicateur (FR / EN) |
| `src/i18n.ts` | Tous les textes de l'interface, en français et en anglais |
| `src/format.ts`, `src/tooltip.ts`, `src/types.ts` | Mise en forme des nombres et dates, infobulle maison, types des données |

## Feuille de route

1. Socle : icône + panneau
2. Données BCE + cache local
3. Mini-courbes sur 1 an
4. Vue détail (courbe 5 ans) + export PNG
5. Indicateurs calculés : taux réel, pente des taux
6. Agenda : prochaine décision BCE, prochaine inflation
7. Notifications
8. FR/EN, mises à jour automatiques, Windows, version publiée ← **ici**
