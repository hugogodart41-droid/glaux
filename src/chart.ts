// Courbes de Glaux, sans bibliothèque :
// - mini-courbe de la liste (SVG) ;
// - courbe de la vue détail (SVG, survolable) ;
// - courbe de l'image exportée (canvas).
// La géométrie (échelles, graduations, tracé) est commune aux trois.

import { formatNumber, formatValue, parsePeriod } from "./format";
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

/** Tracé SVG/canvas : segments droits, ou marches (horizontal puis vertical) pour un taux directeur. */
export function linePath(values: number[], x: (i: number) => number, y: (v: number) => number, step: boolean): string {
  let d = `M${x(0).toFixed(1)},${y(values[0]).toFixed(1)}`;
  for (let i = 1; i < values.length; i++) {
    d += step
      ? `H${x(i).toFixed(1)}V${y(values[i]).toFixed(1)}`
      : `L${x(i).toFixed(1)},${y(values[i]).toFixed(1)}`;
  }
  return d;
}

/**
 * Étiquettes de l'axe horizontal : les années (période > 1 an) ou un mois sur trois (1 an).
 * Renvoie le rang du premier point de chaque année / mois concerné.
 */
export function timeLabels(points: Observation[], months: number): { index: number; text: string }[] {
  const labels: { index: number; text: string }[] = [];
  let previousKey = "";
  points.forEach((point, index) => {
    const date = parsePeriod(point.date);
    if (!date) return;
    const byYear = months > 12;
    const key = byYear ? String(date.getFullYear()) : `${date.getFullYear()}-${date.getMonth()}`;
    if (key === previousKey) return;
    const isFirst = previousKey === "";
    previousKey = key;
    if (isFirst) return; // pas d'étiquette sur l'année ou le mois déjà entamé
    if (!byYear && date.getMonth() % 3 !== 0) return;
    const text = byYear
      ? String(date.getFullYear())
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
 * Dessine la courbe de la vue détail. `onHover` reçoit le point survolé (ou null en sortie).
 */
export function renderDetailChart(
  points: Observation[],
  display: Display,
  months: number,
  width: number,
  height: number,
  onHover: (point: Observation | null) => void,
): SVGSVGElement {
  const root = svg("svg", { class: "chart", width, height, viewBox: `0 0 ${width} ${height}` });
  if (points.length < 2) return root;

  const values = points.map((p) => p.value);
  const scale = niceScale(values, maxTickDecimals(display));
  const tickTexts = scale.ticks.map((t) => formatTick(t, scale.step, display));

  // Marge à gauche pour les valeurs des graduations : la courbe ne les recouvre jamais.
  const gutter = maxTextWidth(tickTexts, DETAIL_LABEL_FONT);
  const box: Box = {
    left: DETAIL_PAD.left + gutter + DETAIL_LABEL_GAP,
    top: DETAIL_PAD.top,
    width: width - DETAIL_PAD.left - gutter - DETAIL_LABEL_GAP - DETAIL_PAD.right,
    height: height - DETAIL_PAD.top - DETAIL_PAD.bottom,
  };
  const { x, y } = project(values, box, scale);

  // Graduations : ligne horizontale + valeur alignée à droite dans la marge.
  scale.ticks.forEach((tick, k) => {
    const ty = y(tick);
    root.append(
      svg("line", { x1: box.left, x2: box.left + box.width, y1: ty, y2: ty, class: "chart__grid" }),
    );
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

  // Étiquettes de temps sous la courbe.
  for (const { index, text } of timeLabels(points, months)) {
    const tx = x(index);
    root.append(svg("line", { x1: tx, x2: tx, y1: box.top + box.height, y2: box.top + box.height + 3, class: "chart__grid" }));
    const label = svg("text", { x: tx, y: height - 4, class: "chart__label", "text-anchor": "middle" });
    label.textContent = text;
    root.append(label);
  }

  // Aire dorée en dégradé sous la courbe, puis la courbe.
  const line = linePath(values, x, y, !!display.step);
  const bottom = box.top + box.height;
  const gradient = svg("linearGradient", { id: "chart-fill", x1: 0, x2: 0, y1: 0, y2: 1 });
  gradient.append(
    svg("stop", { offset: "0%", "stop-color": COLORS.or, "stop-opacity": 0.28 }),
    svg("stop", { offset: "100%", "stop-color": COLORS.or, "stop-opacity": 0 }),
  );
  const defs = svg("defs", {});
  defs.append(gradient);
  const last = values.length - 1;
  root.append(
    defs,
    svg("path", {
      d: `${line}L${x(last).toFixed(1)},${bottom}L${x(0).toFixed(1)},${bottom}Z`,
      class: "chart__area",
      fill: "url(#chart-fill)",
    }),
    svg("path", { d: line, class: "chart__line" }),
  );

  // Survol : ligne verticale + point sur la valeur la plus proche.
  const cursor = svg("line", { y1: box.top, y2: bottom, class: "chart__cursor", visibility: "hidden" });
  const dot = svg("circle", { r: 3.5, class: "chart__dot", visibility: "hidden" });
  const lastDot = svg("circle", { cx: x(last), cy: y(values[last]), r: 3, class: "chart__dot" });
  const hit = svg("rect", { x: 0, y: 0, width, height, fill: "transparent" });
  root.append(lastDot, cursor, dot, hit);

  hit.addEventListener("mousemove", (event) => {
    const rect = root.getBoundingClientRect();
    const mx = event.clientX - rect.left;
    const ratio = Math.min(1, Math.max(0, (mx - box.left) / box.width));
    const i = Math.round(ratio * last);
    cursor.setAttribute("x1", String(x(i)));
    cursor.setAttribute("x2", String(x(i)));
    dot.setAttribute("cx", String(x(i)));
    dot.setAttribute("cy", String(y(values[i])));
    cursor.setAttribute("visibility", "visible");
    dot.setAttribute("visibility", "visible");
    onHover(points[i]);
  });
  hit.addEventListener("mouseleave", () => {
    cursor.setAttribute("visibility", "hidden");
    dot.setAttribute("visibility", "hidden");
    onHover(null);
  });

  return root;
}

// ---------- Courbe de l'image exportée (canvas) ----------

/** Dessine la courbe dans un canvas, pour l'image exportée. Tailles en pixels de l'image. */
export function drawChartOnCanvas(
  ctx: CanvasRenderingContext2D,
  points: Observation[],
  display: Display,
  months: number,
  frame: Box,
  font: string,
): void {
  if (points.length < 2) return;
  const values = points.map((p) => p.value);
  const scale = niceScale(values, maxTickDecimals(display));
  const tickTexts = scale.ticks.map((t) => formatTick(t, scale.step, display));

  // Marge à gauche pour les valeurs des graduations : la courbe ne les recouvre jamais.
  const gutter = maxTextWidth(tickTexts, font, ctx);
  const plot: Box = {
    left: frame.left + gutter + 20,
    top: frame.top,
    width: frame.width - gutter - 20,
    height: frame.height,
  };
  const { x, y } = project(values, plot, scale);
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

  // Étiquettes de temps.
  ctx.textAlign = "center";
  for (const { index, text } of timeLabels(points, months)) {
    ctx.fillText(text, x(index), bottom + 44);
  }

  // Aire en dégradé puis courbe.
  const path = new Path2D(linePath(values, x, y, !!display.step));
  const last = values.length - 1;
  const area = new Path2D(
    `${linePath(values, x, y, !!display.step)}L${x(last)},${bottom}L${x(0)},${bottom}Z`,
  );
  const gradient = ctx.createLinearGradient(0, plot.top, 0, bottom);
  gradient.addColorStop(0, "rgba(201, 162, 74, 0.30)");
  gradient.addColorStop(1, "rgba(201, 162, 74, 0)");
  ctx.fillStyle = gradient;
  ctx.fill(area);

  ctx.strokeStyle = COLORS.or;
  ctx.lineWidth = 5;
  ctx.lineJoin = "round";
  ctx.lineCap = "round";
  ctx.stroke(path);

  // Point final.
  ctx.fillStyle = COLORS.or;
  ctx.beginPath();
  ctx.arc(x(last), y(values[last]), 9, 0, Math.PI * 2);
  ctx.fill();

  ctx.restore();
}
