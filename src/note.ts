// Écran « Note macro » : faits du mois (écrits par Rust, src-tauri/src/note.rs), zone
// « Mon analyse » enregistrée automatiquement, copie du texte du post, PDF 4:5.
//
// PDF : couverture (4 chiffres clés), analyse, une page par rubrique (chiffre vedette + phrases),
// sources. Pages dessinées par src/export.ts, assemblées en PDF par Rust (commande save_note).

import { invoke } from "@tauri-apps/api/core";

import { renderCarouselCover, renderCarouselEnd, renderNoteTextPages, saveNotePdf } from "./export";
import { t } from "./i18n";
import type { NoteView } from "./types";

let note: NoteView | null = null;
/** Analyse tapée mais pas encore enregistrée (null : rien en attente). */
let unsaved: string | null = null;
let saveTimer: number | undefined;
/** Enregistrements enchaînés : jamais deux à la fois, toujours dans l'ordre de la frappe. */
let saving: Promise<void> = Promise.resolve();
/** Paragraphes de la page « Sources et méthode » (les mêmes que le carrousel, fournis par main.ts). */
let endParagraphs: () => string[] = () => [];
/** Raccourci affiché dans les infobulles (« ⌘E » / « Ctrl+E »), fourni par main.ts. */
let keys: (key: string) => string = (key) => key;

const $ = (id: string) => document.getElementById(id);
const analysisField = () => $("note-analysis") as HTMLTextAreaElement | null;

/** Demande la note du mois à Rust et l'affiche. */
export async function loadNote(): Promise<void> {
  try {
    note = await invoke<NoteView>("get_note");
  } catch (err) {
    console.error("Note macro indisponible :", err);
    return;
  }
  renderNote();
}

/** Longueur du post avec le texte en cours de frappe (le post de Rust contient l'analyse enregistrée). */
function postLength(): number {
  if (!note) return 0;
  const length = (text: string) => (text.trim() ? Array.from(text.trim()).length + 2 : 0);
  const current = unsaved ?? analysisField()?.value ?? note.analysis;
  return Array.from(note.post).length - length(note.analysis) + length(current);
}

function renderCount(): void {
  const node = $("note-count");
  if (!node || !note) return;
  const n = postLength();
  const strings = t();
  const tooLong = n > note.postLimit;
  node.textContent = tooLong ? strings.noteTooLong(n, note.postLimit) : strings.noteCount(n, note.postLimit);
  node.classList.toggle("is-warning", tooLong);
}

/** Titre, analyse (sauf pendant la frappe), compteur, phrases de chaque rubrique. */
export function renderNote(): void {
  const strings = t();
  const title = $("note-title");
  if (title) title.textContent = note?.title ?? strings.noteTitle;
  const field = analysisField();
  if (field && note && unsaved === null && document.activeElement !== field) field.value = note.analysis;
  renderCount();

  const container = $("note-sections");
  if (!container) return;
  const sections = (note?.sections ?? []).filter((s) => s.sentences.length > 0);
  if (sections.length === 0) {
    const empty = document.createElement("p");
    empty.className = "note__empty";
    empty.textContent = strings.noteEmpty;
    container.replaceChildren(empty);
    return;
  }
  container.replaceChildren(
    ...sections.map((section) => {
      const block = document.createElement("div");
      block.className = "note__section";
      const heading = document.createElement("div");
      heading.className = "indicators__section";
      heading.textContent = section.title;
      const list = document.createElement("ul");
      list.className = "note__facts";
      list.append(
        ...section.sentences.map((sentence) => {
          const item = document.createElement("li");
          item.textContent = sentence;
          return item;
        }),
      );
      block.append(heading, list);
      return block;
    }),
  );
}

/** Enregistre tout de suite l'analyse en attente (avant de copier, d'exporter ou de fermer). */
export function flushNote(): Promise<void> {
  window.clearTimeout(saveTimer);
  if (unsaved === null || !note) return saving;
  const text = unsaved;
  const month = note.month;
  unsaved = null;
  saving = saving
    .then(async () => {
      note = await invoke<NoteView>("set_note_analysis", { month, text });
      renderCount();
    })
    .catch((err) => console.error("Analyse non enregistrée :", err));
  return saving;
}

/** Bouton : affiche le résultat 2,5 s, puis reprend son texte. */
function flash(button: HTMLElement, text: string, tooltip?: string): void {
  button.textContent = text;
  if (tooltip) button.setAttribute("data-tooltip", tooltip);
  window.setTimeout(() => {
    delete button.dataset.busy;
    applyNoteTexts();
  }, 2500);
}

/** « Copier le texte » : le post (titre, analyse, chiffres, sources) dans le presse-papiers. */
async function copyPost(): Promise<void> {
  const button = $("note-copy");
  if (!button || button.dataset.busy) return;
  button.dataset.busy = "1";
  try {
    await flushNote();
    await invoke("copy_note");
    flash(button, t().noteCopied);
  } catch (err) {
    console.error("Copie du texte impossible :", err);
    flash(button, t().noteFailed, String(err));
  }
}

/** « Enregistrer le PDF » : couverture, analyse, rubriques, sources ; Images › Glaux. */
export async function saveNote(): Promise<void> {
  const button = $("note-save");
  if (!button || button.dataset.busy) return;
  button.dataset.busy = "1";
  const strings = t();
  try {
    await flushNote();
    await loadNote(); // données et analyse à jour
    if (!note) throw new Error("note indisponible");
    const current = note;
    const analysis = current.analysis.trim();
    const total = 2 + (analysis ? 1 : 0) + current.sections.filter((s) => s.sentences.length > 0).length;
    let page = 0;
    const progress = () => {
      page += 1;
      button.textContent = strings.carouselProgress(Math.min(page, total), total);
    };

    progress();
    const pages = [
      await renderCarouselCover(
        current.cover.map((f) => ({ label: f.label, value: f.value, date: f.date })),
        { kicker: current.title, title: strings.noteCoverTitle, subtitle: strings.noteCoverSubtitle(!!analysis) },
      ),
    ];
    if (analysis) {
      progress();
      const paragraphs = analysis.split(/\n+/).map((p) => p.trim()).filter(Boolean);
      pages.push(
        ...(await renderNoteTextPages(current.title, strings.noteAnalysisLabel, paragraphs, { bullets: false })),
      );
    }
    const sections = current.sections.filter((s) => s.sentences.length > 0);
    for (const [k, section] of sections.entries()) {
      progress();
      const hero = section.figures[0];
      pages.push(
        ...(await renderNoteTextPages(`${current.title} · ${k + 1}/${sections.length}`, section.title, section.sentences, {
          bullets: true,
          hero: hero && {
            value: hero.value,
            caption: [hero.label, hero.date, hero.change].filter(Boolean).join(" · "),
          },
        })),
      );
    }
    progress();
    pages.push(await renderCarouselEnd(endParagraphs()));
    const path = await saveNotePdf(pages);
    flash(button, strings.noteSaved, path);
  } catch (err) {
    console.error("PDF de la note impossible :", err);
    flash(button, strings.noteFailed, String(err));
  }
}

/** Textes fixes de l'écran, dans la langue du moment. */
export function applyNoteTexts(): void {
  const strings = t();
  const set = (id: string, text: string, tooltip?: string) => {
    const node = $(id);
    if (!node || node.dataset.busy) return;
    node.textContent = text;
    if (tooltip) node.setAttribute("data-tooltip", tooltip);
  };
  set("note-copy", strings.noteCopy, strings.noteCopyHint);
  set("note-save", strings.noteSave, strings.noteSaveHint(keys("E")));
  set("note-help", strings.noteHelp);
  set("note-analysis-label", strings.noteAnalysisLabel);
  analysisField()?.setAttribute("placeholder", strings.notePlaceholder);
  renderNote();
}

/** Branche l'écran : frappe (enregistrée 0,6 s après la dernière touche), copier, PDF. */
export function setupNote(options: { endParagraphs: () => string[]; keys: (key: string) => string }): void {
  endParagraphs = options.endParagraphs;
  keys = options.keys;
  const field = analysisField();
  field?.addEventListener("input", () => {
    unsaved = field.value;
    renderCount();
    window.clearTimeout(saveTimer);
    saveTimer = window.setTimeout(() => void flushNote(), 600);
  });
  field?.addEventListener("blur", () => void flushNote());
  $("note-copy")?.addEventListener("click", () => void copyPost());
  $("note-save")?.addEventListener("click", () => void saveNote());
}
