// Image exportée aux couleurs de Glaux, dessinée en canvas, sans bibliothèque.
// Trois formats :
// - portrait 1080 × 1350 (4:5) : LinkedIn, Instagram ;
// - paysage 1200 × 675 (16:9) : LinkedIn, X ;
// - story 1080 × 1920 (9:16) : TikTok, Reels, stories (contenu hors des zones couvertes
//   par l'interface de l'application : ~250 px en haut, le bas réservé au texte secondaire).
// Deux mises en page : un indicateur (grande valeur dorée) ou un groupe (une valeur par pays).
// Chaque format est une table de positions (LAYOUTS) ; le dessin est commun.

import { invoke } from "@tauri-apps/api/core";

import { exportAgendaLine } from "./agenda";
import { COLORS, drawChartOnCanvas, type CanvasChartStyle, type ChartSeries } from "./chart";
import { curveChart, type CurveView } from "./curve";
import {
  formatChange,
  formatDataDate,
  formatFullPeriod,
  formatValue,
  parsePeriod,
  shortChange,
  shortUnit,
  shortValue,
} from "./format";
import { locale, t } from "./i18n";
import { curveDisplay, curveSlopeDisplay, type Display, type GroupDisplay, type GroupMember } from "./indicators";
import type { AgendaData, IndicatorView, Observation } from "./types";

export type ExportFormat = "portrait" | "landscape" | "story";
export const EXPORT_FORMATS: ExportFormat[] = ["portrait", "landscape", "story"];

const SERIF = `"Iowan Old Style", Georgia, "Times New Roman", serif`;
const SANS = `-apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif`;

/** Texte placé à (x, y), ligne de base, taille de police en pixels. */
type Spot = { x: number; y: number; size: number };
/** Texte sur plusieurs lignes au plus `lines`, dans `maxWidth`. */
type Block = Spot & { maxWidth: number; lineHeight: number; lines: number };

type Layout = {
  width: number;
  height: number;
  margin: number;
  /** Marque « Glaux » (ligne de base), « Zone euro » et date, filet or. */
  header: { y: number; brand: number; tag: number; rule: number };
  /** Titre réduit jusqu'à `minSize` s'il dépasse `maxWidth`. */
  title: Spot & { maxWidth: number; minSize: number };
  hint: Block;
  /** Indicateur : grande valeur, variation + date de la donnée, rendez-vous de l'agenda. */
  value: Spot;
  change: Block;
  /** Position quand la variation tient sur une ligne ; descend d'une ligne sinon. */
  agenda: Spot;
  /**
   * Groupe : « columns » = une colonne par pays (portrait, story) ;
   * « rows » = une ligne par pays (paysage, colonne de gauche).
   * `top` : ligne de base du nom du premier pays.
   */
  group: {
    mode: "columns" | "rows";
    top: number;
    width: number;
    name: number;
    value: number;
    unit: number;
    change: number;
    rowHeight: number;
  };
  /** Groupe : « Moyennes mensuelles · oct. 2026 ». */
  month: Spot;
  /** « SUR 5 ANS · depuis novembre 2021 » au-dessus de la courbe. */
  period: Spot;
  chart: { left: number; top: number; width: number; height: number; font: number; style: CanvasChartStyle };
  explanation: Block;
  footer: { rule: number; y: number; size: number; brand: number };
};

const LAYOUTS: Record<ExportFormat, Layout> = {
  portrait: {
    width: 1080,
    height: 1350,
    margin: 80,
    header: { y: 112, brand: 46, tag: 28, rule: 150 },
    title: { x: 80, y: 255, size: 70, maxWidth: 920, minSize: 52 },
    hint: { x: 80, y: 305, size: 28, maxWidth: 920, lineHeight: 38, lines: 2 },
    value: { x: 74, y: 520, size: 150 },
    change: { x: 80, y: 580, size: 34, maxWidth: 920, lineHeight: 44, lines: 1 },
    agenda: { x: 80, y: 630, size: 28 },
    group: { mode: "columns", top: 420, width: 920, name: 32, value: 104, unit: 36, change: 28, rowHeight: 0 },
    month: { x: 80, y: 630, size: 30 },
    period: { x: 80, y: 692, size: 28 },
    chart: { left: 80, top: 750, width: 920, height: 275, font: 26, style: { line: 5, dot: 9, labelGap: 44 } },
    explanation: { x: 80, y: 1145, size: 30, maxWidth: 920, lineHeight: 42, lines: 3 },
    footer: { rule: 1268, y: 1314, size: 24, brand: 28 },
  },
  landscape: {
    width: 1200,
    height: 675,
    margin: 60,
    header: { y: 68, brand: 34, tag: 21, rule: 94 },
    // Colonne de gauche (x 60 → 480) : titre, valeur ; colonne de droite (x 520 → 1140) : courbe.
    title: { x: 60, y: 160, size: 44, maxWidth: 420, minSize: 30 },
    hint: { x: 60, y: 198, size: 19, maxWidth: 420, lineHeight: 26, lines: 3 },
    value: { x: 56, y: 365, size: 96 },
    change: { x: 60, y: 410, size: 22, maxWidth: 420, lineHeight: 30, lines: 2 },
    agenda: { x: 60, y: 455, size: 20 },
    group: { mode: "rows", top: 318, width: 420, name: 24, value: 44, unit: 20, change: 20, rowHeight: 56 },
    month: { x: 60, y: 490, size: 19 },
    period: { x: 520, y: 150, size: 19 },
    chart: { left: 520, top: 178, width: 620, height: 282, font: 18, style: { line: 3.5, dot: 6, labelGap: 31 } },
    explanation: { x: 60, y: 560, size: 21, maxWidth: 1080, lineHeight: 29, lines: 2 },
    footer: { rule: 612, y: 648, size: 17, brand: 21 },
  },
  story: {
    width: 1080,
    height: 1920,
    margin: 80,
    header: { y: 250, brand: 52, tag: 30, rule: 292 },
    title: { x: 80, y: 420, size: 80, maxWidth: 920, minSize: 56 },
    hint: { x: 80, y: 480, size: 32, maxWidth: 920, lineHeight: 44, lines: 3 },
    value: { x: 72, y: 770, size: 190 },
    change: { x: 80, y: 840, size: 38, maxWidth: 920, lineHeight: 50, lines: 2 },
    agenda: { x: 80, y: 905, size: 32 },
    group: { mode: "columns", top: 660, width: 920, name: 36, value: 124, unit: 42, change: 30, rowHeight: 0 },
    month: { x: 80, y: 925, size: 34 },
    period: { x: 80, y: 1035, size: 32 },
    chart: { left: 80, top: 1095, width: 920, height: 355, font: 30, style: { line: 6, dot: 10, labelGap: 51 } },
    explanation: { x: 80, y: 1590, size: 34, maxWidth: 920, lineHeight: 48, lines: 3 },
    footer: { rule: 1770, y: 1822, size: 28, brand: 32 },
  },
};

/** Dimensions d'un format (pour l'infobulle du choix de format). */
export function formatSize(format: ExportFormat): string {
  const { width, height } = LAYOUTS[format];
  return `${width} × ${height}`;
}

/** Découpe un texte en lignes qui tiennent dans `maxWidth` (au plus `maxLines`). */
function wrap(ctx: CanvasRenderingContext2D, text: string, maxWidth: number, maxLines: number): string[] {
  const lines: string[] = [];
  let line = "";
  for (const word of text.split(" ")) {
    const candidate = line ? `${line} ${word}` : word;
    if (ctx.measureText(candidate).width <= maxWidth || !line) {
      line = candidate;
    } else {
      lines.push(line);
      line = word;
    }
  }
  if (line) lines.push(line);
  if (lines.length > maxLines) {
    const kept = lines.slice(0, maxLines);
    kept[maxLines - 1] = `${kept[maxLines - 1].replace(/[\s,.;:]+$/, "")}…`;
    return kept;
  }
  return lines;
}

/** Écrit un bloc de texte sur plusieurs lignes (police déjà choisie). */
function drawBlock(ctx: CanvasRenderingContext2D, text: string, block: Block): void {
  wrap(ctx, text, block.maxWidth, block.lines).forEach((line, i) => {
    ctx.fillText(line, block.x, block.y + i * block.lineHeight);
  });
}

/** Canvas du format choisi, fond compris. */
function newCanvas(layout: Layout): { canvas: HTMLCanvasElement; ctx: CanvasRenderingContext2D } {
  const canvas = document.createElement("canvas");
  canvas.width = layout.width;
  canvas.height = layout.height;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("dessin impossible");
  ctx.fillStyle = COLORS.nuit;
  ctx.fillRect(0, 0, layout.width, layout.height);
  ctx.textBaseline = "alphabetic";
  return { canvas, ctx };
}

/** En-tête : marque + date du jour, filet or. */
function drawHeader(ctx: CanvasRenderingContext2D, layout: Layout): void {
  const { margin, width, header } = layout;
  ctx.textAlign = "left";
  ctx.fillStyle = COLORS.or;
  ctx.font = `600 ${header.brand}px ${SERIF}`;
  ctx.fillText("Glaux", margin, header.y);
  const brandWidth = ctx.measureText("Glaux").width;
  ctx.fillStyle = COLORS.cremeDoux;
  ctx.font = `${header.tag}px ${SANS}`;
  ctx.fillText(t().tagline, margin + brandWidth + header.tag * 0.65, header.y - 2);
  ctx.textAlign = "right";
  ctx.fillText(
    new Intl.DateTimeFormat(locale(), { day: "numeric", month: "long", year: "numeric" }).format(new Date()),
    width - margin,
    header.y - 2,
  );
  ctx.fillStyle = COLORS.or;
  ctx.fillRect(margin, header.rule, width - 2 * margin, 2);
}

/** Titre (réduit s'il est trop long) et sous-titre. */
function drawTitle(ctx: CanvasRenderingContext2D, layout: Layout, title: string, subtitle: string): void {
  const { title: spot, hint } = layout;
  ctx.textAlign = "left";
  ctx.fillStyle = COLORS.creme;
  let size = spot.size;
  ctx.font = `600 ${size}px ${SERIF}`;
  while (size > spot.minSize && ctx.measureText(title).width > spot.maxWidth) {
    size -= 2;
    ctx.font = `600 ${size}px ${SERIF}`;
  }
  // Titre encore trop long (« Inflation des services · Pays-Bas » en paysage) : réduit juste
  // assez pour tenir sur sa ligne, sans déborder sur la courbe.
  const width = ctx.measureText(title).width;
  if (width > spot.maxWidth) {
    size = Math.floor((size * spot.maxWidth) / width);
    ctx.font = `600 ${size}px ${SERIF}`;
  }
  ctx.fillText(title, spot.x, spot.y);
  ctx.fillStyle = COLORS.cremeDoux;
  ctx.font = `${hint.size}px ${SANS}`;
  drawBlock(ctx, subtitle, hint);
}

/**
 * Période de la courbe (« SUR 5 ANS · depuis novembre 2021 »), suivie de `note` s'il y en a
 * une (« pointillés : zone euro »), puis la courbe.
 */
function drawChartBlock(
  ctx: CanvasRenderingContext2D,
  layout: Layout,
  series: ChartSeries[],
  display: Display,
  months: number,
  note?: string,
): void {
  const { period, chart } = layout;
  ctx.textAlign = "left";
  ctx.fillStyle = COLORS.cremeDoux;
  ctx.font = `600 ${period.size}px ${SANS}`;
  const firstDate = series
    .flatMap((s) => s.points.slice(0, 1).map((p) => p.date))
    .sort()[0];
  const first = firstDate ? parsePeriod(firstDate) : null;
  const since = first ? new Intl.DateTimeFormat(locale(), { month: "long", year: "numeric" }).format(first) : "";
  const label = t().overYears(Math.max(1, Math.round(months / 12)));
  const notePart = note ? `  ·  ${note}` : "";
  ctx.fillText(`${label.toUpperCase()}${since ? `  ·  ${t().sinceMonth(since)}` : ""}${notePart}`, period.x, period.y);
  drawChartOnCanvas(
    ctx,
    series,
    display,
    months,
    { left: chart.left, top: chart.top, width: chart.width, height: chart.height },
    `${chart.font}px ${SANS}`,
    chart.style,
  );
}

/** Explication puis pied de page : source et marque. */
function drawFooter(ctx: CanvasRenderingContext2D, layout: Layout, explanation: string): void {
  const { explanation: block } = layout;
  ctx.textAlign = "left";
  ctx.fillStyle = COLORS.creme;
  ctx.font = `${block.size}px ${SANS}`;
  drawBlock(ctx, explanation, block);
  drawSourceLine(ctx, layout);
}

/** Bas de page : filet, source (BCE) à gauche, marque à droite. */
function drawSourceLine(ctx: CanvasRenderingContext2D, layout: Layout): void {
  const { margin, width, footer } = layout;
  ctx.textAlign = "left";
  ctx.fillStyle = COLORS.ligne;
  ctx.fillRect(margin, footer.rule, width - 2 * margin, 2);
  ctx.fillStyle = COLORS.cremeDoux;
  ctx.font = `${footer.size}px ${SANS}`;
  ctx.fillText(t().source, margin, footer.y);
  ctx.textAlign = "right";
  ctx.fillStyle = COLORS.or;
  ctx.font = `600 ${footer.brand}px ${SERIF}`;
  ctx.fillText("Glaux", width - margin, footer.y);
}

/**
 * Encodage de l'image : PNG (image seule, net et sans perte) ou JPEG (pages du carrousel :
 * 3 à 5 fois plus léger, et le seul format qu'un PDF peut contenir tel quel).
 */
export type Encoding = "png" | "jpeg";

async function encode(canvas: HTMLCanvasElement, encoding: Encoding): Promise<Uint8Array> {
  const type = encoding === "jpeg" ? "image/jpeg" : "image/png";
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, type, 0.92));
  if (!blob) throw new Error(`conversion en ${encoding.toUpperCase()} impossible`);
  return new Uint8Array(await blob.arrayBuffer());
}

/**
 * Grande valeur en or, puis les détails (variation, date…) sur une ligne, ou une par détail
 * si la ligne dépasse. Renvoie le nombre de lignes de détails.
 */
function drawValueBlock(ctx: CanvasRenderingContext2D, layout: Layout, value: string, details: string[]): number {
  ctx.textAlign = "left";
  ctx.fillStyle = COLORS.or;
  ctx.font = `700 ${layout.value.size}px ${SANS}`;
  ctx.fillText(value, layout.value.x, layout.value.y);
  ctx.fillStyle = COLORS.creme;
  ctx.font = `${layout.change.size}px ${SANS}`;
  const kept = details.filter(Boolean);
  const split = layout.change.lines > 1 && ctx.measureText(kept.join("  ·  ")).width > layout.change.maxWidth;
  const lines = split ? kept : [kept.join("  ·  ")];
  lines.forEach((line, i) => ctx.fillText(line, layout.change.x, layout.change.y + i * layout.change.lineHeight));
  return lines.length;
}

/** Image d'un indicateur : grande valeur, variation, courbe dorée. Renvoie le PNG. */
export async function renderExportImage(
  indicator: IndicatorView,
  display: Display,
  points: Observation[],
  months: number,
  agenda: AgendaData | undefined,
  format: ExportFormat,
  /** Indicateur d'un pays : titre avec le pays, zone euro en pointillés. */
  country?: { title: string; reference: Observation[] },
  encoding: Encoding = "png",
): Promise<Uint8Array> {
  const layout = LAYOUTS[format];
  const { canvas, ctx } = newCanvas(layout);
  drawHeader(ctx, layout);
  drawTitle(ctx, layout, country?.title ?? display.label, display.hint);

  // Valeur, puis variation et date de la donnée (sur deux lignes si besoin).
  const latest = indicator.latest;
  const lineCount = drawValueBlock(ctx, layout, latest ? formatValue(latest.value, display) : "—", [
    formatChange(indicator, display),
    formatDataDate(indicator, display),
  ]);

  // Taux de dépôt et inflation : prochain rendez-vous de l'agenda, en or, sous la variation.
  const agendaLine = exportAgendaLine(indicator.id, agenda);
  if (agendaLine) {
    ctx.fillStyle = COLORS.or;
    ctx.font = `600 ${layout.agenda.size}px ${SANS}`;
    const shift = (lineCount - 1) * layout.change.lineHeight;
    ctx.fillText(agendaLine, layout.agenda.x, layout.agenda.y + shift);
  }

  // Pays : zone euro en pointillés, dessinée d'abord pour rester sous la courbe du pays.
  const series: ChartSeries[] = [{ points, color: COLORS.or }];
  if (country) series.unshift({ points: country.reference, color: COLORS.cremeDoux, dashed: true });
  drawChartBlock(ctx, layout, series, display, months, country ? t().euroDashed : undefined);
  drawFooter(ctx, layout, display.explanation);
  return encode(canvas, encoding);
}

/** Un membre de groupe prêt à dessiner : sa couleur, ses données, sa présentation. */
export type ExportMember = { member: GroupMember; indicator: IndicatorView; display: Display };

/** Une colonne par pays : pastille + nom, grande valeur, variation (portrait, story). */
function drawGroupColumns(ctx: CanvasRenderingContext2D, layout: Layout, members: ExportMember[]): void {
  const { group, margin } = layout;
  const columnWidth = group.width / Math.max(1, members.length);
  // Taille des grandes valeurs : celle prévue, réduite si « 3,42 % » déborde de sa colonne
  // (taux en % plus larges que les écarts en pb). Même taille pour tous les pays.
  const widest = Math.max(
    0,
    ...members.map(({ indicator, display }) => {
      ctx.font = `700 ${group.value}px ${SANS}`;
      const value = ctx.measureText(shortValue(indicator.latest?.value, display)).width;
      ctx.font = `600 ${group.unit}px ${SANS}`;
      return value + group.unit * 0.25 + ctx.measureText(shortUnit(display)).width;
    }),
  );
  const fit = Math.min(1, (columnWidth - 28) / Math.max(1, widest));
  const valueSize = Math.floor(group.value * fit);
  const unitSize = Math.floor(group.unit * fit);
  members.forEach(({ member, indicator, display }, k) => {
    const left = margin + k * columnWidth;
    const dot = group.name * 0.34;
    ctx.fillStyle = member.color;
    ctx.beginPath();
    ctx.arc(left + dot, group.top - group.name * 0.34, dot, 0, Math.PI * 2);
    ctx.fill();
    ctx.textAlign = "left";
    ctx.fillStyle = COLORS.creme;
    ctx.font = `600 ${group.name}px ${SANS}`;
    ctx.fillText(display.label, left + dot * 2 + group.name * 0.4, group.top);

    // Grande valeur sans unité (« 147 », « 3,42 »), puis l'unité en plus petit (« pb », « % »).
    const latest = indicator.latest;
    const value = shortValue(latest?.value, display);
    const valueY = group.top + group.value * 1.04;
    ctx.fillStyle = member.color;
    ctx.font = `700 ${valueSize}px ${SANS}`;
    ctx.fillText(value, left - valueSize * 0.04, valueY);
    if (latest) {
      const valueWidth = ctx.measureText(value).width;
      ctx.font = `600 ${unitSize}px ${SANS}`;
      ctx.fillText(shortUnit(display), left + valueWidth + unitSize * 0.25, valueY);
    }

    ctx.fillStyle = COLORS.cremeDoux;
    ctx.font = `${group.change}px ${SANS}`;
    ctx.fillText(formatChange(indicator, display), left, valueY + group.change * 1.7);
  });
}

/** Une ligne par pays : pastille + nom, valeur, variation courte (paysage). */
function drawGroupRows(ctx: CanvasRenderingContext2D, layout: Layout, members: ExportMember[]): void {
  const { group, margin } = layout;
  const dot = group.name * 0.3;
  const nameX = margin + dot * 2 + 10;
  // Colonnes communes, poussées à droite si un nom (« Allemagne », « Netherlands ») ou une
  // valeur (« 4,89 % ») est plus large que prévu : rien ne se chevauche.
  ctx.font = `600 ${group.name}px ${SANS}`;
  const nameWidth = Math.max(0, ...members.map(({ display }) => ctx.measureText(display.label).width));
  const valueX = Math.max(margin + group.width * 0.36, nameX + nameWidth + 18);
  const valueWidths = members.map(({ indicator, display }) => {
    ctx.font = `700 ${group.value}px ${SANS}`;
    const value = ctx.measureText(shortValue(indicator.latest?.value, display)).width;
    ctx.font = `600 ${group.unit}px ${SANS}`;
    return value + 6 + ctx.measureText(shortUnit(display)).width;
  });
  const changeX = Math.max(margin + group.width * 0.72, valueX + Math.max(0, ...valueWidths) + 22);

  members.forEach(({ member, indicator, display }, k) => {
    const y = group.top + k * group.rowHeight;
    ctx.fillStyle = member.color;
    ctx.beginPath();
    ctx.arc(margin + dot, y - group.name * 0.34, dot, 0, Math.PI * 2);
    ctx.fill();
    ctx.textAlign = "left";
    ctx.fillStyle = COLORS.creme;
    ctx.font = `600 ${group.name}px ${SANS}`;
    ctx.fillText(display.label, nameX, y);

    // Valeur alignée sur une colonne commune, puis l'unité et la variation.
    const latest = indicator.latest;
    const value = shortValue(latest?.value, display);
    ctx.fillStyle = member.color;
    ctx.font = `700 ${group.value}px ${SANS}`;
    ctx.fillText(value, valueX, y + 2);
    const valueWidth = ctx.measureText(value).width;
    if (latest) {
      ctx.font = `600 ${group.unit}px ${SANS}`;
      ctx.fillText(shortUnit(display), valueX + valueWidth + 6, y + 2);
    }
    ctx.fillStyle = COLORS.cremeDoux;
    ctx.font = `${group.change}px ${SANS}`;
    ctx.fillText(shortChange(indicator, display, true), changeX, y);
  });
}

/**
 * Image d'un groupe (taux ou écarts par pays) : une valeur par pays dans la couleur de sa courbe,
 * puis les courbes superposées. Renvoie le PNG.
 */
export async function renderGroupExportImage(
  group: GroupDisplay,
  members: ExportMember[],
  series: ChartSeries[],
  months: number,
  format: ExportFormat,
  encoding: Encoding = "png",
): Promise<Uint8Array> {
  const layout = LAYOUTS[format];
  const { canvas, ctx } = newCanvas(layout);
  drawHeader(ctx, layout);
  drawTitle(ctx, layout, group.label, group.hint);

  if (layout.group.mode === "rows") drawGroupRows(ctx, layout, members);
  else drawGroupColumns(ctx, layout, members);

  // Mois des données (les trois séries sont publiées ensemble).
  const dated = members.find((m) => m.indicator.latest);
  if (dated?.indicator.latest) {
    ctx.textAlign = "left";
    ctx.fillStyle = COLORS.creme;
    ctx.font = `${layout.month.size}px ${SANS}`;
    const month = formatFullPeriod(dated.indicator.latest.date, true);
    ctx.fillText(`${t().monthlyAverages}  ·  ${month}`, layout.month.x, layout.month.y);
  }

  const display = (dated ?? members[0])?.display;
  if (display) drawChartBlock(ctx, layout, series, display, months);
  drawFooter(ctx, layout, group.explanation);
  return encode(canvas, encoding);
}

/**
 * Légende de la courbe des taux, à la place de la période : un point or pour la courbe du jour,
 * un trait en pointillés crème pour celle de comparaison, chacun suivi de sa date.
 */
function drawCurveLegend(
  ctx: CanvasRenderingContext2D,
  layout: Layout,
  items: { color: string; dashed: boolean; text: string }[],
): void {
  const { period } = layout;
  const size = period.size;
  const middle = period.y - size * 0.34;
  let x = period.x;
  ctx.textAlign = "left";
  ctx.font = `600 ${size}px ${SANS}`;
  for (const { color, dashed, text } of items) {
    ctx.fillStyle = color;
    ctx.strokeStyle = color;
    let marker: number;
    if (dashed) {
      marker = size * 1.5;
      ctx.lineWidth = Math.max(2, size * 0.14);
      ctx.setLineDash([size * 0.3, size * 0.25]);
      ctx.beginPath();
      ctx.moveTo(x, middle);
      ctx.lineTo(x + marker, middle);
      ctx.stroke();
      ctx.setLineDash([]);
    } else {
      marker = size * 0.6;
      ctx.beginPath();
      ctx.arc(x + marker / 2, middle, marker / 2, 0, Math.PI * 2);
      ctx.fill();
    }
    x += marker + size * 0.45;
    ctx.fillStyle = COLORS.creme;
    ctx.fillText(text, x, period.y);
    x += ctx.measureText(text).width + size * 1.4;
  }
}

/**
 * Image de la courbe des taux : écart 10 ans − 2 ans en grand, courbe du jour (or) et celle
 * d'il y a `months` mois (pointillés crème), une échéance par graduation. Renvoie le PNG.
 */
export async function renderCurveExportImage(
  curve: CurveView,
  slope: IndicatorView | null,
  months: number,
  format: ExportFormat,
  encoding: Encoding = "png",
): Promise<Uint8Array> {
  const layout = LAYOUTS[format];
  const display = curveDisplay();
  const slopeDisplay = curveSlopeDisplay();
  const { canvas, ctx } = newCanvas(layout);
  drawHeader(ctx, layout);
  drawTitle(ctx, layout, display.label, display.hint);

  // Grande valeur : la pente 2-10 ans (indicateur « slope »), avec sa variation sur 1 mois.
  const hasSlope = slope?.latest && slopeDisplay;
  drawValueBlock(ctx, layout, hasSlope ? formatValue(slope.latest!.value, slopeDisplay) : "—", [
    hasSlope ? `${t().curveSlope}  ·  ${formatChange(slope!, slopeDisplay)}` : "",
    curve.latest ? formatFullPeriod(curve.latest.date, false) : "",
  ]);

  const legend = [
    curve.latest ? { color: COLORS.or, dashed: false, text: formatFullPeriod(curve.latest.date, false) } : null,
    curve.past
      ? { color: COLORS.creme, dashed: true, text: `${formatFullPeriod(curve.past.date, false)} (${t().curveAgo(months)})` }
      : null,
  ].flatMap((item) => (item ? [item] : []));
  drawCurveLegend(ctx, layout, legend);

  const { series, labels } = curveChart(curve);
  const { chart } = layout;
  drawChartOnCanvas(
    ctx,
    series,
    display,
    months,
    { left: chart.left, top: chart.top, width: chart.width, height: chart.height },
    `${chart.font}px ${SANS}`,
    chart.style,
    labels,
  );
  drawFooter(ctx, layout, display.explanation);
  return encode(canvas, encoding);
}

// ---------- Carrousel LinkedIn (PDF) ----------
// Couverture, une page par indicateur affiché (images portrait 4:5), page des sources.
// Les pages sont dessinées ici en JPEG ; Rust les assemble en PDF (src-tauri/src/pdf.rs).

/** Un chiffre clé de la couverture : nom, valeur, date de la donnée. */
export type CoverFigure = { label: string; value: string; date: string };

/** Textes d'une couverture : petit titre en or, grand titre (2 lignes), sous-titre. */
export type CoverTexts = { kicker: string; title: string[]; subtitle: string };

/**
 * Couverture : « La zone euro en chiffres » (ou les textes de `texts`, ex. note macro),
 * 4 chiffres clés, invitation à faire défiler.
 */
export async function renderCarouselCover(figures: CoverFigure[], texts?: CoverTexts): Promise<Uint8Array> {
  const layout = LAYOUTS.portrait;
  const { canvas, ctx } = newCanvas(layout);
  const { margin, width } = layout;
  const strings = t();
  drawHeader(ctx, layout);

  const month = new Intl.DateTimeFormat(locale(), { month: "long", year: "numeric" }).format(new Date());
  const kicker = texts?.kicker ?? strings.coverKicker(month);
  const title = texts?.title ?? strings.coverTitle;
  ctx.textAlign = "left";
  ctx.fillStyle = COLORS.or;
  ctx.font = `600 30px ${SANS}`;
  ctx.fillText(fitText(ctx, kicker.toUpperCase(), width - 2 * margin), margin, 290);
  ctx.fillStyle = COLORS.creme;
  // Grand titre réduit si une ligne dépasse la largeur (« the month in review »).
  let size = 100;
  ctx.font = `600 ${size}px ${SERIF}`;
  while (size > 70 && title.some((line) => ctx.measureText(line).width > width - 2 * margin)) {
    size -= 4;
    ctx.font = `600 ${size}px ${SERIF}`;
  }
  title.forEach((line, i) => ctx.fillText(line, margin - 4, 410 + i * 112));
  ctx.fillStyle = COLORS.cremeDoux;
  ctx.font = `32px ${SANS}`;
  drawBlock(ctx, texts?.subtitle ?? strings.coverSubtitle, {
    x: margin,
    y: 610,
    size: 32,
    maxWidth: width - 2 * margin,
    lineHeight: 44,
    lines: 2,
  });

  // Chiffres clés : 2 colonnes × 2 lignes, dans des cadres.
  const gap = 40;
  const cardWidth = (width - 2 * margin - gap) / 2;
  const cardHeight = 220;
  figures.slice(0, 4).forEach((figure, k) => {
    const x = margin + (k % 2) * (cardWidth + gap);
    const y = 700 + Math.floor(k / 2) * (cardHeight + gap);
    ctx.fillStyle = "#14233a";
    ctx.strokeStyle = COLORS.ligne;
    ctx.lineWidth = 2;
    roundedRect(ctx, x, y, cardWidth, cardHeight, 18);
    ctx.fill();
    ctx.stroke();
    const inner = x + 32;
    ctx.textAlign = "left";
    ctx.fillStyle = COLORS.cremeDoux;
    ctx.font = `28px ${SANS}`;
    ctx.fillText(fitText(ctx, figure.label, cardWidth - 64), inner, y + 56);
    ctx.fillStyle = COLORS.or;
    ctx.font = `700 84px ${SANS}`;
    ctx.fillText(figure.value, inner - 3, y + 148);
    ctx.fillStyle = COLORS.creme;
    ctx.font = `26px ${SANS}`;
    ctx.fillText(fitText(ctx, figure.date, cardWidth - 64), inner, y + 192);
  });

  ctx.textAlign = "right";
  ctx.fillStyle = COLORS.or;
  ctx.font = `600 30px ${SANS}`;
  ctx.fillText(strings.swipe, width - margin, 1222);
  drawSourceLine(ctx, layout);
  return encode(canvas, "jpeg");
}

/** Dernière page : sources et méthode, avertissement, puis la marque Glaux. */
export async function renderCarouselEnd(paragraphs: string[]): Promise<Uint8Array> {
  const layout = LAYOUTS.portrait;
  const { canvas, ctx } = newCanvas(layout);
  const { margin, width } = layout;
  const strings = t();
  drawHeader(ctx, layout);

  ctx.textAlign = "left";
  ctx.fillStyle = COLORS.creme;
  ctx.font = `600 72px ${SERIF}`;
  ctx.fillText(strings.endTitle, margin, 290);

  // Paragraphes l'un sous l'autre, chacun sur autant de lignes que nécessaire.
  let y = 380;
  ctx.font = `30px ${SANS}`;
  for (const paragraph of paragraphs) {
    const lines = wrap(ctx, paragraph, width - 2 * margin, 4);
    ctx.fillStyle = COLORS.creme;
    lines.forEach((line, i) => ctx.fillText(line, margin, y + i * 42));
    y += lines.length * 42 + 30;
  }

  // La marque, centrée, au-dessus du pied de page.
  ctx.textAlign = "center";
  ctx.fillStyle = COLORS.or;
  ctx.font = `600 110px ${SERIF}`;
  ctx.fillText("Glaux", width / 2, 1070);
  ctx.fillStyle = COLORS.creme;
  ctx.font = `34px ${SANS}`;
  ctx.fillText(strings.endTagline, width / 2, 1130);
  ctx.fillStyle = COLORS.cremeDoux;
  ctx.font = `28px ${SANS}`;
  ctx.fillText(strings.endLink, width / 2, 1180);
  drawSourceLine(ctx, layout);
  return encode(canvas, "jpeg");
}

/** Rectangle aux coins arrondis (tracé seulement ; `roundRect` manque aux anciens macOS). */
function roundedRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number): void {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

/** Texte raccourci avec « … » s'il dépasse `maxWidth` (police déjà choisie). */
function fitText(ctx: CanvasRenderingContext2D, text: string, maxWidth: number): string {
  if (ctx.measureText(text).width <= maxWidth) return text;
  let cut = text;
  while (cut.length > 1 && ctx.measureText(`${cut}…`).width > maxWidth) cut = cut.slice(0, -1);
  return `${cut.trimEnd()}…`;
}

/** Envoie des pages JPEG à Rust (`command` : « save_carousel », « save_note »), qui les
 * assemble en PDF dans Images › Glaux. Renvoie le chemin du fichier. */
function savePdfPages(command: string, pages: Uint8Array[]): Promise<string> {
  const total = pages.reduce((sum, page) => sum + page.length, 0);
  const body = new Uint8Array(total);
  let offset = 0;
  for (const page of pages) {
    body.set(page, offset);
    offset += page.length;
  }
  return invoke<string>(command, body, {
    headers: { "x-pages": pages.map((page) => page.length).join(",") },
  });
}

/** Enregistre le carrousel (PDF assemblé par Rust, Images › Glaux). Renvoie le chemin du fichier. */
export function saveCarousel(pages: Uint8Array[]): Promise<string> {
  return savePdfPages("save_carousel", pages);
}

/** Enregistre la note macro (PDF assemblé par Rust, Images › Glaux). Renvoie le chemin du fichier. */
export function saveNotePdf(pages: Uint8Array[]): Promise<string> {
  return savePdfPages("save_note", pages);
}

// ---------- Note macro mensuelle (PDF) ----------
// Pages 4:5 de texte : petit titre, grand titre, chiffre vedette (facultatif), paragraphes.
// La taille du texte baisse de 32 à 24 px pour tout tenir sur une page ; au-delà, la suite
// passe sur une page « (suite) ».

/** Chiffre vedette en haut d'une rubrique : grande valeur en or et sa légende. */
export type NoteHero = { value: string; caption: string };

/** Haut de la zone de texte (sans / avec chiffre vedette) et bas (au-dessus du pied de page). */
const TEXT_TOP = 400;
const TEXT_TOP_HERO = 620;
const TEXT_BOTTOM = LAYOUTS.portrait.footer.rule - 40;
const TEXT_SIZES = [32, 30, 28, 26, 24];

type TextLine = { text: string; first: boolean };

/** Découpe les paragraphes en lignes (police déjà choisie) ; `first` : début de paragraphe. */
function paragraphLines(ctx: CanvasRenderingContext2D, paragraphs: string[], maxWidth: number): TextLine[][] {
  return paragraphs.map((paragraph) =>
    wrap(ctx, paragraph, maxWidth, 1000).map((text, i) => ({ text, first: i === 0 })),
  );
}

/**
 * Pages d'une rubrique (ou de l'analyse) : `kicker` en or, `title`, `hero` sur la première
 * page, `paragraphs` (précédés d'un tiret or si `bullets`).
 */
export async function renderNoteTextPages(
  kicker: string,
  title: string,
  paragraphs: string[],
  options: { hero?: NoteHero; bullets: boolean },
): Promise<Uint8Array[]> {
  const layout = LAYOUTS.portrait;
  const { margin, width } = layout;
  const indent = options.bullets ? 34 : 0;
  const maxWidth = width - 2 * margin - indent;
  const measure = newCanvas(layout).ctx;
  const firstTop = options.hero ? TEXT_TOP_HERO : TEXT_TOP;

  // Plus grande taille où tout tient sur la première page ; sinon 24 px et des pages de suite.
  let size = TEXT_SIZES[TEXT_SIZES.length - 1];
  let blocks: TextLine[][] = [];
  for (const candidate of TEXT_SIZES) {
    measure.font = `${candidate}px ${SANS}`;
    blocks = paragraphLines(measure, paragraphs, maxWidth);
    const lineHeight = Math.round(candidate * 1.42);
    const gap = Math.round(candidate * 0.7);
    const height = blocks.reduce((sum, lines) => sum + lines.length * lineHeight, 0) + gap * (blocks.length - 1);
    size = candidate;
    if (height <= TEXT_BOTTOM - firstTop) break;
  }
  const lineHeight = Math.round(size * 1.42);
  const gap = Math.round(size * 0.7);

  // Répartition des lignes sur les pages.
  const pages: { lines: TextLine[]; gapsBefore: boolean[] }[] = [{ lines: [], gapsBefore: [] }];
  let y = firstTop;
  blocks.forEach((lines, k) => {
    lines.forEach((line, i) => {
      const extra = k > 0 && i === 0 ? gap : 0;
      // Ligne de base au-delà du bas de la zone de texte : page suivante.
      if (y + extra + size > TEXT_BOTTOM && pages[pages.length - 1].lines.length > 0) {
        pages.push({ lines: [], gapsBefore: [] });
        y = TEXT_TOP;
      }
      const page = pages[pages.length - 1];
      const withGap = page.lines.length > 0 && extra > 0;
      page.lines.push(line);
      page.gapsBefore.push(withGap);
      y += (withGap ? gap : 0) + lineHeight;
    });
  });

  const strings = t();
  const images: Uint8Array[] = [];
  for (const [index, page] of pages.entries()) {
    const { canvas, ctx } = newCanvas(layout);
    drawHeader(ctx, layout);
    ctx.textAlign = "left";
    ctx.fillStyle = COLORS.or;
    ctx.font = `600 28px ${SANS}`;
    ctx.fillText(fitText(ctx, kicker.toUpperCase(), width - 2 * margin), margin, 238);
    ctx.fillStyle = COLORS.creme;
    ctx.font = `600 64px ${SERIF}`;
    ctx.fillText(fitText(ctx, index === 0 ? title : strings.noteContinued(title), width - 2 * margin), margin, 318);

    if (index === 0 && options.hero) {
      ctx.fillStyle = COLORS.or;
      ctx.font = `700 120px ${SANS}`;
      ctx.fillText(options.hero.value, margin - 4, 480);
      ctx.fillStyle = COLORS.cremeDoux;
      ctx.font = `28px ${SANS}`;
      ctx.fillText(fitText(ctx, options.hero.caption, width - 2 * margin), margin, 536);
      ctx.fillStyle = COLORS.ligne;
      ctx.fillRect(margin, 576, width - 2 * margin, 2);
    }

    let lineY = (index === 0 ? firstTop : TEXT_TOP) + size;
    ctx.font = `${size}px ${SANS}`;
    page.lines.forEach((line, i) => {
      if (page.gapsBefore[i]) lineY += gap;
      if (options.bullets && line.first) {
        ctx.fillStyle = COLORS.or;
        ctx.fillRect(margin, lineY - size * 0.36, 18, 3);
      }
      ctx.fillStyle = COLORS.creme;
      ctx.fillText(line.text, margin + indent, lineY);
      lineY += lineHeight;
    });
    drawSourceLine(ctx, layout);
    images.push(await encode(canvas, "jpeg"));
  }
  return images;
}

/** Enregistre l'image (Images › Glaux) ; `id` : indicateur ou groupe. Renvoie le chemin du fichier. */
export function savePng(png: Uint8Array, id: string, format: ExportFormat): Promise<string> {
  return invoke<string>("save_png", png, { headers: { "x-indicator": id, "x-format": format } });
}

/** Copie l'image dans le presse-papiers, prête à coller dans un post. */
export function copyPng(png: Uint8Array): Promise<void> {
  return invoke<void>("copy_png", png);
}
