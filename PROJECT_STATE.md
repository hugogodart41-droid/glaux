# Glaux — état du projet (27/11/2026)

## 1. Objectif
Tableau de bord macro de la zone euro dans la barre des menus (Mac) / zone de notification (Windows), FR/EN, données officielles de la BCE, sans compte ni pistage. Construit en public pour LinkedIn/TikTok par Hugo (Howzen, M1 Finance) : chaque chiffre doit être exact et bien nommé.

## 2. Stack
- Tauri 2.12 : cœur Rust (édition 2021) + interface TypeScript/Vite sans framework (DOM natif, canvas maison).
- Rust : `tauri` (tray-icon, image-png), `serde`/`serde_json`, `ureq` 3 (HTTP synchrone), `csv`, `chrono`, `sys-locale`, `rust_xlsxwriter`, `tauri-plugin-clipboard-manager`, `tauri-plugin-updater` ; Mac : `objc2`, `objc2-foundation`, `objc2-user-notifications`, `block2` ; Windows/Linux : `tauri-plugin-notification`.
- JS : `@tauri-apps/api` ; dev : `@tauri-apps/cli`, `typescript`, `vite`.
- Données : API BCE `data-api.ecb.europa.eu` (CSV) : 80 séries en 15 requêtes (quotidiennes, mensuelles, trimestrielles). macOS 14+, Windows 10/11. Publication : GitHub Actions (`tauri-action@v1`).

## 3. Fichiers
| Fichier | Rôle |
|---|---|
| `index.html` | Structure du panneau (en-tête déplaçable, agenda, liste, vue détail, écran « Indicateurs affichés ») |
| `src/main.ts` | Écrans, rendu de la liste par rubriques, vue détail, pays, groupes, mode agrandi, exports, carrousel |
| `src/indicators.ts` | Formats et textes FR/EN de chaque série, groupes (pays), pays, courbe |
| `src/format.ts` | Mise en forme des nombres, dates, variations |
| `src/chart.ts` | Courbes (mini et détail) sans bibliothèque |
| `src/curve.ts` | Courbe des taux AAA (chargement, dessin) |
| `src/export.ts` | Images PNG (portrait/paysage/story), pages JPEG du carrousel |
| `src/agenda.ts` | Bandeau « prochaine décision BCE / inflation flash » |
| `src/note.ts` | Écran « Note macro » : analyse (enregistrée à la frappe), copie du post, PDF |
| `src/i18n.ts` · `src/tooltip.ts` · `src/types.ts` · `src/styles.css` | Textes d'interface · infobulles · types partagés avec Rust · styles |
| `src-tauri/src/main.rs` | Fenêtre/panneau, icône, commandes Tauri, fil d'arrière-plan (5 min), notifications |
| `src-tauri/src/ecb.rs` | Définition des séries, requêtes BCE groupées, lecture CSV |
| `src-tauri/src/store.rs` | Cache, préférences, rubriques, choix des pays, calculs dérivés, vue envoyée à l'interface |
| `src-tauri/src/data.rs` | Observations, fréquences, résumés, mini-courbes |
| `src-tauri/src/agenda.rs` | Calendrier BCE/inflation (intégré + `agenda.json` publié), rappels du jour J |
| `src-tauri/src/alerts.rs` | Alertes (taux de dépôt changé, nouvelle inflation) |
| `src-tauri/src/note.rs` | Note macro mensuelle : faits du mois en phrases FR/EN, texte du post LinkedIn |
| `src-tauri/src/macos_notify.rs` | Notifications natives Mac + délégué (clic → indicateur) |
| `src-tauri/src/updates.rs` | Mises à jour automatiques (6 h, réveil) |
| `src-tauri/src/excel.rs` · `pdf.rs` · `i18n.rs` | Export Excel · PDF du carrousel · textes Rust |
| `src-tauri/tauri.conf.json` · `tauri.release.conf.json` · `capabilities/default.json` · `Info.plist` | Config app (updater, signature ad hoc) · config de publication · permissions · app sans Dock |
| `agenda.json` | Calendrier publié, lu par les apps installées (à compléter chaque année) |
| `.github/workflows/release.yml` · `tests.yml` | Publication (Mac universel + Windows, brouillon + latest.json) · tests |
| `dev.command` · `publier.command` · `premiere-mise-en-ligne.command` | Lancer en dev · publier une version (`npm version`) · configuration initiale (déjà faite) |
| `README.md` · `LISEZMOI.md` | Page publique (EN + résumé FR) · documentation développeur détaillée (FR) |

## 4. Décisions de conception
- Rust calcule tout (cache, dérivés, rubriques) ; l'interface affiche seulement. Pas de framework JS, pas de bibliothèque de graphiques (légèreté).
- Courbe des taux en AAA (pas « toutes notations ») : la pente 2-10 ans « toutes notations » incluait une prime de risque (73 pb au lieu de 30).
- Requêtes BCE groupées (« A+B » sur une seule dimension, tri par colonne `KEY`) : ~2,5 s par requête quelle que soit la taille → 12 requêtes au lieu de 34.
- Indicateurs par pays (inflation, sous-jacente, services, chômage) : un pays à la fois, choisi dans la vue détail, zone euro en pointillés. Taux 10 ans et écarts : 3 pays au choix sur une ligne (mensuel, pas de quotidien par pays à la BCE).
- 3 rubriques : Zone euro / Zone euro ou par pays / Calculés par Glaux (données officielles séparées des calculs).
- PIB : 2 lignes (sur 1 trimestre affichée, sur 1 an masquée), jeu MNA, CVS-CJO. Zone euro = `I10` (21 pays depuis le 1er janvier 2026 ; `I9` arrêtée au T1 2026), unité EUR ; pays : unité XDC. Clés incompatibles → 3 requêtes. Fréquence `Quarterly` (dates « 2026-Q3 », affichage « T3 2026 », variation « sur 1 trimestre », Excel « Trimestriel » avec dates en texte).
- Notifications Mac natives (le plugin Tauri utilise une API dépréciée) ; clic → indicateur via l'identifiant « glaux-hicp-3 ».
- Calendrier publié dans `agenda.json` (pas de scraping fragile) ; repli sur les dates intégrées.
- Mac : un seul DMG universel ; signature ad hoc. Mises à jour signées (clé privée hors dépôt).
- Carrousel : pages JPEG dessinées en canvas, PDF assemblé en Rust sans dépendance.
- Note macro : Rust écrit des phrases factuelles à partir des données (aucune interprétation générée, chaque chiffre daté) ; l'analyse est celle de Hugo (`notes.json`, un texte par mois). Même chaîne PDF que le carrousel (`save_pdf` dans main.rs).

## 5. Ce qui fonctionne (v0.5.0 publiée le 27/11/2026 : socle v0.4.0 + note macro + PIB, voir 5 bis et 5 ter)
16 lignes (dont 5 masquées par défaut) en 3 rubriques ; vue détail 1 an / 5 ans ; pays ; courbe AAA ; panneau agrandi déplaçable ; agenda + rappels ; alertes ; FR/EN ; exports image (3 formats), carrousel LinkedIn (PDF), Excel ; mises à jour automatiques (validées 0.1→0.2→0.3) ; 64 tests Rust.

## 5 bis. Note macro mensuelle (27/11/2026, validée par Hugo)
Icône « document » / ⌘N : faits du mois en 4 rubriques (Prix ; Taux et politique monétaire ; Emploi, crédit et monnaie ; Marchés), zone « Mon analyse », « Copier le texte » (post LinkedIn, compteur 3 000), « Enregistrer le PDF » (couverture, analyse, 4 pages de rubrique, sources ; Images/Glaux/Glaux_note_macro_{date}.pdf). Vérifié côté Claude : 73 tests Rust (9 nouveaux), clippy, tsc, vite build, aperçus Playwright FR/EN (petit et grand panneau, PDF 7 pages, analyse de 5 000 caractères → pages « suite »).

## 5 ter. PIB par pays (27/11/2026, publié dans la v0.5.0)
Lignes « PIB (sur 1 trimestre) » et « PIB (sur 1 an) », zone euro ou 8 pays ; note macro : rubrique « Croissance, emploi et crédit » (PIB en vedette, écart entre pays) ; Excel : feuille « Trimestriel » (18 colonnes). Clés et valeurs vérifiées sur l'API BCE le 27/11/2026 (zone euro T3 2026 : +0,4 % sur 1 trimestre, +1,3 % sur 1 an ; France −0,1 %). Vérifié côté Claude : 76 tests Rust, clippy, tsc, build, aperçus Playwright avec les vraies données (liste, détail zone euro et France, image exportée).

## 6. En cours / prochaine étape
- v0.5.0 publiée le 27/11/2026 (note macro + PIB) : téléchargement réel des 80 séries OK, mise à jour 0.4.0 → 0.5.0 validée sur le Mac de Hugo.
- Reste à vérifier : clic sur la notification « Alertes Glaux activées » (app compilée).
- Pistes suivantes : données US, test Windows, signature Developer ID ; première note macro publiée mi-décembre.
- Publication : `publier.command` (2 = nouveautés), attendre la fin des 2 tâches GitHub Actions (~10 min ; la page Actions peut rester figée, vérifier par l'API), puis Releases › brouillon › notes « Nouveautés / What's new » au-dessus de « Which file? » › « Publish release ».

## 7. Bugs connus et pistes déjà testées
- `monitor_from_point` de Tauri mélange pixels et points sur Retina → remplacé par `monitor_at` + `work_area`. `window.screen.availWidth` peu fiable → abandonné.
- `Instant` ne compte pas la veille sur Mac → `SystemTime` pour les mises à jour.
- `canvas.roundRect` absent des anciens WebKit → tracé maison.
- API BCE, série trimestrielle : `startPeriod=2026-08-27` exclut le 3e trimestre (commencé le 1er juillet) et renvoie une réponse vide (HTTP 200, 0 caractère) → début ramené au 1er jour du trimestre (`request_start` dans ecb.rs) ; réponse vide = aucune donnée.
- `latest.json` d'un brouillon pointe vers l'API GitHub : fonctionne (en-tête `Accept: application/octet-stream`).
- Machines Mac GitHub parfois indisponibles → `gh run rerun <id> --failed`.
- Notifications en dev (`tauri dev`, hors .app) : repli `osascript`, pas de clic ; seul Glaux.app compilé teste le délégué.
- Côté Claude : impossible de compiler le code Mac (cible Apple non installable) → `macsim.sh` (stub) + projet d'essai objc2 ; le dossier `.github/` n'est pas modifiable à distance (passer par `workflows-github/` puis `mv`).

## 8. Conventions
- Commentaires et documentation en français ; identifiants en anglais ; noms de tests en français.
- Avant tout dépôt : `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `npx tsc --noEmit`, aperçus Playwright FR/EN.
- Toute nouvelle série : définition dans `ecb.rs`, format + textes FR/EN dans `indicators.ts`, valeurs vérifiées sur le portail BCE (intitulé officiel).
- Jamais : écraser `Cargo.lock`, déposer `package.json` depuis une vieille copie, mettre une clé ou un secret dans le dépôt, mentionner dans les fichiers publics des informations de compte Apple.
- Vie privée : connexions limitées à la BCE et à GitHub ; données locales.
- Hugo compile, teste, commite et publie lui-même : toujours lui donner les commandes Terminal complètes.
