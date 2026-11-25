# Glaux

**Euro area macro dashboard in your menu bar (Mac) or system tray (Windows).**
ECB deposit rate, inflation, €STR, 10-year sovereign yield, EUR/USD and unemployment, plus a real rate and a yield-curve slope computed by Glaux. In English and French.

Data comes straight from the European Central Bank's official API. No account, no tracking.

> **En français.** Glaux est un tableau de bord macro de la zone euro, dans la barre des menus (Mac) ou la zone de notification (Windows) : taux de la BCE, inflation, €STR, taux à 10 ans, EUR/USD, chômage, taux réel et pente des taux. Données officielles de la BCE, sans compte ni pistage. Installation ci-dessous ; sur Mac, la première ouverture passe par Réglages Système › Confidentialité et sécurité › « Ouvrir quand même ».

## Features

- 8 indicators: latest value, change over 1 month and a 1-year mini chart
- Detail view: 1-year or 5-year chart, low and high, a two-line explanation
- PNG export (1080 × 1350), ready for LinkedIn, Instagram or TikTok
- Calendar: next ECB decision and next flash inflation release, with a reminder on the day
- Notifications when the ECB changes rates or a new inflation figure comes out
- Works offline with the last downloaded data
- Signed automatic updates

## Install

Download the latest version from [Releases](../../releases/latest):

| Computer | File |
|---|---|
| Mac with Apple chip (M1 and later) | `Glaux_x.y.z_aarch64.dmg` |
| Intel Mac | `Glaux_x.y.z_x64.dmg` |
| Windows 10 or 11 | `Glaux_x.y.z_x64-setup.exe` |

Mac: macOS 14 Sonoma or later.

### First launch on Mac

The first time you open Glaux, macOS asks you to confirm it:

1. Open the DMG, drag Glaux into Applications, then open it. macOS says it cannot verify the app: click **Done**.
2. Open **System Settings › Privacy & Security**, scroll down to **Security** and click **Open Anyway** next to Glaux, then confirm with your password.

On macOS 14 Sonoma, Control-click Glaux in Applications and choose **Open** instead.

### First launch on Windows

On first install, Windows SmartScreen may show "Windows protected your PC": click **More info**, then **Run anyway**.

## Privacy

- No account, no analytics, no tracking.
- Glaux only connects to the ECB Data Portal (data, at most every 6 hours) and to GitHub (once a day, to check whether a new version exists).
- Data and settings stay on your computer.

## Build from source

Requires Node.js and Rust.

```bash
npm install
npm run tauri dev
```

Developer notes (in French): [LISEZMOI.md](LISEZMOI.md).

## Disclaimer

Glaux is an independent project, not affiliated with the European Central Bank. Figures are shown as published by the ECB. Nothing in Glaux is investment advice.

© 2026 Howzen. All rights reserved.
