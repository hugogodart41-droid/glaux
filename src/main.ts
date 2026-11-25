// Interface de Glaux : affiche les données envoyées par le cœur Rust.
// Rust télécharge et met en cache ; ici, on ne fait que de l'affichage.
// Deux écrans dans le même panneau : la liste des indicateurs, et la vue détail.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { formatCountdown, formatEventDate, upcomingEvents } from "./agenda";
import { COLORS, renderDetailChart, renderSparkline, type ChartSeries } from "./chart";
import {
  EXPORT_FORMATS,
  copyPng,
  formatSize,
  renderExportImage,
  renderGroupExportImage,
  savePng,
  type ExportFormat,
} from "./export";
import {
  IS_MAC,
  formatChange,
  formatDateLine,
  formatFullPeriod,
  formatNumber,
  formatUpdate,
  formatValue,
} from "./format";
import { lang, locale, setLang, t } from "./i18n";
import { displayFor, groupFor, type GroupDisplay } from "./indicators";
import { setupTooltips } from "./tooltip";
import type { DashboardView, IndicatorView, Observation } from "./types";

// ---------- État ----------

/** Dernières données reçues de Rust. */
let dashboard: DashboardView | null = null;
/** Indicateur ouvert en vue détail (null : on est sur la liste). */
let detailId: string | null = null;
/** Période de la vue détail : 12 (1 an) ou 60 (5 ans). */
let detailMonths = 60;
/** Historiques déjà chargés (clé « id:mois »), vidés à chaque actualisation. */
const historyCache = new Map<string, Observation[]>();
/** Numéro de la dernière demande d'historique : ignore les réponses arrivées en retard. */
let detailRequest = 0;
/** Message passager à la place du statut (ex. « Fichier Excel enregistré »), jusqu'à `until`. */
let statusFlash: { text: string; detail: string; until: number } | null = null;
/** Format de l'image exportée, retenu d'une fois sur l'autre (stockage local de l'interface). */
let exportFormat: ExportFormat = loadExportFormat();

function loadExportFormat(): ExportFormat {
  try {
    const saved = localStorage.getItem("glaux.exportFormat");
    return EXPORT_FORMATS.find((f) => f === saved) ?? "portrait";
  } catch {
    return "portrait";
  }
}

function saveExportFormat(format: ExportFormat): void {
  exportFormat = format;
  try {
    localStorage.setItem("glaux.exportFormat", format);
  } catch {
    // stockage indisponible : le choix vaut pour cette session seulement
  }
}

const $ = (id: string) => document.getElementById(id);

/** Crée un élément avec une classe et un texte (jamais de HTML injecté). */
function el<K extends keyof HTMLElementTagNameMap>(tag: K, className: string, text = "") {
  const node = document.createElement(tag);
  node.className = className;
  node.textContent = text;
  return node;
}

// ---------- Liste ----------

function renderRow(indicator: IndicatorView): HTMLLIElement | null {
  const display = displayFor(indicator.id);
  if (!display) return null; // indicateur inconnu de l'interface : ignoré

  const row = el("li", "indicator");
  row.dataset.id = indicator.id;

  // Survol du nom : à quoi sert cet indicateur.
  const left = el("div", "indicator__left");
  left.setAttribute("data-tooltip", display.hint);
  left.append(
    el("span", "indicator__label", display.label),
    el("span", "indicator__sub", formatDateLine(indicator, display)),
  );

  const right = el("div", "indicator__right");
  const value = indicator.latest ? formatValue(indicator.latest.value, display) : "—";
  right.append(
    el("span", "indicator__value", value),
    el("span", "indicator__sub", formatChange(indicator, display)),
  );

  row.append(left, renderSparkline(indicator.spark ?? [], display), right);
  return row;
}

/** Membres d'un groupe présents dans les données, avec leur présentation. */
function groupMembers(group: GroupDisplay, indicators: IndicatorView[]) {
  return group.members.flatMap((member) => {
    const indicator = indicators.find((i) => i.id === member.id);
    const display = displayFor(member.id);
    return indicator && display ? [{ member, indicator, display }] : [];
  });
}

/** Valeur courte d'un écart, en points de base sans unité : « 147 ». */
function shortBp(value: number | undefined): string {
  return value === undefined ? "—" : formatNumber(value * 100, 0);
}

/** Variation courte sur 1 mois, en points de base : « +45 », « −3 », « = ». */
function shortBpChange(indicator: IndicatorView): string {
  if (!indicator.latest || !indicator.reference) return "";
  const change = (indicator.latest.value - indicator.reference.value) * 100;
  return Math.abs(change) < 0.5 ? "=" : formatNumber(change, 0, true);
}

/** Ligne d'un groupe (écarts de taux) : nom à gauche, une valeur par pays à droite. */
function renderGroupRow(groupId: string, indicators: IndicatorView[]): HTMLLIElement | null {
  const group = groupFor(groupId);
  if (!group) return null;
  const members = groupMembers(group, indicators);

  const row = el("li", "indicator indicator--group");
  row.dataset.id = groupId;

  const dated = members.find((m) => m.indicator.latest);
  const left = el("div", "indicator__left");
  left.setAttribute("data-tooltip", group.hint);
  left.append(
    el("span", "indicator__label", group.short),
    el("span", "indicator__sub", dated ? t().groupSub(formatDateLine(dated.indicator, dated.display)) : ""),
  );

  const cells = el("div", "group-cells");
  for (const { member, indicator, display } of members) {
    const cell = el("div", "group-cell");
    cell.style.setProperty("--serie", member.color);
    const value = indicator.latest ? formatValue(indicator.latest.value, display) : "—";
    cell.setAttribute("data-tooltip", `${t().memberHint(display.label, value, formatChange(indicator, display))}. ${display.hint}`);
    const top = el("span", "group-cell__top");
    top.append(el("span", "group-cell__code", member.code), el("span", "group-cell__value", shortBp(indicator.latest?.value)));
    cell.append(top, el("span", "indicator__sub", shortBpChange(indicator)));
    cells.append(cell);
  }

  row.append(left, cells);
  return row;
}

function renderStatus(view: DashboardView): void {
  const status = $("status");
  const refresh = $("refresh");
  if (!status || !refresh) return;

  // Message passager (export Excel) : prioritaire pendant quelques secondes.
  if (statusFlash && Date.now() < statusFlash.until) {
    status.textContent = statusFlash.text;
    status.setAttribute("data-tooltip", statusFlash.detail);
    refresh.classList.toggle("is-spinning", view.refreshing);
    return;
  }
  statusFlash = null;

  const hasData = view.indicators.some((i) => i.latest);
  const strings = t();
  let text: string;
  if (view.refreshing) {
    text = strings.refreshing;
  } else if (view.error && view.updatedAt) {
    const update = formatUpdate(view.updatedAt);
    text = strings.offlineSince(update.text, update.today);
  } else if (view.error && hasData) {
    text = strings.partialData;
  } else if (view.error) {
    text = strings.unreachable;
  } else if (view.updatedAt) {
    const update = formatUpdate(view.updatedAt);
    text = strings.updated(update.text, update.today);
  } else {
    text = strings.loadingData;
  }

  status.textContent = text;
  // Détail technique de l'erreur au survol (utile pour me l'envoyer).
  if (view.error) status.setAttribute("data-tooltip", view.error);
  else status.removeAttribute("data-tooltip");
  refresh.classList.toggle("is-spinning", view.refreshing);
}

function render(view: DashboardView): void {
  dashboard = view;
  // Langue choisie dans Rust (système ou préférence) : textes fixes remis à jour si elle change.
  if (setLang(view.language)) applyStaticTexts();
  // Nouvelles données : les historiques en mémoire sont périmés.
  if (!view.refreshing) historyCache.clear();

  const list = $("indicators");
  if (list) {
    const rows: HTMLLIElement[] = [];
    let sectionShown = false;
    const groupsShown = new Set<string>();
    for (const indicator of view.indicators) {
      // Intertitre avant le premier indicateur calculé par Glaux.
      if (indicator.derived && !sectionShown) {
        sectionShown = true;
        const section = el("li", "indicators__section", t().derivedSection);
        section.setAttribute("data-tooltip", t().derivedHint);
        rows.push(section);
      }
      // Groupe (écarts de taux) : une seule ligne, à la place de son premier membre.
      if (indicator.group) {
        if (groupsShown.has(indicator.group)) continue;
        groupsShown.add(indicator.group);
        const members = view.indicators.filter((i) => i.group === indicator.group);
        const row = renderGroupRow(indicator.group, members);
        if (row) rows.push(row);
        continue;
      }
      const row = renderRow(indicator);
      if (row) rows.push(row);
    }
    list.replaceChildren(...rows);
  }
  renderStatus(view);
  renderUpdate(view);
  renderAgenda();
  renderAlertsToggle(view);
  document.querySelector(".brand")?.setAttribute("data-tooltip", t().brandHint(view.version));
  if (detailId && !view.refreshing) void renderDetail();
}

// ---------- Agenda ----------

/** Bandeau sous l'en-tête : prochaine décision BCE et prochaine inflation. */
function renderAgenda(): void {
  const agenda = $("agenda");
  if (!agenda) return;
  const items = upcomingEvents(dashboard?.agenda).map((event) => {
    const item = el("button", `agenda__item agenda__item--${event.kind}`);
    item.type = "button";
    item.dataset.indicator = event.indicatorId;
    item.setAttribute("data-tooltip", event.hint);
    if (event.daysLeft <= 1) item.classList.add("is-soon"); // aujourd'hui ou demain : en or
    item.append(
      el("span", "agenda__label", event.label),
      el("span", "agenda__when", `${formatEventDate(event.date)} · ${formatCountdown(event.daysLeft)}`),
    );
    return item;
  });
  agenda.replaceChildren(...items);
  agenda.hidden = items.length === 0; // calendrier épuisé : bandeau masqué
}

// ---------- Vue détail ----------

async function loadHistory(id: string, months: number): Promise<Observation[]> {
  const key = `${id}:${months}`;
  const cached = historyCache.get(key);
  if (cached) return cached;
  const points = await invoke<Observation[]>("get_history", { id, months });
  historyCache.set(key, points);
  return points;
}

function openDetail(id: string): void {
  detailId = id;
  $("indicators")?.setAttribute("hidden", "");
  $("detail")?.removeAttribute("hidden");
  void renderDetail();
}

function closeDetail(): void {
  detailId = null;
  $("detail")?.setAttribute("hidden", "");
  $("indicators")?.removeAttribute("hidden");
}

/** Remplit un élément de la vue détail par son identifiant. */
function setText(elementId: string, text: string): void {
  const node = $(elementId);
  if (node) node.textContent = text;
}

/** Période active (1 an / 5 ans) mise en évidence. */
function renderPeriodButtons(): void {
  document.querySelectorAll<HTMLButtonElement>("[data-months]").forEach((button) => {
    button.classList.toggle("is-active", Number(button.dataset.months) === detailMonths);
  });
}

async function renderDetail(): Promise<void> {
  const id = detailId;
  const group = id ? groupFor(id) : undefined;
  if (id && group) return renderGroupDetail(id, group);

  const indicator = dashboard?.indicators.find((i) => i.id === id);
  const display = id ? displayFor(id) : undefined;
  if (!id || !indicator || !display) {
    closeDetail();
    return;
  }

  // Vue d'un seul indicateur : valeur à droite, pas de légende.
  $("detail-legend")?.setAttribute("hidden", "");
  document.querySelector(".detail__right")?.removeAttribute("hidden");
  const set = setText;
  set("detail-title", display.label);
  set("detail-date", formatDateLine(indicator, display));
  set("detail-value", indicator.latest ? formatValue(indicator.latest.value, display) : "—");
  set("detail-change", formatChange(indicator, display));
  set("detail-explain", display.explanation);
  renderPeriodButtons();

  const months = detailMonths;
  const request = ++detailRequest;
  let points: Observation[];
  try {
    points = await loadHistory(id, months);
  } catch (err) {
    console.error("Historique indisponible :", err);
    points = [];
  }
  if (request !== detailRequest) return; // l'utilisateur a changé d'indicateur ou de période

  const readout = $("detail-readout");
  const chart = $("detail-chart");
  if (!readout || !chart) return;

  // Texte par défaut sous la période : plus bas et plus haut ; au survol : le point visé.
  const values = points.map((p) => p.value);
  const range = values.length
    ? t().range(formatValue(Math.min(...values), display), formatValue(Math.max(...values), display))
    : t().noHistory;
  readout.textContent = range;

  const onHover = (date: string | null, hovered: (number | null)[]) => {
    const value = hovered[0];
    readout.textContent =
      date && value !== null && value !== undefined
        ? t().readout(formatFullPeriod(date, display.monthly), formatValue(value, display))
        : range;
  };
  // La courbe prend toute la place libre du panneau.
  const width = chart.clientWidth || 364;
  const height = Math.max(150, chart.clientHeight || 150);
  const series: ChartSeries[] = [{ points, color: COLORS.or }];
  chart.replaceChildren(renderDetailChart(series, display, months, width, height, onHover));
}

/** Historiques des membres d'un groupe, chacun dans sa couleur. */
async function loadGroupSeries(group: GroupDisplay, months: number): Promise<ChartSeries[]> {
  return Promise.all(
    group.members.map(async (member) => {
      let points: Observation[] = [];
      try {
        points = await loadHistory(member.id, months);
      } catch (err) {
        console.error("Historique indisponible :", err);
      }
      return { points, color: member.color };
    }),
  );
}

/** Vue détail d'un groupe (écarts de taux) : légende par pays et courbes superposées. */
async function renderGroupDetail(id: string, group: GroupDisplay): Promise<void> {
  const members = groupMembers(group, dashboard?.indicators ?? []);
  const first = members.find((m) => m.indicator.latest) ?? members[0];
  if (!first) {
    closeDetail();
    return;
  }
  const display = first.display;

  setText("detail-title", group.label);
  setText("detail-date", `${t().monthlyAverages} · ${t().groupSub(formatDateLine(first.indicator, display))}`);
  setText("detail-explain", group.explanation);
  document.querySelector(".detail__right")?.setAttribute("hidden", "");
  renderPeriodButtons();

  // Légende : un pays par élément, dernière valeur et variation sur 1 mois.
  const legend = $("detail-legend");
  if (legend) {
    legend.replaceChildren(
      ...members.map(({ member, indicator, display: memberDisplay }) => {
        const item = el("span", "legend-item");
        item.style.setProperty("--serie", member.color);
        item.setAttribute("data-tooltip", `${memberDisplay.label} · ${memberDisplay.hint}`);
        const value = indicator.latest ? formatValue(indicator.latest.value, memberDisplay) : "—";
        item.append(
          el("span", "", member.code),
          el("span", "legend-item__value", value),
          el("span", "", shortBpChange(indicator)),
        );
        return item;
      }),
    );
    legend.removeAttribute("hidden");
  }

  const months = detailMonths;
  const request = ++detailRequest;
  const series = await loadGroupSeries(group, months);
  if (request !== detailRequest || detailId !== id) return;

  const readout = $("detail-readout");
  const chart = $("detail-chart");
  if (!readout || !chart) return;
  readout.textContent = t().hoverHint;

  // Au survol : « oct. 2026 : FR 147 · IT 126 · ES 65 pb ».
  const onHover = (date: string | null, hovered: (number | null)[]) => {
    if (!date) {
      readout.textContent = t().hoverHint;
      return;
    }
    const parts = group.members
      .map((member, k) => (hovered[k] === null || hovered[k] === undefined ? "" : `${member.code} ${shortBp(hovered[k] as number)}`))
      .filter(Boolean)
      .join(" · ");
    readout.textContent = t().readout(formatFullPeriod(date, true), `${parts} ${t().bpUnit}`);
  };
  const width = chart.clientWidth || 364;
  const height = Math.max(150, chart.clientHeight || 150);
  chart.replaceChildren(renderDetailChart(series, display, months, width, height, onHover));
}

/** Image de la vue détail ouverte, au format choisi. */
async function buildImage(id: string): Promise<Uint8Array> {
  const group = groupFor(id);
  if (group) {
    const members = groupMembers(group, dashboard?.indicators ?? []);
    const series = await loadGroupSeries(group, detailMonths);
    return renderGroupExportImage(group, members, series, detailMonths, exportFormat);
  }
  const indicator = dashboard?.indicators.find((i) => i.id === id);
  const display = displayFor(id);
  if (!indicator || !display) throw new Error("indicateur inconnu");
  const points = await loadHistory(id, detailMonths);
  return renderExportImage(indicator, display, points, detailMonths, dashboard?.agenda, exportFormat);
}

/**
 * « Enregistrer » (Images › Glaux, affichée dans le Finder) ou « Copier » (presse-papiers).
 * Le bouton affiche le résultat 2,5 s, puis reprend son texte.
 */
async function exportCurrent(action: "save" | "copy"): Promise<void> {
  const button = $(action === "save" ? "export" : "copy");
  const id = detailId;
  if (!button || !id || button.dataset.busy) return;

  const strings = t();
  button.dataset.busy = "1";
  button.textContent = action === "save" ? strings.exporting : strings.copying;
  try {
    const png = await buildImage(id);
    if (action === "save") {
      const path = await savePng(png, id, exportFormat);
      button.setAttribute("data-tooltip", path);
    } else {
      await copyPng(png);
    }
    button.textContent = action === "save" ? strings.exported : strings.copied;
  } catch (err) {
    console.error("Export impossible :", err);
    button.textContent = action === "save" ? strings.exportFailed : strings.copyFailed;
    button.setAttribute("data-tooltip", String(err));
  }
  window.setTimeout(() => {
    delete button.dataset.busy;
    applyExportButtons(); // texte d'origine, dans la langue du moment
  }, 2500);
}

// ---------- Export Excel ----------

/** Toutes les séries dans un fichier Excel (Documents › Glaux), montré dans le Finder. */
async function exportExcel(): Promise<void> {
  const button = $("excel");
  if (!button || button.classList.contains("is-busy")) return;
  button.classList.add("is-busy");
  try {
    const path = await invoke<string>("export_excel");
    statusFlash = { text: t().excelDone, detail: path, until: Date.now() + 5000 };
  } catch (err) {
    console.error("Export Excel impossible :", err);
    statusFlash = { text: t().excelFailed, detail: String(err), until: Date.now() + 8000 };
  }
  button.classList.remove("is-busy");
  const flash = statusFlash;
  if (dashboard) renderStatus(dashboard);
  // Le statut normal revient à la fin du message.
  window.setTimeout(() => {
    if (dashboard && statusFlash === flash) renderStatus(dashboard);
  }, flash.until - Date.now() + 50);
}

// ---------- Alertes (notifications) ----------

function renderAlertsToggle(view: DashboardView): void {
  const button = $("alerts");
  if (!button) return;
  // Activées dans Glaux mais refusées par macOS : on le dit, avec la raison au survol.
  const blocked = view.notifications && !!view.notificationError;
  const strings = t();
  button.textContent = blocked ? strings.alertsBlocked : view.notifications ? strings.alertsOn : strings.alertsOff;
  button.classList.toggle("is-off", !view.notifications);
  button.classList.toggle("is-blocked", blocked);
  button.setAttribute(
    "data-tooltip",
    blocked ? (view.notificationError ?? "") : view.notifications ? strings.alertsOnHint : strings.alertsOffHint,
  );
}

function toggleAlerts(): void {
  if (!dashboard) return;
  // Rust enregistre le choix puis renvoie la vue à jour (événement dashboard-updated).
  invoke("set_notifications", { enabled: !dashboard.notifications }).catch((err) =>
    console.error("Réglage des alertes impossible :", err),
  );
}

// ---------- Mise à jour de Glaux ----------

/** Nouvelle version disponible : un bouton en or remplace le statut, en bas à gauche. */
function renderUpdate(view: DashboardView): void {
  const button = $("update") as HTMLButtonElement | null;
  const status = $("status");
  if (!button || !status) return;
  const version = view.update;
  button.hidden = !version;
  status.hidden = !!version;
  if (!version) return;
  const strings = t();
  button.disabled = view.updating;
  button.textContent = view.updating ? strings.updating : strings.installUpdate(version);
  button.setAttribute(
    "data-tooltip",
    view.updateError ? strings.updateFailed(view.updateError) : strings.installUpdateHint(version),
  );
}

function installUpdate(): void {
  // Rust télécharge, vérifie la signature, installe puis relance Glaux.
  invoke("install_update").catch((err) => console.error("Mise à jour impossible :", err));
}

// ---------- Langue ----------

function toggleLanguage(): void {
  // Rust enregistre le choix, traduit le menu de l'icône et renvoie la vue (événement dashboard-updated).
  invoke("set_language", { language: lang() === "fr" ? "en" : "fr" }).catch((err) =>
    console.error("Changement de langue impossible :", err),
  );
}

/** Raccourci clavier affiché dans les infobulles : « ⌘R » sur Mac, « Ctrl+R » sous Windows. */
function keys(key: string): string {
  return `${IS_MAC ? "⌘" : "Ctrl+"}${key}`;
}

/** Boutons de la vue détail : format, « Copier », « Enregistrer » (dans la langue du moment). */
function applyExportButtons(): void {
  const strings = t();
  const save = $("export");
  if (save && !save.dataset.busy) {
    save.textContent = strings.exportImage;
    save.setAttribute("data-tooltip", strings.exportHint(keys("E")));
  }
  const copy = $("copy");
  if (copy && !copy.dataset.busy) {
    copy.textContent = strings.copyImage;
    copy.setAttribute("data-tooltip", strings.copyHint(keys("C")));
  }
  const select = $("export-format") as HTMLSelectElement | null;
  if (select) {
    for (const option of Array.from(select.options)) {
      const format = option.value as ExportFormat;
      option.textContent = strings.formatNames[format];
    }
    select.value = exportFormat;
    select.setAttribute("aria-label", strings.formatAria);
    select.setAttribute("data-tooltip", strings.formatHint(formatSize(exportFormat), strings.formatPlatforms[exportFormat]));
  }
}

/** Textes fixes du panneau (index.html), remis à chaque changement de langue. */
function applyStaticTexts(): void {
  const strings = t();
  const set = (selector: string, text: string, tooltip?: string) => {
    const node = document.querySelector<HTMLElement>(selector);
    if (!node) return;
    node.textContent = text;
    if (tooltip) node.setAttribute("data-tooltip", tooltip);
  };
  set(".brand__tagline", strings.tagline);
  set("#back", strings.back, strings.backHint);
  set('[data-months="12"]', strings.oneYear);
  set('[data-months="60"]', strings.fiveYears);
  set("#quit", strings.quit, strings.quitHint(keys("Q")));
  set("#language", strings.switchLanguage, strings.switchLanguageHint);
  applyExportButtons();

  const excel = $("excel");
  excel?.setAttribute("aria-label", strings.excelAria);
  excel?.setAttribute("data-tooltip", strings.excelHint);
  const refresh = $("refresh");
  refresh?.setAttribute("aria-label", strings.refresh);
  refresh?.setAttribute("data-tooltip", strings.refreshHint(keys("R")));
  $("agenda")?.setAttribute("aria-label", strings.agendaAria);
  $("indicators")?.setAttribute("aria-label", strings.indicatorsAria);
  document.querySelector(".segmented")?.setAttribute("aria-label", strings.periodAria);
  renderToday();
}

// ---------- Actions ----------

function quitApp(): void {
  invoke("quit_app").catch((err) => console.error("Impossible de quitter :", err));
}

function refreshNow(): void {
  invoke("refresh_now").catch((err) => console.error("Actualisation impossible :", err));
}

function setupButtons(): void {
  $("quit")?.addEventListener("click", quitApp);
  $("refresh")?.addEventListener("click", refreshNow);
  $("excel")?.addEventListener("click", () => void exportExcel());
  $("language")?.addEventListener("click", toggleLanguage);
  $("update")?.addEventListener("click", installUpdate);

  // Clic sur une ligne de la liste : vue détail (un seul écouteur pour toutes les lignes).
  $("indicators")?.addEventListener("click", (event) => {
    const row = (event.target as Element | null)?.closest<HTMLElement>(".indicator");
    if (row?.dataset.id) openDetail(row.dataset.id);
  });

  // Clic sur un événement de l'agenda : vue détail de l'indicateur concerné.
  $("agenda")?.addEventListener("click", (event) => {
    const item = (event.target as Element | null)?.closest<HTMLElement>(".agenda__item");
    if (item?.dataset.indicator) openDetail(item.dataset.indicator);
  });

  $("alerts")?.addEventListener("click", toggleAlerts);

  $("back")?.addEventListener("click", closeDetail);
  $("export")?.addEventListener("click", () => void exportCurrent("save"));
  $("copy")?.addEventListener("click", () => void exportCurrent("copy"));
  $("export-format")?.addEventListener("change", (event) => {
    const value = (event.target as HTMLSelectElement).value;
    const format = EXPORT_FORMATS.find((f) => f === value);
    if (format) saveExportFormat(format);
    applyExportButtons();
  });

  document.querySelectorAll<HTMLButtonElement>("[data-months]").forEach((button) => {
    button.addEventListener("click", () => {
      detailMonths = Number(button.dataset.months) || 60;
      void renderDetail();
    });
  });
}

// Échap : retour à la liste, ou ferme le panneau. ⌘Q quitte, ⌘R actualise,
// en vue détail : ⌘E enregistre l'image, ⌘C la copie (sauf si du texte est sélectionné).
function listenForShortcuts(): void {
  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      if (detailId) {
        closeDetail();
        return;
      }
      getCurrentWindow()
        .hide()
        .catch((err) => console.error("Impossible de masquer la fenêtre :", err));
      return;
    }
    const modifier = IS_MAC ? event.metaKey : event.ctrlKey;
    if (!modifier) return;
    const key = event.key.toLowerCase();
    if (key === "q") {
      event.preventDefault();
      quitApp();
    } else if (key === "r") {
      event.preventDefault(); // évite le rechargement de la page web
      refreshNow();
    } else if (key === "e" && detailId) {
      event.preventDefault();
      void exportCurrent("save");
    } else if (key === "c" && detailId && !window.getSelection()?.toString()) {
      event.preventDefault();
      void exportCurrent("copy");
    }
  });
}

function renderToday(): void {
  const node = $("today");
  if (!node) return;
  node.textContent = new Intl.DateTimeFormat(locale(), {
    weekday: "short",
    day: "numeric",
    month: "short",
  }).format(new Date());
}

// ---------- Démarrage ----------

async function start(): Promise<void> {
  applyStaticTexts();
  setupButtons();
  setupTooltips();
  listenForShortcuts();

  // 1. Nouvelles données envoyées par Rust (actualisation auto ou manuelle).
  await listen<DashboardView>("dashboard-updated", (event) => render(event.payload));

  // 2. Données déjà en cache : affichage immédiat, même hors ligne.
  render(await invoke<DashboardView>("get_dashboard"));

  // La date du jour change si le Mac reste allumé : date et compte à rebours
  // sont remis à jour à chaque ouverture du panneau.
  window.addEventListener("focus", () => {
    renderToday();
    renderAgenda();
  });
}

window.addEventListener("DOMContentLoaded", () => {
  start().catch((err) => console.error("Démarrage de l'interface impossible :", err));
});
