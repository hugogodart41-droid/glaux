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
les 11 indicateurs BCE du panneau, plus les taux à 10 ans de 8 pays (Allemagne, France, Italie, Espagne, Grèce,
Portugal, Belgique, Pays-Bas ; jeu `IRS`, statistiques de convergence : moyennes mensuelles, publiées environ
3 semaines après la fin du mois). La BCE ne publie pas de taux quotidien par pays.
- Zone euro ou par pays (V3) : inflation (totale, sous-jacente, services) et chômage existent aussi pour les
  8 pays (`BY_COUNTRY` dans `ecb.rs` : mêmes clés BCE, `U2` remplacé par le code pays, ex.
  `HICP.M.FR.N.000000.4D0.ANR`), téléchargés avec la zone euro (voir « Téléchargement »).
  Dans la vue détail, des pastilles « ZE DE FR IT… » choisissent la zone euro ou un pays : la ligne du panneau
  affiche alors ce pays (« France · oct. 2026 ») et la courbe de la zone euro reste en pointillés pour
  comparer (vue détail et image exportée). Choix enregistré dans prefs.json (`countries`, commande
  `set_country`) ; les alertes et le taux directeur réel restent calculés sur la zone euro.
- Taux à 10 ans par pays (V3) : une ligne (« groupe » `yields`) pour 3 pays au plus, choisis par l'utilisateur
  (Allemagne, France, Italie par défaut), en %, variation en points ; juste après « Taux souverain 10 ans »
  (moyenne quotidienne de la zone euro), courbes superposées en vue détail et à l'export.
- Depuis la V3 : inflation sous-jacente et des services (jeu `HICP`), M3, crédit aux entreprises et aux ménages
  (jeu `BSI`, croissance sur 1 an, publiée vers la fin du mois suivant). Masqués par défaut (`DEFAULT_HIDDEN`).

Calculés par Glaux (`src-tauri/src/store.rs`, à la demande, rien de plus en cache) :
- taux directeur réel = taux de dépôt − inflation sur 1 an (taux réel ex post) ;
- pente 2-10 ans = taux AAA à 10 ans − taux AAA à 2 ans (courbe de la BCE), en points de base ;
- écarts de taux avec l'Allemagne (7 pays), en points de base, affichés sur une seule ligne (« groupe » `spreads` :
  `src/indicators.ts`) pour 3 pays au plus, choisis par l'utilisateur (France, Italie, Espagne par défaut),
  avec leurs courbes superposées en vue détail et à l'export. Couleur selon la place : or, crème, bleu gris.
- Courbe des taux AAA (V3) : jeu `YC`, échantillon `G_N_A` (États notés AAA, les plus proches du sans-risque),
  taux zéro coupon quotidiens de 3 mois à 30 ans (11 séries `yc_*`), groupe « curve » : une ligne (forme de la
  courbe du jour, extrémités 3 mois → 30 ans) ; vue détail et image : courbe du jour (or) et celle d'il y a 1 ou
  5 ans (pointillés), avec la pente 2-10 ans en grand ; préparées par Rust (`get_curve`, `CURVE` dans store.rs)
  et dessinées par `src/curve.ts`.
- À ne pas confondre : « Taux souverain 10 ans » (`yield_10y`) = échantillon `G_N_C`, toutes notations (taux
  moyen de la zone euro, plus élevé que le 10 ans AAA : 3,93 % contre 3,31 % le 24/11/2026).
Quand une nouvelle version suit une série de plus, elle est téléchargée au lancement sans attendre les 6 h.
Téléchargement : 62 séries en 12 requêtes (`ecb::downloads`), 4 à la fois (`PARALLEL_DOWNLOADS`). La BCE met
~2 s à répondre quelle que soit la taille : les séries dont les clés ne diffèrent que par une dimension partent
ensemble (`combined_key` : « HICP.M.U2+DE+FR+….N.000000.4D0.ANR », courbe « …SR_3M+SR_6M+… », taux par pays,
crédit) ; la réponse est triée par la colonne `KEY`. Le taux de dépôt reste seul (requête de la valeur d'avant).

## Agenda (décisions BCE, inflation flash)

Bandeau sous l'en-tête et rappels du jour J. Calendrier en vigueur (`Calendar` dans `src-tauri/src/agenda.rs`) :
- dates intégrées à l'app (jusqu'à janvier 2028) ;
- remplacées par celles du fichier **`agenda.json`** à la racine du dépôt GitHub, lu à chaque actualisation
  (adresse `raw.githubusercontent.com/…/main/agenda.json`), dès qu'il va plus loin ou corrige une date ;
  copie locale `agenda.json` dans le dossier de données de Glaux (hors ligne). Fichier invalide : ignoré.
- **Chaque année** : ajouter les nouvelles dates dans `agenda.json` (sources dans le fichier), puis
  `git add agenda.json && git commit -m "Calendrier 2028" && git push` : toutes les apps installées les
  reçoivent, sans nouvelle version. Les dates intégrées, elles, se mettent à jour avec la version suivante.
Cache : `~/Library/Application Support/com.howzen.glaux/cache.json` (Mac), `%APPDATA%\com.howzen.glaux\cache.json` (Windows).
Actualisation automatique si les données ont plus de 6 h (vérification toutes les 15 min), ou via ↻ / ⌘R.
Préférences (alertes activées, rappels déjà envoyés, langue, indicateurs affichés) : `prefs.json`, dans le même dossier que le cache.

## Image exportée

Vue détail : choix du format (retenu d'une fois sur l'autre), « Copier » (⌘C) et « Enregistrer » (⌘E).
- Portrait 1080 × 1350 (LinkedIn, Instagram), paysage 1200 × 675 (LinkedIn, X), story 1080 × 1920 (TikTok, Reels).
  La story garde l'essentiel hors des zones couvertes par l'interface des applications (haut et bas de l'écran).
- Chaque format est une table de positions dans `src/export.ts` (`LAYOUTS`) ; le dessin est commun.
- Enregistrement : `Images/Glaux/Glaux_{indicateur}_{format}_{date}.png` (nom construit côté Rust à partir de
  valeurs connues). Copie : module presse-papiers de Tauri (`copy_png` dans `main.rs`).

## Carrousel LinkedIn (PDF)

Menu des formats › « Carrousel PDF », puis « Enregistrer » (⌘E) : un PDF prêt à joindre à un post LinkedIn
(« Ajouter un document » : chaque page devient une diapositive).
- Pages 4:5 (1080 × 1350) : couverture « La zone euro en chiffres » (mois, 4 chiffres clés de la zone euro :
  taux de dépôt, inflation, chômage, taux 10 ans), une page par indicateur affiché dans le panneau (dans son
  ordre, avec le pays, les pays des groupes et la période choisis), page « Sources et méthode » (date du
  téléchargement, calculs de Glaux, avertissement « pas un conseil en investissement »).
- L'interface dessine les pages en JPEG (`renderCarouselCover`, `buildImage`, `renderCarouselEnd`) ; Rust les
  assemble en PDF sans bibliothèque (`src-tauri/src/pdf.rs` : une image par page, 810 × 1012,5 points) et
  l'enregistre dans `Images/Glaux/Glaux_carrousel_{date}.pdf` (commande `save_carousel`). Pas de « Copier ».
- Environ 2 Mo pour 13 pages (LinkedIn accepte jusqu'à 300 pages et 100 Mo).

## Export Excel

Icône « tableau » dans l'en-tête (à côté de ↻) : toutes les séries dans `Documents/Glaux/Glaux_donnees_{date}.xlsx`,
construit côté Rust à partir du cache (`src-tauri/src/excel.rs`, crate `rust_xlsxwriter`), sans réseau.
- « Quotidien » : taux de dépôt (valeur en vigueur reportée sur chaque date), €STR, taux 10 ans, EUR/USD, pente (pb).
- « Mensuel » : inflation (totale, sous-jacente, services), chômage, M3, crédit, taux réel, taux 10 ans des 8 pays,
  écarts avec l'Allemagne des 7 pays (pb), quel que soit l'affichage choisi dans le panneau.
- « Par pays » : inflation (totale, sous-jacente, services) et chômage des 8 pays (32 colonnes).
- « Courbe des taux » : taux quotidiens de chaque échéance, de 3 mois à 30 ans.
- « Sources » : clé BCE ou calcul de chaque colonne, unité, date du dernier téléchargement.
Vraies dates Excel, formats de nombre par unité, en-tête figé, filtres ; titres dans la langue de Glaux.

## Indicateurs affichés

Icône « réglages » dans l'en-tête (ou ⌘,) : écran « Indicateurs affichés ».
- Clic sur une ligne (ou Espace) : l'afficher ou la masquer. Le dernier indicateur affiché ne se masque pas.
- Trois rubriques, dans cet ordre (`Section` dans `store.rs`) : « Zone euro » (taux de la BCE, €STR, taux
  souverain 10 ans, courbe AAA, EUR/USD, M3, crédit), « Zone euro ou par pays » (inflation, sous-jacente,
  services, chômage, taux 10 ans par pays), « Calculés par Glaux ».
- Glisser une ligne (ou ↑ ↓ au clavier) : changer l'ordre, à l'intérieur de sa rubrique. Les séries de la BCE
  restent avant les indicateurs calculés par Glaux, pour ne pas mélanger données officielles et calculs.
- Lignes masquées par défaut (`DEFAULT_HIDDEN` dans `store.rs`) : prefs.json ne retient que l'écart avec
  l'affichage par défaut (`hidden` : lignes habituelles masquées ; `shown` : lignes masquées par défaut affichées).
- Taux par pays et écarts de taux : pastilles des pays sous la ligne (3 au plus, au moins 1) ; enregistrées
  dans `yields` et `spreads` (`CHOOSABLE` dans `store.rs`, commande `set_selection`).
- « Par défaut » (`reset_layout`) : ordre d'origine, lignes habituelles, pays par défaut, zone euro partout.
- Enregistré dans `prefs.json` (`order`, `hidden`, `shown`, `yields`, `spreads`) par Rust (`store.rs`, `set_layout`, `set_selection`). Une ligne ajoutée par
  une future version se place en fin de sa section ; un identifiant inconnu est ignoré.
- Les alertes, l'agenda et l'export Excel ne dépendent pas de ce choix.

Hauteur du panneau : calculée par l'interface d'après son contenu (`fitPanel` dans `src/main.ts`), appliquée
par Rust (`resize_panel` dans `main.rs`), qui replace le panneau contre l'icône. Liste : juste la place des
lignes affichées ; vue détail : même hauteur que la liste, 560 points au minimum pour la courbe ; jamais plus
grand que la zone utile de l'écran, sans la barre des menus ni le Dock / la barre des tâches (`work_area`) ;
la liste défile alors. Le panneau se déplace en le faisant glisser par son en-tête (`data-tauri-drag-region`,
permission `core:window:allow-start-dragging`) ; il revient contre l'icône à la prochaine ouverture.

Panneau agrandi (bouton à droite de ↻, retenu d'une fois sur l'autre) : 1040 × 820 points (moins si la zone
utile de l'écran est plus petite), liste à gauche et vue détail à droite (le premier indicateur s'ouvre d'office ; ↑ ↓ pour
passer d'un indicateur à l'autre) ; « Indicateurs affichés » s'ouvre aussi à droite. Mise en page : grille
`.panel.is-large` dans `src/styles.css`.

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
- **Clic sur une notification (Mac, app compilée)** : le panneau s'ouvre sur l'indicateur concerné (taux de dépôt
  ou inflation). L'indicateur voyage dans l'identifiant de la notification (« glaux-hicp-3 ») ; un délégué
  (`NotificationDelegate`) le transmet à Rust, qui le garde (`take_pending_open`) et prévient l'interface
  (événement `open-indicator`) : ça marche aussi quand le clic relance Glaux.
- **Windows / Linux** : module de notifications de Tauri.

## Mises à jour automatiques

Module « updater » de Tauri (`src-tauri/src/updates.rs`). Glaux vérifie au lancement, toutes les 6 h et au réveil
de l'ordinateur (si la dernière vérification date d'au moins 1 h) le fichier `latest.json` de la dernière version
publiée sur GitHub. Le réveil se repère à l'heure réelle : le fil d'arrière-plan tourne toutes les 5 min, un écart
bien plus grand entre deux tours signale une mise en veille. Si une version plus récente existe,
« Installer la vX.Y.Z » remplace le statut en bas du panneau ; un clic télécharge, vérifie la signature,
installe et relance. Rien ne s'installe sans clic. Pas de vérification en mode développement.

- Adresse et clé publique : `src-tauri/tauri.conf.json` › `plugins.updater` (écrites par `premiere-mise-en-ligne.command`).
- Clé privée : `~/.tauri/glaux.key` sur le Mac (+ mot de passe), copiée dans les secrets du dépôt GitHub.
  **À sauvegarder** : sans elle, plus aucune mise à jour ne peut être publiée.
- `requireSignedVersion` activé : une version signée ne peut pas être resservie sous un autre numéro.

## Publier une version

Sur GitHub (dépôt public, compilation gratuite) : `.github/workflows/release.yml` compile Mac (app universelle :
puce Apple et Intel dans un seul DMG, `--target universal-apple-darwin`) et Windows à chaque étiquette `vX.Y.Z`, et prépare un **brouillon** de version avec les installeurs et `latest.json`.
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
| `src-tauri/src/main.rs` | Icône de la barre des menus, panneau (position, hauteur), commandes appelées par l'interface, fil d'actualisation |
| `src-tauri/src/ecb.rs` | Les 6 séries BCE, téléchargement et lecture du CSV |
| `src-tauri/src/data.rs` | Calculs : dernière valeur, variation sur 1 mois, mini-courbe, historique, taux réel, pente |
| `src-tauri/src/agenda.rs` | Calendrier des décisions BCE et des inflations flash (dates intégrées + `agenda.json` publié) et rappels du jour J |
| `agenda.json` | Calendrier publié, lu par les apps installées (**à compléter chaque année**) |
| `src-tauri/src/macos_notify.rs` | Notifications Mac (UNUserNotificationCenter, repli osascript en mode dev) |
| `src-tauri/src/alerts.rs` | Textes des notifications : changement de taux BCE, nouvelle inflation |
| `src-tauri/src/i18n.rs` | Langue (système ou choix), noms des mois et nombres en FR / EN |
| `src-tauri/src/excel.rs` | Export Excel des historiques (feuilles quotidienne, mensuelle, par pays, courbe des taux, sources) |
| `src-tauri/src/pdf.rs` | Carrousel LinkedIn : PDF minimal, une image JPEG par page |
| `src-tauri/src/updates.rs` | Mises à jour automatiques : vérification quotidienne, installation au clic |
| `src-tauri/src/store.rs` | Cache local (lecture, écriture, actualisation), indicateurs calculés à la demande, préférences (dont lignes affichées et leur ordre) |
| `src-tauri/tauri.conf.json` | Fenêtre, identifiant de l'app, icônes, sécurité, mises à jour |
| `src-tauri/tauri.release.conf.json` | En plus pour les versions publiées : DMG, installeur Windows, fichiers de mise à jour |
| `.github/workflows/` | GitHub Actions : tests à chaque envoi, compilation et brouillon de version à chaque étiquette |
| `premiere-mise-en-ligne.command`, `publier.command` | Mise en ligne (une fois) et publication d'une version |
| `README.md` | Page publique du dépôt (anglais + résumé en français) : installation, confidentialité |
| `src-tauri/capabilities/default.json` | Ce que l'interface a le droit de demander au cœur Rust |
| `src-tauri/Info.plist` | Mac : pas d'icône dans le Dock |
| `src-tauri/icons/` | Icônes générées depuis `app-icon.png` (`npm run tauri icon app-icon.png`) |
| `index.html`, `src/styles.css` | Structure et style du panneau (liste, vue détail, indicateurs affichés) |
| `src/main.ts` | Interface : liste, vue détail, indicateurs affichés (glisser, interrupteurs), hauteur du panneau, boutons, raccourcis |
| `src/chart.ts` | Courbes sans bibliothèque : mini-courbes, courbe de la vue détail, courbe de l'image |
| `src/curve.ts` | Courbe des taux : écart 10 ans − 2 ans, courbes à comparer, échéances de l'axe |
| `src/export.ts` | Image exportée : portrait 1080 × 1350, paysage 1200 × 675, story 1080 × 1920 (une table de positions par format), enregistrée dans Images › Glaux ou copiée ; pages du carrousel (couverture, sources) |
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
8. FR/EN, mises à jour automatiques, Windows, version publiée (v0.1.0)
9. V2 : écarts de taux par pays, formats d'image + copier, export Excel, choix et ordre des indicateurs (v0.2.0)
10. V3 : plus de données BCE, courbe des taux AAA, panneau agrandi, rubriques et indicateurs par pays (v0.3.0),
    carrousel LinkedIn, fiabilité (DMG universel, calendrier publié, clic sur notification, mises à jour
    toutes les 6 h, actualisation 2,5 fois plus rapide) ← **ici** (v0.4.0)
