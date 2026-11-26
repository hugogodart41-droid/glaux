// Interface de Glaux : affiche les données envoyées par le cœur Rust.
// Rust télécharge et met en cache ; ici, on ne fait que de l'affichage.
// Trois écrans dans le même panneau : la liste des indicateurs, la vue détail,
// et « Indicateurs affichés » (choix et ordre des lignes).
// La hauteur du panneau suit son contenu (fitPanel).

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { formatCountdown, formatEventDate, upcomingEvents } from "./agenda";
import { COLORS, renderDetailChart, renderSparkline, type ChartSeries } from "./chart";
import {
  EXPORT_FORMATS,
  copyPng,
  formatSize,
  renderCarouselCover,
  renderCarouselEnd,
  renderCurveExportImage,
  renderExportImage,
  renderGroupExportImage,
  saveCarousel,
  savePng,
  type CoverFigure,
  type Encoding,
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
  shortChange,
  shortUnit,
  shortValue,
} from "./format";
import { lang, locale, setLang, t } from "./i18n";
import { curveChart, curveSlope, loadCurve, tenorAt, todayRates, type CurveView } from "./curve";
import {
  COUNTRY_CODES,
  CURVE_ID,
  CURVE_TENORS,
  countryCode,
  countryName,
  countrySeries,
  curveDisplay,
  curveSlopeDisplay,
  displayFor,
  groupCandidates,
  groupFor,
  hasCountries,
  MAX_MEMBERS,
  setGroupSelection,
  tenorName,
  type Display,
  type GroupDisplay,
} from "./indicators";
import { setupTooltips } from "./tooltip";
import type { DashboardView, IndicatorView, Observation, RowView, Section } from "./types";

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
/** Écran « Indicateurs affichés » ouvert. */
let customizeOpen = false;
/** Enregistrements de l'affichage en cours : les lignes reçues entre-temps sont ignorées. */
let layoutSaves = 0;
/** Format de l'image exportée, retenu d'une fois sur l'autre (stockage local de l'interface). */
/** Choix du menu d'export : un format d'image, ou le carrousel LinkedIn (PDF de plusieurs pages). */
type ExportChoice = ExportFormat | "carousel";
let exportFormat: ExportChoice = loadExportFormat();
/**
 * Panneau agrandi : liste à gauche, vue détail (ou « Indicateurs affichés ») à droite,
 * en grand. Retenu d'une fois sur l'autre.
 */
let large = loadLarge();

function loadLarge(): boolean {
  try {
    return localStorage.getItem("glaux.large") === "1";
  } catch {
    return false;
  }
}

function saveLarge(): void {
  try {
    localStorage.setItem("glaux.large", large ? "1" : "0");
  } catch {
    // Stockage indisponible : le choix vaut pour cette session.
  }
}

function loadExportFormat(): ExportChoice {
  try {
    const saved = localStorage.getItem("glaux.exportFormat");
    if (saved === "carousel") return "carousel";
    return EXPORT_FORMATS.find((f) => f === saved) ?? "portrait";
  } catch {
    return "portrait";
  }
}

function saveExportFormat(format: ExportChoice): void {
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

/**
 * Ligne d'un indicateur. `rowId` : la ligne du panneau (« hicp ») quand l'indicateur affiché
 * est sa version par pays (« hicp_fr ») ; le nom du pays (ou « Zone euro ») précède la date.
 */
function renderRow(indicator: IndicatorView, rowId = indicator.id, country?: string): HTMLLIElement | null {
  const display = displayFor(indicator.id);
  if (!display) return null; // indicateur inconnu de l'interface : ignoré

  const row = el("li", "indicator");
  row.dataset.id = rowId;

  // Survol du nom : à quoi sert cet indicateur.
  const left = el("div", "indicator__left");
  left.setAttribute("data-tooltip", display.hint);
  const date = formatDateLine(indicator, display);
  left.append(
    el("span", "indicator__label", display.label),
    el("span", "indicator__sub", hasCountries(rowId) ? `${countryName(country)} · ${date}` : date),
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

/** Ligne d'un groupe (taux ou écarts par pays) : nom à gauche, une valeur par pays à droite. */
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
    el(
      "span",
      "indicator__sub",
      dated ? t().groupSub(shortUnit(dated.display), formatDateLine(dated.indicator, dated.display)) : "",
    ),
  );

  const cells = el("div", "group-cells");
  for (const { member, indicator, display } of members) {
    const cell = el("div", "group-cell");
    cell.style.setProperty("--serie", member.color);
    const value = indicator.latest ? formatValue(indicator.latest.value, display) : "—";
    cell.setAttribute("data-tooltip", `${t().memberHint(display.label, value, formatChange(indicator, display))}. ${display.hint}`);
    const top = el("span", "group-cell__top");
    top.append(
      el("span", "group-cell__code", member.code),
      el("span", "group-cell__value", shortValue(indicator.latest?.value, display)),
    );
    cell.append(top, el("span", "indicator__sub", shortChange(indicator, display)));
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

/** Titre et infobulle d'une rubrique. */
function sectionTexts(section: Section): [string, string] {
  const strings = t();
  if (section === "euro") return [strings.euroSection, strings.euroHint];
  if (section === "country") return [strings.countrySection, strings.countryHint];
  return [strings.derivedSection, strings.derivedHint];
}

/** Liste du panneau : lignes affichées, dans l'ordre choisi (écran « Indicateurs affichés »). */
function renderList(view: DashboardView): void {
  const list = $("indicators");
  if (!list) return;
  const nodes: HTMLLIElement[] = [];
  let section: Section | null = null;
  for (const row of view.rows) {
    if (row.hidden) continue;
    // Indicateur par pays : la série du pays choisi (« hicp_fr »), ou la zone euro.
    const country = view.countries?.[row.id];
    const indicator = view.indicators.find((i) => i.id === countrySeries(row.id, country));
    // Groupe (taux, écarts, courbe) : une seule ligne pour ses membres.
    const node =
      row.id === CURVE_ID
        ? renderCurveRow(view.indicators)
        : groupFor(row.id)
          ? renderGroupRow(row.id, view.indicators.filter((i) => i.group === row.id))
          : indicator
            ? renderRow(indicator, row.id, country)
            : null;
    if (!node) continue;
    // Intertitre avant la première ligne de chaque rubrique.
    if (row.section !== section) {
      section = row.section;
      const [title, hint] = sectionTexts(section);
      const header = el("li", "indicators__section", title);
      header.setAttribute("data-tooltip", hint);
      nodes.push(header);
    }
    nodes.push(node);
  }
  list.replaceChildren(...nodes);
  markSelectedRow();
}

function render(incoming: DashboardView): void {
  // Choix d'affichage en cours d'enregistrement : on garde celui de l'écran (déjà à jour).
  const view =
    layoutSaves > 0 && dashboard
      ? { ...incoming, rows: dashboard.rows, selections: dashboard.selections, countries: dashboard.countries }
      : incoming;
  dashboard = view;
  for (const [groupId, ids] of Object.entries(view.selections ?? {})) setGroupSelection(groupId, ids);
  // Langue choisie dans Rust (système ou préférence) : textes fixes remis à jour si elle change.
  if (setLang(view.language)) applyStaticTexts();
  // Nouvelles données : les historiques en mémoire sont périmés.
  if (!view.refreshing) historyCache.clear();

  renderList(view);
  if (customizeOpen) renderCustomize();
  renderStatus(view);
  renderUpdate(view);
  renderAgenda();
  renderAlertsToggle(view);
  document.querySelector(".brand")?.setAttribute("data-tooltip", t().brandHint(view.version));
  if (large && !customizeOpen) ensureSelection();
  if (detailId && !view.refreshing) void renderDetail();
  scheduleFit();
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
  scheduleFit();
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

/** Affiche l'écran du moment (liste, détail ou « Indicateurs affichés ») et ajuste la hauteur. */
function showScreen(): void {
  const list = $("indicators");
  const detail = $("detail");
  const customize = $("customize");
  document.querySelector(".panel")?.classList.toggle("is-large", large);
  if (large) {
    // Agrandi : la liste reste visible ; à droite, la vue détail ou « Indicateurs affichés ».
    if (list) list.hidden = false;
    if (detail) detail.hidden = customizeOpen || detailId === null;
    markSelectedRow();
  } else {
    if (list) list.hidden = detailId !== null || customizeOpen;
    if (detail) detail.hidden = detailId === null;
  }
  if (customize) customize.hidden = !customizeOpen;
  $("customize-open")?.classList.toggle("is-active", customizeOpen);
  renderLargeButton();
  scheduleFit();
}

/** Panneau agrandi : ligne de la liste dont la vue détail est ouverte à droite. */
function markSelectedRow(): void {
  document.querySelectorAll<HTMLElement>("#indicators .indicator").forEach((row) => {
    row.classList.toggle("is-selected", large && row.dataset.id === detailId);
  });
}

/** Lignes affichées dans la liste, dans l'ordre (identifiants). */
function visibleRowIds(): string[] {
  return Array.from(document.querySelectorAll<HTMLElement>("#indicators .indicator")).flatMap((row) =>
    row.dataset.id ? [row.dataset.id] : [],
  );
}

/** Panneau agrandi : sans indicateur choisi (ou masqué entre-temps), on ouvre le premier de la liste. */
function ensureSelection(): void {
  if (!large) return;
  const ids = visibleRowIds();
  if (detailId && ids.includes(detailId)) return;
  if (ids.length === 0) return;
  detailId = ids[0];
  void renderDetail();
}

function openDetail(id: string): void {
  detailId = id;
  customizeOpen = false;
  showScreen();
  void renderDetail();
}

/** Indicateur demandé par un clic sur une notification (Rust le garde jusqu'à cette lecture). */
async function openPendingIndicator(): Promise<void> {
  try {
    const id = await invoke<string | null>("take_pending_open");
    if (id && dashboard?.indicators.some((i) => i.id === id)) openDetail(id);
  } catch (err) {
    console.error("Indicateur de la notification introuvable :", err);
  }
}

function closeDetail(): void {
  if (large) return; // agrandi : la vue détail reste ouverte à droite
  detailId = null;
  showScreen();
}

/** Agrandir ou réduire le panneau. */
function toggleLarge(): void {
  large = !large;
  saveLarge();
  if (large) {
    ensureSelection();
  } else {
    detailId = null; // réduit : retour à la liste
  }
  showScreen();
  if (large && detailId) void renderDetail();
}

function renderLargeButton(): void {
  const button = $("large-toggle");
  if (!button) return;
  const strings = t();
  button.setAttribute("aria-label", large ? strings.shrinkPanel : strings.enlargePanel);
  button.setAttribute("data-tooltip", large ? strings.shrinkPanel : strings.enlargePanel);
  button.classList.toggle("is-active", large);
}

/** Panneau agrandi : ↑ ↓ passent à l'indicateur précédent ou suivant. */
function moveSelection(step: -1 | 1): void {
  const ids = visibleRowIds();
  if (ids.length === 0) return;
  const index = detailId ? ids.indexOf(detailId) : -1;
  const next = ids[Math.min(ids.length - 1, Math.max(0, index + step))];
  if (next && next !== detailId) {
    openDetail(next);
    document.querySelector<HTMLElement>(`#indicators .indicator[data-id="${next}"]`)?.scrollIntoView({ block: "nearest" });
  }
}

/** Remplit un élément de la vue détail par son identifiant. */
/**
 * Ligne « Courbe des taux AAA » : la forme de la courbe du jour en mini-graphique
 * (3 mois → 30 ans) et ses deux extrémités (« 2,69 → 3,60 % »). La pente 2-10 ans a sa
 * propre ligne (indicateur calculé « slope »), avec son historique.
 */
function renderCurveRow(indicators: IndicatorView[]): HTMLLIElement | null {
  const ten = indicators.find((i) => i.id === "yc_10y");
  const rates = todayRates(indicators);
  if (!ten || rates.some((v) => v === undefined)) return null; // données pas encore téléchargées
  const values = rates as number[];
  const display = curveDisplay();

  const row = el("li", "indicator");
  row.dataset.id = CURVE_ID;
  const left = el("div", "indicator__left");
  left.setAttribute("data-tooltip", display.hint);
  left.append(
    el("span", "indicator__label", display.label),
    el("span", "indicator__sub", formatDateLine(ten, display)),
  );

  const spark = renderSparkline(values, display);
  const first = values[0];
  const last = values[values.length - 1];
  spark.setAttribute("data-tooltip", t().curveSparkHint(formatValue(first, display), formatValue(last, display)));

  const right = el("div", "indicator__right");
  right.setAttribute("data-tooltip", t().curveEndsHint);
  right.append(
    el("span", "indicator__value", `${formatNumber(first, display.decimals)} → ${formatValue(last, display)}`),
    el("span", "indicator__sub", t().curveEnds),
  );
  row.append(left, spark, right);
  return row;
}

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
  if (id === CURVE_ID) return renderCurveDetail();
  const group = id ? groupFor(id) : undefined;
  if (id && group) return renderGroupDetail(id, group);

  // Indicateur par pays : série du pays choisi (« hicp_fr ») ; zone euro sinon.
  const country = id ? dashboard?.countries?.[id] : undefined;
  const seriesId = id ? countrySeries(id, country) : undefined;
  const indicator = dashboard?.indicators.find((i) => i.id === seriesId);
  const display = seriesId ? displayFor(seriesId) : undefined;
  if (!id || !seriesId || !indicator || !display) {
    closeDetail();
    return;
  }
  const byCountry = hasCountries(id);
  const euro = byCountry && country ? dashboard?.indicators.find((i) => i.id === id) : undefined;

  // Vue d'un seul indicateur : valeur à droite ; par pays : pastilles et légende pays / zone euro.
  renderCountrySwitch(byCountry ? id : null);
  renderCountryLegend(euro ? { indicator, display, country, euro } : null);
  document.querySelector(".detail__right")?.removeAttribute("hidden");
  const set = setText;
  const date = formatDateLine(indicator, display);
  set("detail-title", display.label);
  set("detail-date", byCountry ? `${countryName(country)} · ${date}` : date);
  set("detail-value", indicator.latest ? formatValue(indicator.latest.value, display) : "—");
  set("detail-change", formatChange(indicator, display));
  set("detail-explain", display.explanation);
  renderPeriodButtons();

  const months = detailMonths;
  const request = ++detailRequest;
  const history = async (seriesToLoad: string): Promise<Observation[]> => {
    try {
      return await loadHistory(seriesToLoad, months);
    } catch (err) {
      console.error("Historique indisponible :", err);
      return [];
    }
  };
  // Pays choisi : la zone euro en pointillés, pour comparer.
  const [points, euroPoints] = await Promise.all([history(seriesId), euro ? history(id) : Promise.resolve([])]);
  if (request !== detailRequest) return; // l'utilisateur a changé d'indicateur, de pays ou de période

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
    // Pays : la zone euro est dessinée en premier (sous la courbe du pays).
    const value = hovered[euro ? 1 : 0];
    if (!date || value === null || value === undefined) {
      readout.textContent = range;
      return;
    }
    // Pays : « oct. 2025 : 3,4 % · zone euro 2,9 % ».
    const euroValue = euro ? hovered[0] : null;
    const text =
      euro && euroValue !== null && euroValue !== undefined
        ? t().countryReadout(
            formatValue(value, display),
            countryName(undefined).toLowerCase(),
            formatValue(euroValue, display),
          )
        : formatValue(value, display);
    readout.textContent = t().readout(formatFullPeriod(date, display.monthly), text);
  };
  // La courbe prend toute la place libre du panneau.
  const width = chart.clientWidth || 364;
  const height = Math.max(150, chart.clientHeight || 150);
  // Pays : zone euro en pointillés discrets, dessinée d'abord pour rester sous le pays.
  const series: ChartSeries[] = [{ points, color: COLORS.or }];
  if (euro) series.unshift({ points: euroPoints, color: COLORS.cremeDoux, dashed: true });
  chart.replaceChildren(renderDetailChart(series, display, months, width, height, onHover));
}

/**
 * Pastilles « ZE DE FR IT… » sous le titre d'un indicateur disponible par pays : un clic
 * affiche ce pays sur la ligne du panneau et dans la vue détail. `row` null : masquées.
 */
function renderCountrySwitch(row: string | null): void {
  const container = $("detail-countries");
  if (!container) return;
  if (!row) {
    container.setAttribute("hidden", "");
    container.replaceChildren();
    return;
  }
  const chosen = dashboard?.countries?.[row];
  const focused = (document.activeElement as HTMLElement | null)?.dataset.country;
  const chips = [undefined, ...COUNTRY_CODES].map((code) => {
    const chip = el("button", "chip", countryCode(code));
    chip.type = "button";
    chip.dataset.country = code ?? "";
    chip.setAttribute("aria-pressed", String(code === chosen));
    chip.setAttribute("data-tooltip", countryName(code));
    return chip;
  });
  container.setAttribute("aria-label", t().countriesSwitch);
  container.replaceChildren(...chips);
  container.removeAttribute("hidden");
  // Clavier : la pastille cliquée garde le focus après le nouvel affichage.
  if (focused !== undefined) chips.find((chip) => chip.dataset.country === focused)?.focus();
}

/** Légende quand un pays est affiché : le pays (trait or) et la zone euro (pointillés). */
function renderCountryLegend(
  shown: { indicator: IndicatorView; display: Display; country?: string; euro: IndicatorView } | null,
): void {
  const legend = $("detail-legend");
  if (!legend) return;
  if (!shown) {
    legend.setAttribute("hidden", "");
    legend.replaceChildren();
    return;
  }
  const { indicator, display, country, euro } = shown;
  const item = (name: string, view: IndicatorView, color: string, dashed: boolean) => {
    const node = el("span", dashed ? "legend-item legend-item--dashed" : "legend-item");
    node.style.setProperty("--serie", color);
    const value = view.latest ? formatValue(view.latest.value, display) : "—";
    node.setAttribute("data-tooltip", `${t().memberHint(name, value, formatChange(view, display))}`);
    node.append(el("span", "", name), el("span", "legend-item__value", value));
    return node;
  };
  legend.replaceChildren(
    item(countryName(country), indicator, COLORS.or, false),
    item(countryName(undefined), euro, COLORS.cremeDoux, true),
  );
  legend.removeAttribute("hidden");
}

/** Affiche un pays (ou la zone euro) sur une ligne ; Rust l'enregistre dans prefs.json. */
function setCountry(row: string, country: string | undefined): void {
  if (!dashboard || dashboard.countries?.[row] === country) return;
  const countries = { ...(dashboard.countries ?? {}) };
  if (country) countries[row] = country;
  else delete countries[row];
  dashboard = { ...dashboard, countries };
  renderList(dashboard);
  if (detailId === row) void renderDetail();
  layoutSaves += 1;
  invoke("set_country", { id: row, country: country ?? null })
    .catch((err) => console.error("Choix du pays non enregistré :", err))
    .finally(() => {
      layoutSaves -= 1;
    });
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

/** Vue détail d'un groupe (taux ou écarts par pays) : légende par pays et courbes superposées. */
async function renderGroupDetail(id: string, group: GroupDisplay): Promise<void> {
  const members = groupMembers(group, dashboard?.indicators ?? []);
  const first = members.find((m) => m.indicator.latest) ?? members[0];
  if (!first) {
    closeDetail();
    return;
  }
  const display = first.display;

  renderCountrySwitch(null);
  setText("detail-title", group.label);
  const unit = shortUnit(display);
  setText("detail-date", `${t().monthlyAverages} · ${t().groupSub(unit, formatDateLine(first.indicator, display))}`);
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
        const value = indicator.latest ? formatValue(indicator.latest.value, memberDisplay) : "—";
        item.setAttribute("data-tooltip", `${t().memberHint(memberDisplay.label, value, formatChange(indicator, memberDisplay))}. ${memberDisplay.hint}`);
        // Sans l'unité (déjà dans le sous-titre « en pb », « en % ») : 3 pays tiennent sur une ligne.
        item.append(
          el("span", "", member.code),
          el("span", "legend-item__value", shortValue(indicator.latest?.value, memberDisplay)),
          el("span", "", shortChange(indicator, memberDisplay)),
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

  // Au survol : « oct. 2026 : FR 147 · IT 126 · ES 65 pb » ou « … : DE 2,71 · FR 3,42 · IT 3,56 % ».
  const onHover = (date: string | null, hovered: (number | null)[]) => {
    if (!date) {
      readout.textContent = t().hoverHint;
      return;
    }
    const parts = group.members
      .map((member, k) => {
        const value = hovered[k];
        return value === null || value === undefined ? "" : `${member.code} ${shortValue(value, display)}`;
      })
      .filter(Boolean)
      .join(" · ");
    const withUnit = display.kind === "bp" ? t().basisPoints(parts) : t().percent(parts);
    readout.textContent = t().readout(formatFullPeriod(date, true), withUnit);
  };
  const width = chart.clientWidth || 364;
  const height = Math.max(150, chart.clientHeight || 150);
  chart.replaceChildren(renderDetailChart(series, display, months, width, height, onHover));
}

/**
 * Vue détail de la courbe des taux : écart 10 ans − 2 ans à droite, courbe du jour (or) et
 * courbe d'il y a 1 ou 5 ans (pointillés crème), une échéance par graduation.
 */
async function renderCurveDetail(): Promise<void> {
  const indicators = dashboard?.indicators ?? [];
  const ten = indicators.find((i) => i.id === "yc_10y");
  const slope = curveSlope(indicators);
  const slopeDisplay = curveSlopeDisplay();
  if (!ten) {
    closeDetail();
    return;
  }
  const display = curveDisplay();

  // À droite : la pente 2-10 ans, la lecture la plus utile d'une courbe.
  renderCountrySwitch(null);
  setText("detail-title", display.label);
  setText("detail-date", t().curveSub(formatDateLine(ten, display)));
  document.querySelector(".detail__right")?.removeAttribute("hidden");
  setText("detail-value", slope?.latest && slopeDisplay ? formatValue(slope.latest.value, slopeDisplay) : "—");
  setText("detail-change", slope && slopeDisplay ? `${t().curveSlope} · ${formatChange(slope, slopeDisplay)}` : "");
  setText("detail-explain", display.explanation);
  renderPeriodButtons();

  const months = detailMonths;
  const request = ++detailRequest;
  let curve: CurveView = { latest: null, past: null };
  try {
    curve = await loadCurve(months);
  } catch (err) {
    console.error("Courbe indisponible :", err);
  }
  if (request !== detailRequest || detailId !== CURVE_ID) return;

  // Légende : date de chaque courbe, dans sa couleur.
  const legend = $("detail-legend");
  const { series, labels } = curveChart(curve);
  if (legend) {
    const items = [
      curve.latest ? { color: COLORS.or, text: formatFullPeriod(curve.latest.date, false) } : null,
      curve.past ? { color: COLORS.creme, text: `${formatFullPeriod(curve.past.date, false)} (${t().curveAgo(months)})` } : null,
    ].flatMap((item) => (item ? [item] : []));
    legend.replaceChildren(
      ...items.map(({ color, text }) => {
        const item = el("span", "legend-item");
        item.style.setProperty("--serie", color);
        item.append(el("span", "", text));
        return item;
      }),
    );
    legend.removeAttribute("hidden");
  }

  const readout = $("detail-readout");
  const chart = $("detail-chart");
  if (!readout || !chart) return;
  readout.textContent = t().curveHoverHint;

  // Au survol : « 10 ans : 3,93 % (il y a 1 an : 2,60 %) ».
  const onHover = (key: string | null, hovered: (number | null)[]) => {
    const tenor = key === null ? undefined : CURVE_TENORS[tenorAt(key)];
    const today = hovered[hovered.length - 1];
    if (!tenor || today === null || today === undefined) {
      readout.textContent = t().curveHoverHint;
      return;
    }
    const past = curve.past ? hovered[0] : null;
    const value = formatValue(today, display);
    readout.textContent =
      past === null || past === undefined
        ? t().readout(tenorName(tenor), value)
        : t().curveReadout(tenorName(tenor), value, t().curveAgo(months), formatValue(past, display));
  };
  const width = chart.clientWidth || 364;
  const height = Math.max(150, chart.clientHeight || 150);
  chart.replaceChildren(renderDetailChart(series, display, months, width, height, onHover, labels));
}

/**
 * Image d'un indicateur (ou groupe, ou courbe), telle que la vue détail l'afficherait :
 * pays choisi, pays des groupes, période (1 ou 5 ans). PNG pour l'image seule, JPEG pour
 * une page du carrousel.
 */
async function buildImage(id: string, format: ExportFormat, encoding: Encoding = "png"): Promise<Uint8Array> {
  if (id === CURVE_ID) {
    const curve = await loadCurve(detailMonths);
    return renderCurveExportImage(curve, curveSlope(dashboard?.indicators ?? []), detailMonths, format, encoding);
  }
  const group = groupFor(id);
  if (group) {
    const members = groupMembers(group, dashboard?.indicators ?? []);
    const series = await loadGroupSeries(group, detailMonths);
    return renderGroupExportImage(group, members, series, detailMonths, format, encoding);
  }
  const country = dashboard?.countries?.[id];
  const seriesId = countrySeries(id, country);
  const indicator = dashboard?.indicators.find((i) => i.id === seriesId);
  const display = displayFor(seriesId);
  if (!indicator || !display) throw new Error("indicateur inconnu");
  const points = await loadHistory(seriesId, detailMonths);
  if (seriesId === id) {
    return renderExportImage(indicator, display, points, detailMonths, dashboard?.agenda, format, undefined, encoding);
  }
  // Pays : son nom dans le titre, la zone euro en pointillés pour comparer.
  const euroPoints = await loadHistory(id, detailMonths);
  const reference = { title: `${display.label} · ${countryName(country)}`, reference: euroPoints };
  return renderExportImage(indicator, display, points, detailMonths, dashboard?.agenda, format, reference, encoding);
}

// ---------- Carrousel LinkedIn ----------

/** Chiffres clés de la couverture : toujours la zone euro (taux, inflation, chômage, 10 ans). */
function coverFigures(): CoverFigure[] {
  return ["deposit_rate", "hicp", "unemployment", "yield_10y"].flatMap((id) => {
    const indicator = dashboard?.indicators.find((i) => i.id === id);
    const display = displayFor(id);
    if (!indicator?.latest || !display) return [];
    return [
      {
        label: display.label,
        value: formatValue(indicator.latest.value, display),
        date: formatDateLine(indicator, display),
      },
    ];
  });
}

/** Paragraphes de la dernière page : source et date du téléchargement, méthode, avertissement. */
function endParagraphs(): string[] {
  const strings = t();
  const updated = dashboard?.updatedAt;
  const downloaded = updated
    ? new Intl.DateTimeFormat(locale(), { dateStyle: "long", timeStyle: "short" }).format(new Date(updated))
    : "—";
  return [strings.endSources(downloaded), strings.endComputed, strings.endMonthly, strings.endDisclaimer];
}

/** Nombre de pages du carrousel : couverture, indicateurs affichés, sources. */
function carouselPageCount(): number {
  return visibleRowIds().length + 2;
}

/**
 * Carrousel LinkedIn : couverture, une page portrait par indicateur affiché (dans l'ordre du
 * panneau, avec le pays et la période choisis), page des sources ; PDF dans Images › Glaux.
 */
async function exportCarousel(button: HTMLElement): Promise<string> {
  const ids = visibleRowIds();
  const total = ids.length + 2;
  const progress = (page: number) => {
    button.textContent = t().carouselProgress(page, total);
  };
  progress(1);
  const pages = [await renderCarouselCover(coverFigures())];
  for (const [k, id] of ids.entries()) {
    progress(k + 2);
    pages.push(await buildImage(id, "portrait", "jpeg"));
  }
  progress(total);
  pages.push(await renderCarouselEnd(endParagraphs()));
  return saveCarousel(pages);
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
  const carousel = exportFormat === "carousel";
  if (carousel && action === "copy") return; // un PDF ne se colle pas dans un post
  button.dataset.busy = "1";
  button.textContent = action === "save" ? strings.exporting : strings.copying;
  try {
    if (exportFormat === "carousel") {
      const path = await exportCarousel(button);
      button.setAttribute("data-tooltip", path);
      button.textContent = strings.carouselDone;
      window.setTimeout(() => {
        delete button.dataset.busy;
        applyExportButtons();
      }, 2500);
      return;
    }
    const png = await buildImage(id, exportFormat);
    if (action === "save") {
      // Nom du fichier : la série affichée (« hicp_fr » pour la France).
      const path = await savePng(png, countrySeries(id, dashboard?.countries?.[id]), exportFormat);
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

// ---------- Indicateurs affichés (choix et ordre des lignes) ----------

/** Nom d'une ligne : indicateur ou groupe. */
function rowLabel(id: string): string {
  if (id === CURVE_ID) return curveDisplay().label;
  return groupFor(id)?.label ?? displayFor(id)?.label ?? id;
}

function openCustomize(): void {
  if (!large) detailId = null; // agrandi : la vue détail revient à la fermeture
  customizeOpen = true;
  renderCustomize();
  showScreen();
}

function closeCustomize(): void {
  customizeOpen = false;
  showScreen();
  if (large) {
    ensureSelection();
    void renderDetail();
  }
}

/** Trois rubriques, comme dans la liste : zone euro, zone euro ou par pays, calculés par Glaux. */
function renderCustomize(): void {
  const container = $("customize-sections");
  if (!container || !dashboard || drag) return; // jamais pendant un glisser
  const rows = dashboard.rows;
  const lastVisible = rows.filter((row) => !row.hidden).length <= 1;
  const sections: Section[] = ["euro", "country", "derived"];
  const nodes = sections.flatMap((section) => {
    const [title] = sectionTexts(section);
    const items = rows.filter((row) => row.section === section);
    if (items.length === 0) return [];
    const list = el("ul", "customize__list");
    list.append(...items.map((row) => renderCustomItem(row, lastVisible)));
    return [el("div", "indicators__section", title), list];
  });
  // Les lignes sont recréées : le clavier reste au même endroit (ligne ou pastille de pays).
  const active = document.activeElement as HTMLElement | null;
  const chip = active?.dataset.member;
  const focused = active?.closest<HTMLElement>(".custom-item")?.dataset.id;
  container.replaceChildren(...nodes);
  const target = chip
    ? container.querySelector<HTMLElement>(`.chip[data-member="${chip}"]`)
    : focused
      ? container.querySelector<HTMLElement>(`.custom-item[data-id="${focused}"] .switch`)
      : null;
  target?.focus();
}

function renderCustomItem(row: RowView, lastVisible: boolean): HTMLLIElement {
  const strings = t();
  const label = rowLabel(row.id);
  const item = el("li", "custom-item");
  item.dataset.id = row.id;
  item.classList.toggle("is-hidden", row.hidden);

  const grip = el("span", "custom-item__grip");
  grip.setAttribute("data-tooltip", strings.moveHint);

  // Interrupteur : afficher / masquer. Le dernier indicateur affiché ne peut pas être masqué.
  const toggle = el("button", "switch");
  toggle.type = "button";
  toggle.setAttribute("role", "switch");
  toggle.setAttribute("aria-checked", String(!row.hidden));
  toggle.setAttribute("aria-label", label);
  if (!row.hidden && lastVisible) {
    toggle.setAttribute("aria-disabled", "true");
    toggle.setAttribute("data-tooltip", strings.keepOne);
  }

  const body = el("div", "custom-item__body");
  body.append(el("span", "custom-item__label", label));
  if (groupCandidates(row.id).length > 0) body.append(renderCountryChips(row.id));
  item.append(grip, body, toggle);
  return item;
}

/** Pays choisis pour une ligne (taux ou écarts par pays). */
function chosenMembers(groupId: string): string[] {
  return dashboard?.selections?.[groupId] ?? [];
}

/** Pastilles des pays d'une ligne : 3 au plus, au moins 1, dans la couleur de leur courbe. */
function renderCountryChips(groupId: string): HTMLElement {
  const strings = t();
  const chosen = chosenMembers(groupId);
  const colors = new Map((groupFor(groupId)?.members ?? []).map((m) => [m.id, m.color]));
  const chips = el("div", "country-chips");
  chips.setAttribute("role", "group");
  chips.setAttribute("aria-label", strings.countriesAria);
  for (const candidate of groupCandidates(groupId)) {
    const on = chosen.includes(candidate.id);
    const chip = el("button", "chip", candidate.code);
    chip.type = "button";
    chip.dataset.group = groupId;
    chip.dataset.member = candidate.id;
    chip.setAttribute("aria-pressed", String(on));
    const color = colors.get(candidate.id);
    if (on && color) chip.style.setProperty("--serie", color);
    const full = !on && chosen.length >= MAX_MEMBERS;
    const last = on && chosen.length <= 1;
    if (full || last) chip.setAttribute("aria-disabled", "true");
    const name = displayFor(candidate.id)?.label ?? candidate.code;
    chip.setAttribute("data-tooltip", full ? strings.countriesFull : last ? strings.countriesOne : name);
    chips.append(chip);
  }
  return chips;
}

/** Ajoute ou retire un pays d'une ligne (Rust l'enregistre dans prefs.json). */
function toggleMember(groupId: string, id: string): void {
  if (!dashboard) return;
  const current = chosenMembers(groupId);
  const next = current.includes(id) ? current.filter((x) => x !== id) : [...current, id];
  if (next.length === 0 || next.length > MAX_MEMBERS) return;
  const ordered = groupCandidates(groupId)
    .map((candidate) => candidate.id)
    .filter((x) => next.includes(x));
  dashboard = { ...dashboard, selections: { ...dashboard.selections, [groupId]: ordered } };
  setGroupSelection(groupId, ordered);
  renderCustomize();
  renderList(dashboard);
  layoutSaves += 1;
  invoke("set_selection", { group: groupId, members: ordered })
    .catch((err) => console.error("Choix des pays non enregistré :", err))
    .finally(() => {
      layoutSaves -= 1;
    });
}

/**
 * Enregistre l'ordre et les lignes masquées. L'écran est mis à jour tout de suite ;
 * Rust enregistre dans prefs.json puis renvoie la vue (événement dashboard-updated).
 */
function saveLayout(rows: RowView[]): void {
  if (!dashboard) return;
  dashboard = { ...dashboard, rows };
  renderCustomize();
  renderList(dashboard);
  layoutSaves += 1;
  invoke("set_layout", {
    order: rows.map((row) => row.id),
    hidden: rows.filter((row) => row.hidden).map((row) => row.id),
  })
    .catch((err) => console.error("Choix des indicateurs non enregistré :", err))
    .finally(() => {
      layoutSaves -= 1;
    });
}

function toggleRow(id: string): void {
  if (!dashboard) return;
  const rows = dashboard.rows.map((row) => (row.id === id ? { ...row, hidden: !row.hidden } : row));
  if (rows.every((row) => row.hidden)) return; // au moins une ligne reste affichée
  saveLayout(rows);
}

/** Clavier (↑ ↓ sur une ligne) : déplace la ligne d'un cran, dans sa section. */
function moveRow(id: string, step: -1 | 1): void {
  if (!dashboard) return;
  const rows = [...dashboard.rows];
  const from = rows.findIndex((row) => row.id === id);
  const to = from + step;
  if (from < 0 || to < 0 || to >= rows.length || rows[to].section !== rows[from].section) return;
  [rows[from], rows[to]] = [rows[to], rows[from]];
  saveLayout(rows);
}

/** Lignes dans l'ordre affiché à l'écran (après un glisser). */
function rowsFromScreen(): RowView[] {
  const byId = new Map((dashboard?.rows ?? []).map((row) => [row.id, row]));
  return Array.from(document.querySelectorAll<HTMLElement>(".custom-item")).flatMap((item) => {
    const row = byId.get(item.dataset.id ?? "");
    return row ? [row] : [];
  });
}

// Glisser pour réordonner : la ligne suit le pointeur ; dès que son centre dépasse celui
// d'une voisine, les deux échangent leur place (sans quitter la section).

/** Distance (en points) à partir de laquelle un appui devient un glisser, pas un clic. */
const DRAG_THRESHOLD = 4;

type Drag = { item: HTMLElement; pointerId: number; startY: number; lastY: number; shift: number; moved: boolean };
let drag: Drag | null = null;
/** Le clic qui suit un glisser ne masque pas la ligne. */
let ignoreClick = false;

/** Petite animation de la voisine qui change de place. */
function slide(node: HTMLElement, from: number): void {
  node.animate?.([{ transform: `translateY(${from}px)` }, { transform: "translateY(0)" }], {
    duration: 120,
    easing: "ease-out",
  });
}

function followPointer(current: Drag): void {
  const { item } = current;
  const list = item.parentElement;
  if (!list) return;
  for (;;) {
    const next = item.nextElementSibling as HTMLElement | null;
    const prev = item.previousElementSibling as HTMLElement | null;
    if (next && current.shift > next.offsetHeight / 2) {
      list.insertBefore(next, item); // la voisine remonte au-dessus
      current.shift -= next.offsetHeight;
      slide(next, item.offsetHeight);
    } else if (prev && current.shift < -prev.offsetHeight / 2) {
      list.insertBefore(prev, item.nextSibling); // la voisine descend en dessous
      current.shift += prev.offsetHeight;
      slide(prev, -item.offsetHeight);
    } else {
      break;
    }
  }
  // La ligne ne sort pas visuellement de sa section.
  let shift = current.shift;
  if (!item.previousElementSibling) shift = Math.max(shift, 0);
  if (!item.nextElementSibling) shift = Math.min(shift, 0);
  item.style.transform = `translateY(${shift}px)`;
}

function setupCustomize(): void {
  const root = $("customize");
  if (!root) return;

  root.addEventListener("pointerdown", (event) => {
    const target = event.target as Element | null;
    const item = target?.closest<HTMLElement>(".custom-item");
    if (event.button !== 0 || !item || target?.closest(".switch, .chip")) return;
    ignoreClick = false;
    drag = { item, pointerId: event.pointerId, startY: event.clientY, lastY: event.clientY, shift: 0, moved: false };
    item.setPointerCapture(event.pointerId);
  });

  root.addEventListener("pointermove", (event) => {
    if (!drag || event.pointerId !== drag.pointerId) return;
    if (!drag.moved && Math.abs(event.clientY - drag.startY) < DRAG_THRESHOLD) return;
    if (!drag.moved) {
      drag.moved = true;
      drag.item.classList.add("is-dragging");
    }
    drag.shift += event.clientY - drag.lastY;
    drag.lastY = event.clientY;
    followPointer(drag);
  });

  const endDrag = (event: PointerEvent) => {
    if (!drag || event.pointerId !== drag.pointerId) return;
    const { item, moved } = drag;
    drag = null;
    item.classList.remove("is-dragging");
    item.style.transform = "";
    if (moved) {
      ignoreClick = true;
      saveLayout(rowsFromScreen());
    }
  };
  root.addEventListener("pointerup", endDrag);
  root.addEventListener("pointercancel", endDrag);

  // Clic sur une ligne (ou son interrupteur, ou Espace au clavier) : afficher / masquer.
  root.addEventListener("click", (event) => {
    if (ignoreClick) {
      ignoreClick = false;
      return;
    }
    const target = event.target as Element | null;
    const chip = target?.closest<HTMLElement>(".chip");
    if (chip?.dataset.group && chip.dataset.member) {
      if (chip.getAttribute("aria-disabled") !== "true") toggleMember(chip.dataset.group, chip.dataset.member);
      return;
    }
    if (target?.closest(".country-chips")) return; // entre deux pastilles : rien
    const item = target?.closest<HTMLElement>(".custom-item");
    if (!item?.dataset.id) return;
    if (item.querySelector(".switch")?.getAttribute("aria-disabled") === "true") return;
    toggleRow(item.dataset.id);
    item.querySelector<HTMLElement>(".switch")?.focus();
  });

  root.addEventListener("keydown", (event) => {
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
    const source = event.target as Element | null;
    if (!source?.closest(".switch")) return;
    const item = source.closest<HTMLElement>(".custom-item");
    if (!item?.dataset.id) return;
    event.preventDefault();
    moveRow(item.dataset.id, event.key === "ArrowUp" ? -1 : 1);
  });

  $("customize-back")?.addEventListener("click", closeCustomize);
  // « Par défaut » : ordre d'origine, lignes habituelles, pays par défaut (Rust renvoie la vue).
  $("customize-reset")?.addEventListener("click", () => {
    invoke("reset_layout").catch((err) => console.error("Retour à la liste d'origine impossible :", err));
  });
}

// ---------- Hauteur du panneau ----------

/** Hauteur minimale de la vue détail : la courbe garde une taille lisible. */
const DETAIL_MIN_HEIGHT = 560;
/** Bordure du panneau, en haut et en bas (voir .panel dans styles.css). */
const PANEL_BORDER = 2;
/** Hauteur du panneau sur la liste, retenue pour la vue détail (mêmes dimensions si possible). */
let listHeight = 0;
let fitTimer: number | undefined;

/** Hauteur « naturelle » d'un élément : celle de ses enfants, quelle que soit la fenêtre. */
function contentHeight(node: HTMLElement | null): number {
  if (!node || node.hidden) return 0;
  const style = getComputedStyle(node);
  let height =
    parseFloat(style.paddingTop) +
    parseFloat(style.paddingBottom) +
    parseFloat(style.borderTopWidth) +
    parseFloat(style.borderBottomWidth);
  for (const child of Array.from(node.children)) height += child.getBoundingClientRect().height;
  return height;
}

/** Largeur du panneau normal (celle de tauri.conf.json). */
const PANEL_WIDTH = 400;
/** Panneau agrandi : taille visée. Rust la réduit à l'écran s'il est plus petit (resize_panel). */
const LARGE_WIDTH = 1040;
const LARGE_HEIGHT = 820;

function largeSize(): { width: number; height: number } {
  return { width: LARGE_WIDTH, height: LARGE_HEIGHT };
}

/** Hauteur voulue pour l'écran du moment : en-tête, agenda et pied compris. */
function desiredHeight(): number {
  const fixed = ["panel__header", "agenda", "panel__footer"]
    .map((name) => document.querySelector<HTMLElement>(`.${name}`)?.offsetHeight ?? 0)
    .reduce((a, b) => a + b, PANEL_BORDER);
  if (customizeOpen) return fixed + contentHeight($("customize"));
  if (detailId) return Math.max(listHeight, DETAIL_MIN_HEIGHT);
  listHeight = fixed + contentHeight($("indicators"));
  return listHeight;
}

/** Demande à Rust d'ajuster la hauteur du panneau (regroupe les demandes rapprochées). */
function scheduleFit(): void {
  window.clearTimeout(fitTimer);
  fitTimer = window.setTimeout(() => {
    const size = large ? largeSize() : { width: PANEL_WIDTH, height: Math.ceil(desiredHeight()) };
    if (Math.abs(size.height - window.innerHeight) < 1 && Math.abs(size.width - window.innerWidth) < 1) return;
    invoke("resize_panel", size).catch((err) => console.error("Redimensionnement impossible :", err));
  }, 0);
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
  // Carrousel : un PDF à joindre au post (« Ajouter un document »), pas d'image à copier.
  if (copy) copy.hidden = exportFormat === "carousel";
  const select = $("export-format") as HTMLSelectElement | null;
  if (select) {
    for (const option of Array.from(select.options)) {
      const format = option.value as ExportChoice;
      option.textContent = strings.formatNames[format];
    }
    select.value = exportFormat;
    select.setAttribute("aria-label", strings.formatAria);
    select.setAttribute(
      "data-tooltip",
      exportFormat === "carousel"
        ? strings.carouselHint(carouselPageCount())
        : strings.formatHint(formatSize(exportFormat), strings.formatPlatforms[exportFormat]),
    );
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

  set("#customize-back", strings.back, strings.backHint);
  set("#customize-reset", strings.resetLayout, strings.resetLayoutHint);
  set("#customize-title", strings.customizeTitle);
  set("#customize-help", strings.customizeHelp);
  renderLargeButton();
  const customize = $("customize-open");
  customize?.setAttribute("aria-label", strings.customizeAria);
  customize?.setAttribute("data-tooltip", strings.customizeHint(keys(",")));
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
  $("customize-open")?.addEventListener("click", () => (customizeOpen ? closeCustomize() : openCustomize()));
  $("large-toggle")?.addEventListener("click", toggleLarge);
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
    const format = value === "carousel" ? "carousel" : EXPORT_FORMATS.find((f) => f === value);
    if (format) saveExportFormat(format);
    applyExportButtons();
  });

  document.querySelectorAll<HTMLButtonElement>("[data-months]").forEach((button) => {
    button.addEventListener("click", () => {
      detailMonths = Number(button.dataset.months) || 60;
      void renderDetail();
    });
  });

  // Pastilles de pays (inflation, chômage) : zone euro ou un pays.
  $("detail-countries")?.addEventListener("click", (event) => {
    const chip = (event.target as Element | null)?.closest<HTMLElement>(".chip");
    if (chip && detailId) setCountry(detailId, chip.dataset.country || undefined);
  });
}

// Échap : retour à la liste, ou ferme le panneau. ⌘Q quitte, ⌘R actualise,
// ⌘, ouvre « Indicateurs affichés » ;
// en vue détail : ⌘E enregistre l'image, ⌘C la copie (sauf si du texte est sélectionné).
function listenForShortcuts(): void {
  window.addEventListener("keydown", (event) => {
    if (large && !customizeOpen && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      const target = event.target as Element | null;
      if (!target?.closest("select, input, textarea")) {
        event.preventDefault();
        moveSelection(event.key === "ArrowUp" ? -1 : 1);
        return;
      }
    }
    if (event.key === "Escape") {
      if (detailId && !large) {
        closeDetail();
        return;
      }
      if (customizeOpen) {
        closeCustomize();
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
    } else if (key === ",") {
      event.preventDefault();
      if (customizeOpen) closeCustomize();
      else openCustomize();
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
  setupCustomize();
  setupTooltips();
  listenForShortcuts();

  // 1. Nouvelles données envoyées par Rust (actualisation auto ou manuelle).
  await listen<DashboardView>("dashboard-updated", (event) => render(event.payload));

  // 2. Données déjà en cache : affichage immédiat, même hors ligne.
  render(await invoke<DashboardView>("get_dashboard"));
  // Panneau agrandi la dernière fois : on rouvre en grand, sur le premier indicateur.
  ensureSelection();
  showScreen();

  // 3. Clic sur une notification (Mac) : vue détail de l'indicateur concerné. Aussi au
  // lancement, quand c'est ce clic qui a démarré Glaux.
  await listen("open-indicator", () => void openPendingIndicator());
  await openPendingIndicator();

  // La date du jour change si le Mac reste allumé : date et compte à rebours
  // sont remis à jour à chaque ouverture du panneau.
  window.addEventListener("focus", () => {
    renderToday();
    renderAgenda();
  });

  // Le panneau a changé de hauteur (ex. liste courte → vue détail) : la courbe suit.
  let resizeTimer: number | undefined;
  window.addEventListener("resize", () => {
    window.clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(() => {
      if (detailId) void renderDetail();
    }, 30);
  });
}

window.addEventListener("DOMContentLoaded", () => {
  start().catch((err) => console.error("Démarrage de l'interface impossible :", err));
});
