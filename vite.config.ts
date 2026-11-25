// Configuration de Vite, l'outil qui sert l'interface pendant le développement
// et la compile (dossier dist/) pour la version finale.
import { defineConfig } from "vite";

export default defineConfig({
  // Ne pas effacer le Terminal : on garde les messages de Rust visibles.
  clearScreen: false,
  server: {
    // Tauri attend l'interface sur ce port précis (voir tauri.conf.json).
    port: 1420,
    strictPort: true,
    // Ne pas recharger l'interface quand on modifie le code Rust.
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // Cibles assez récentes pour le moteur web de macOS 14 et de Windows 10/11.
    target: ["safari17", "chrome120"],
  },
});
