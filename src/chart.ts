// Courbes de Glaux, sans bibliothèque :
// - mini-courbe de la liste (SVG) ;
// - courbe de la vue détail (SVG, survolable) ;
// - courbe de l'image exportée (canvas).
// La géométrie (échelles, graduations, tracé) est commune aux trois.
// Vue détail et image acceptent plusieurs courbes (ex. écarts France, Italie, Espagne) :
// une seule courbe garde l'aire dorée ; plusieurs courbes sont tracées chacune dans sa couleur.

import { formatNumber, formatValue, isQuarter, parsePeriod } from "./format";
import { locale, t } from "./i18n";
import type { Display } from "./indicators";
import type { Observation } from "./types";

const SVG_NS = "http://www.w3.org/2000/svg";

/** Palette (identique à styles.css). */
export const COLORS = {
  nuit: "#0e1a2b",
  ligne: "#22344f",
  creme: "#f5f1e8",
  cremeDoux: "#b9b4a8",
  or: "#c9a24a",
};

// ---------- Géométrie commune ----------

export type Box = { left: number; top: number; width: number; height: number };

type Scale = { lo: number; hi: number; step: number; ticks: number[] };

/**
 * Pas « rond » (1, 2, 2,5 ou 5 × 10^n) pour environ `count` intervalles,
 * sans plus de `maxDecimals` décimales (pas de « 7,25 % » pour un chômage à 1 décimale).
 */
function niceStep(range: number, count: number, maxDecimals: number): number {
  const raw = range / count;
  const power = 10 ** Math.floor(Math.log10(raw));
  const n = raw / power;
  const candidates = [1, 2, 2.5, 5, 10].filter((c) => c >= n).map((c) => c * power);
  const step = candidates.find((c) => stepDecimals(c) <= maxDecimals) ?? 10 * power;
  // Pas minimal : la plus petite unité affichable (ex. 0,1 pour 1 décimale).
  return Math.max(step, 10 ** -maxDecimals);
}

/** Échelle verticale avec des graduations rondes englobant min et max. */
export function niceScale(values: number[], maxDecimals = 2, count = 4): Scale {
  let min = Math.min(...values);
  let max = Math.max(...values);
  if (min === max) {
    // Courbe plate : on ouvre un peu l'échelle autour de la valeur.
    const pad = Math.abs(min) * 0.05 || 0.5;
    min -= pad;
    max += pad;
  }
  const step = niceStep(max - min, count, maxDecimals);
  const lo = Math.floor(min / step) * step;
  const hi = Math.ceil(max / step) * step;
  const ticks: number[] = [];
  for (let v = lo; v <= hi + step / 2; v += step) ticks.push(Number(v.toFixed(10)));
  return { lo, hi, step, ticks };
}

/** Nombre de décimales nécessaires pour afficher un pas de graduation (0,25 → 2). */
function stepDecimals(step: number): number {
  const text = String(Number(step.toFixed(6)));
  const dot = text.indexOf(".");
  return dot < 0 ? 0 : text.length - dot - 1;
}

/** Décimales maximales des graduations d'un indicateur (pour "bp" : points entiers). */
export function maxTickDecimals(display: Display): number {
  return display.kind === "bp" ? 2 : display.decimals;
}

export function formatTick(value: number, step: number, display: Display): string {
  const decimals = stepDecimals(step);
  switch (display.kind) {
    case "rate":
      return formatValue(value, display, decimals);
    case "bp":
      return formatValue(value, display, Math.max(2, decimals));
    default:
      return formatNumber(value, decimals);
  }
}

/** Largeur du texte le plus long, pour réserver la marge des graduations. */
function maxTextWidth(texts: string[], font: string, ctx?: CanvasRenderingContext2D): number {
  const context = ctx ?? document.createElement("canvas").getContext("2d");
  if (!context) return texts.reduce((w, t) => Math.max(w, t.length * 6), 0);
  context.save();
  context.font = font;
  // Les graduations du panneau ont des chiffres à chasse fixe (CSS tabular-nums), que le canvas
  // ne sait pas mesurer : chaque chiffre est compté comme un « 0 », le plus large. Sans ça,
  // « 1,150 » était mesuré trop court et son « 1 » coupé au bord du panneau.
  const width = texts.reduce((w, t) => Math.max(w, context.measureText(t.replace(/\d/g, "0")).width), 0);
  context.restore();
  return Math.ceil(width);
}

/** Position de chaque point : abscisse par rang (points équidistants), ordonnée par valeur. */
export function project(values: number[], box: Box, scale: { lo: number; hi: number }) {
  const n = values.length;
  const x = (i: number) => box.left + (n === 1 ? box.width / 2 : (i / (n - 1)) * box.width);
  const span = scale.hi - scale.lo || 1;
  const y = (v: number) => box.top + (1 - (v - scale.lo) / span) * box.height;
  return { x, y };
}

/**
 * Tracé SVG/canvas : segments droits, ou marches (horizontal puis vertical) pour un taux directeur.
 * Une valeur manquante (null) coupe la courbe.
 */
export function linePath(
  values: (number | null)[],
  x: (i: number) => number,
  y: (v: number) => number,
  step: boolean,
): string {
  let d = "";
  let drawing = false;
  values.forEach((v, i) => {
    if (v === null) {
      drawing = false;
      return;
    }
    if (!drawing) d += `M${x(i).toFixed(1)},${y(v).toFixed(1)}`;
    else d += step ? `H${x(i).toFixed(1)}V${y(v).toFixed(1)}` : `L${x(i).toFixed(1)},${y(v).toFixed(1)}`;
    drawing = true;
  });
  return d;
}

/** Une courbe d'un graphique (vue détail ou image exportée). */
/**
 * Une courbe : ses points et sa couleur. `dashed` : trait en pointillés, sans point final
 * (courbe de comparaison, ex. la courbe des taux d'il y a un an).
 */
export type ChartSeries = { points: Observation[]; color: string; dashed?: boolean };

/** Étiquettes de l'axe horizontal imposées (ex. échéances « 3M … 30A ») : rang du point et texte. */
export type AxisLabel = { index: number; text: string };

/**
 * Dates de toutes les courbes (réunies, dans l'ordre) et valeur de chaque courbe à chacune
 * (null si la courbe n'a pas de valeur ce jour-là ou ce mois-là).
 */
export function alignSeries(series: ChartSeries[]): { dates: string[]; rows: (number | null)[][] } {
  const dates = [...new Set(series.flatMap((s) => s.points.map((p) => p.date)))].sort();
  const rows = series.map((s) => {
    const byDate = new Map(s.points.map((p) => [p.date, p.value]));
    return dates.map((date) => byDate.get(date) ?? null);
  });
  return { dates, rows };
}

/** Rang de la dernière valeur connue d'une courbe (-1 si aucune). */
function lastIndex(values: (number | null)[]): number {
  for (let i = values.length - 1; i >= 0; i--) if (values[i] !== null) return i;
  return -1;
}

/**
 * Étiquettes de l'axe horizontal : les années (période > 1 an) ou un mois sur trois (1 an).
 * Renvoie le rang du premier point de chaque année / mois concerné.
 */
export function timeLabels(dates: string[], months: number): { index: number; text: string }[] {
  const labels: { index: number; text: string }[] = [];
  let previousKey = "";
  dates.forEach((period, index) => {
    const date = parsePeriod(period);
    if (!date) return;
    const byYear = months > 12;
    const key = byYear ? String(date.getFullYear()) : `${date.getFullYear()}-${date.getMonth()}`;
    if (key === previousKey) return;
    const isFirst = previousKey === "";
    previousKey = key;
    if (isFirst) return; // pas d'étiquette sur l'année ou le mois déjà entamé
    if (!byYear && date.getMonth() % 3 !== 0) return;
    // Série trimestrielle (PIB) sur 1 an : « T3 » plutôt que le premier mois du trimestre.
    const text = byYear
      ? String(date.getFullYear())
      : isQuarter(period)
        ? t().quarter(date.getMonth() / 3 + 1, date.getFullYear()).split(" ")[0]
        : new Intl.DateTimeFormat(locale(), { month: "short" }).format(date);
    labels.push({ index, text });
  });
  return labels;
}

function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>) {
  const node = document.createElementNS(SVG_NS, tag);
  for (const [name, value] of Object.entries(attrs)) node.setAttribute(name, String(value));
  return node;
}

// ---------- Mini-courbe (liste) ----------

const SPARK_WIDTH = 64;
const SPARK_HEIGHT = 24;
const SPARK_MARGIN = 3; // le trait et le point final ne sont pas coupés

export function renderSparkline(values: number[], display: Display): SVGSVGElement {
  const root = svg("svg", {
    class: "spark",
    width: SPARK_WIDTH,
    height: SPARK_HEIGHT,
    viewBox: `0 0 ${SPARK_WIDTH} ${SPARK_HEIGHT}`,
    "aria-hidden": "true",
  });
  if (values.length < 2) return root; // emplacement vide : l'alignement est gardé

  const min = Math.min(...values);
  const max = Math.max(...values);
  const box = {
    left: SPARK_MARGIN,
    top: SPARK_MARGIN,
    width: SPARK_WIDTH - 2 * SPARK_MARGIN,
    height: SPARK_HEIGHT - 2 * SPARK_MARGIN,
  };
  // Échelle serrée sur l'année (pas d'arrondi) : la forme de la courbe ressort mieux.
  const { x, y } = project(values, box, min === max ? { lo: min - 1, hi: max + 1 } : { lo: min, hi: max });
  const last = values.length - 1;

  root.setAttribute("data-tooltip", t().sparkHint(formatValue(min, display), formatValue(max, display)));
  root.append(
    svg("path", { d: linePath(values, x, y, !!display.step), class: "spark__line" }),
    svg("circle", { cx: x(last).toFixed(1), cy: y(values[last]).toFixed(1), r: 2, class: "spark__dot" }),
  );
  return root;
}

// ---------- Courbe de la vue détail (SVG survolable) ----------

const DETAIL_PAD = { left: 0, right: 8, top: 8, bottom: 18 };
const DETAIL_LABEL_FONT = `10px -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif`;
const DETAIL_LABEL_GAP = 6; // espace entre les valeurs des graduations et la courbe

/**
 * Dessine la courbe de la vue détail (une ou plusieurs courbes).
 * `onHover` reçoit la date survolée et la valeur de chaque courbe (ou null en sortie).
 */
export function renderDetailChart(
  series: ChartSeries[],
  display: Display,
  months: number,
  width: number,
  height: number,
  onHover: (date: string | null, values: (number | null)[]) => void,
  axisLabels?: AxisLabel[],
): SVGSVGElement {
  const root = svg("svg", { class: "chart", width, height, viewBox: `0 0 ${width} ${height}` });
  const { dates, rows } = alignSeries(series);
  const all = rows.flat().filter((v): v is number => v !== null);
  if (dates.length < 2 || all.length < 2) return root;

  const scale = niceScale(all, maxTickDecimals(display));
  const tickTexts = scale.ticks.map((t) => formatTick(t, scale.step, display));

  // Marge à gauche pour les valeurs des graduations : la courbe ne les recouvre jamais.
  const gutter = maxTextWidth(tickTexts, DETAIL_LABEL_FONT);
  // Étiquettes imposées (ex. « 30A ») centrées sous le dernier point : un peu plus de marge à droite.
  const right = axisLabels ? 14 : DETAIL_PAD.right;
  const box: Box = {
    left: DETAIL_PAD.left + gutter + DETAIL_LABEL_GAP,
    top: DETAIL_PAD.top,
    width: width - DETAIL_PAD.left - gutter - DETAIL_LABEL_GAP - right,
    height: height - DETAIL_PAD.top - DETAIL_PAD.bottom,
  };
  const { x, y } = project(dates.map(() => 0), box, scale);
  const bottom = box.top + box.height;
  const last = dates.length - 1;

  // Graduations : ligne horizontale + valeur alignée à droite dans la marge.
  scale.ticks.forEach((tick, k) => {
    const ty = y(tick);
    root.append(svg("line", { x1: box.left, x2: box.left + box.width, y1: ty, y2: ty, class: "chart__grid" }));
    const label = svg("text", {
      x: DETAIL_PAD.left + gutter,
      y: ty,
      class: "chart__label",
      "text-anchor": "end",
      "dominant-baseline": "middle",
    });
    label.textContent = tickTexts[k];
    root.append(label);
  });

  // Étiquettes de temps (ou imposées, ex. échéances) sous la courbe.
  for (const { index, text } of axisLabels ?? timeLabels(dates, months)) {
    const tx = x(index);
    root.append(svg("line", { x1: tx, x2: tx, y1: bottom, y2: bottom + 3, class: "chart__grid" }));
    const label = svg("text", { x: tx, y: height - 4, class: "chart__label", "text-anchor": "middle" });
    label.textContent = text;
    root.append(label);
  }

  const single = series.length === 1;
  if (single) {
    // Une seule courbe : aire dorée en dégradé dessous.
    const values = rows[0];
    const first = values.findIndex((v) => v !== null);
    const end = lastIndex(values);
    const gradient = svg("linearGradient", { id: "chart-fill", x1: 0, x2: 0, y1: 0, y2: 1 });
    gradient.append(
      svg("stop", { offset: "0%", "stop-color": COLORS.or, "stop-opacity": 0.28 }),
      svg("stop", { offset: "100%", "stop-color": COLORS.or, "stop-opacity": 0 }),
    );
    const defs = svg("defs", {});
    defs.append(gradient);
    root.append(
      defs,
      svg("path", {
        d: `${linePath(values, x, y, !!display.step)}L${x(end).toFixed(1)},${bottom}L${x(first).toFixed(1)},${bottom}Z`,
        class: "chart__area",
        fill: "url(#chart-fill)",
      }),
    );
  }

  // Courbes, puis point final de chacune.
  rows.forEach((values, k) => {
    const attrs: Record<string, string> = { d: linePath(values, x, y, !!display.step), class: "chart__line" };
    if (!single) {
      attrs.class = "chart__line chart__line--multi";
      attrs.style = `stroke: ${series[k].color}${series[k].dashed ? "; stroke-dasharray: 4 4" : ""}`;
    }
    root.append(svg("path", attrs));
  });
  rows.forEach((values, k) => {
    if (series[k].dashed) return;
    const end = lastIndex(values);
    const value = values[end];
    if (value === null || value === undefined) return;
    root.append(svg("circle", { cx: x(end), cy: y(value), r: 3, class: "chart__dot", style: `fill: ${series[k].color}` }));
  });

  // Survol : ligne verticale + un point par courbe sur la date la plus proche.
  const cursor = svg("line", { y1: box.top, y2: bottom, class: "chart__cursor", visibility: "hidden" });
  const dots = rows.map((_, k) =>
    svg("circle", { r: 3.5, class: "chart__dot", visibility: "hidden", style: `fill: ${series[k].color}` }),
  );
  const hit = svg("rect", { x: 0, y: 0, width, height, fill: "transparent" });
  root.append(cursor, ...dots, hit);

  const hide = () => {
    cursor.setAttribute("visibility", "hidden");
    dots.forEach((dot) => dot.setAttribute("visibility", "hidden"));
  };
  hit.addEventListener("mousemove", (event) => {
    const rect = root.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (event.clientX - rect.left - box.left) / box.width));
    const i = Math.round(ratio * last);
    cursor.setAttribute("x1", String(x(i)));
    cursor.setAttribute("x2", String(x(i)));
    cursor.setAttribute("visibility", "visible");
    rows.forEach((values, k) => {
      const value = values[i];
      if (value === null) {
        dots[k].setAttribute("visibility", "hidden");
        return;
      }
      dots[k].setAttribute("cx", String(x(i)));
      dots[k].setAttribute("cy", String(y(value)));
      dots[k].setAttribute("visibility", "visible");
    });
    onHover(dates[i], rows.map((values) => values[i]));
  });
  hit.addEventListener("mouseleave", () => {
    hide();
    onHover(null, []);
  });

  return root;
}

// ---------- Courbe de l'image exportée (canvas) ----------

/** Épaisseurs du dessin exporté, selon la taille de l'image. */
export type CanvasChartStyle = {
  /** Épaisseur du trait (une seule courbe ; un peu moins quand il y en a plusieurs). */
  line: number;
  /** Rayon du point final. */
  dot: number;
  /** Distance entre le bas de la courbe et les étiquettes de temps. */
  labelGap: number;
};

/**
 * Dessine la courbe (une ou plusieurs) dans un canvas, pour l'image exportée.
 * Tailles en pixels de l'image.
 */
export function drawChartOnCanvas(
  ctx: CanvasRenderingContext2D,
  series: ChartSeries[],
  display: Display,
  months: number,
  frame: Box,
  font: string,
  style: CanvasChartStyle = { line: 5, dot: 9, labelGap: 44 },
  axisLabels?: AxisLabel[],
): void {
  const { dates, rows } = alignSeries(series);
  const all = rows.flat().filter((v): v is number => v !== null);
  if (dates.length < 2 || all.length < 2) return;
  const scale = niceScale(all, maxTickDecimals(display));
  const tickTexts = scale.ticks.map((t) => formatTick(t, scale.step, display));

  // Marge à gauche pour les valeurs des graduations : la courbe ne les recouvre jamais.
  const gutter = maxTextWidth(tickTexts, font, ctx);
  const plot: Box = {
    left: frame.left + gutter + 20,
    top: frame.top,
    width: frame.width - gutter - 20,
    height: frame.height,
  };
  const { x, y } = project(dates.map(() => 0), plot, scale);
  const bottom = plot.top + plot.height;

  ctx.save();
  ctx.font = font;

  // Graduations : ligne horizontale + valeur alignée à droite dans la marge.
  ctx.strokeStyle = COLORS.ligne;
  ctx.lineWidth = 2;
  ctx.fillStyle = COLORS.cremeDoux;
  ctx.textAlign = "right";
  ctx.textBaseline = "middle";
  scale.ticks.forEach((tick, k) => {
    const ty = Math.round(y(tick));
    ctx.beginPath();
    ctx.moveTo(plot.left, ty);
    ctx.lineTo(plot.left + plot.width, ty);
    ctx.stroke();
    ctx.fillText(tickTexts[k], frame.left + gutter, ty);
  });
  ctx.textBaseline = "alphabetic";

  // Étiquettes de temps (ou imposées, ex. échéances).
  ctx.textAlign = "center";
  for (const { index, text } of axisLabels ?? timeLabels(dates, months)) {
    ctx.fillText(text, x(index), bottom + style.labelGap);
  }

  const single = series.length === 1;
  if (single) {
    // Une seule courbe : aire en dégradé dessous.
    const values = rows[0];
    const first = values.findIndex((v) => v !== null);
    const end = lastIndex(values);
    const area = new Path2D(
      `${linePath(values, x, y, !!display.step)}L${x(end)},${bottom}L${x(first)},${bottom}Z`,
    );
    const gradient = ctx.createLinearGradient(0, plot.top, 0, bottom);
    gradient.addColorStop(0, "rgba(201, 162, 74, 0.30)");
    gradient.addColorStop(1, "rgba(201, 162, 74, 0)");
    ctx.fillStyle = gradient;
    ctx.fill(area);
  }

  // Courbes puis point final de chacune.
  ctx.lineJoin = "round";
  ctx.lineCap = "round";
  rows.forEach((values, k) => {
    ctx.strokeStyle = series[k].color;
    ctx.lineWidth = single ? style.line : style.line * 0.8;
    ctx.setLineDash(series[k].dashed ? [style.line * 1.6, style.line * 1.8] : []);
    ctx.stroke(new Path2D(linePath(values, x, y, !!display.step)));
  });
  ctx.setLineDash([]);
  rows.forEach((values, k) => {
    if (series[k].dashed) return;
    const end = lastIndex(values);
    const value = values[end];
    if (value === null || value === undefined) return;
    ctx.fillStyle = series[k].color;
    ctx.beginPath();
    ctx.arc(x(end), y(value), single ? style.dot : style.dot * 0.78, 0, Math.PI * 2);
    ctx.fill();
  });

  ctx.restore();
}
