// Image exportée aux couleurs de Glaux (1080 × 1350, format portrait 4:5 :
// bien affiché sur LinkedIn, Instagram et TikTok). Dessinée en canvas, sans bibliothèque.
// Deux mises en page : un indicateur (grande valeur dorée) ou un groupe (une colonne par pays).

import { invoke } from "@tauri-apps/api/core";

import { exportAgendaLine } from "./agenda";
import { COLORS, drawChartOnCanvas, type ChartSeries } from "./chart";
import { formatChange, formatDataDate, formatFullPeriod, formatNumber, formatValue, parsePeriod } from "./format";
import { locale, t } from "./i18n";
import type { Display, GroupDisplay, GroupMember } from "./indicators";
import type { AgendaData, IndicatorView, Observation } from "./types";

const WIDTH = 1080;
const HEIGHT = 1350;
const MARGIN = 80;

const SERIF = `"Iowan Old Style", Georgia, "Times New Roman", serif`;
const SANS = `-apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif`;

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

/** Canvas de l'image, fond compris. */
function newCanvas(): { canvas: HTMLCanvasElement; ctx: CanvasRenderingContext2D } {
  const canvas = document.createElement("canvas");
  canvas.width = WIDTH;
  canvas.height = HEIGHT;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("dessin impossible");
  ctx.fillStyle = COLORS.nuit;
  ctx.fillRect(0, 0, WIDTH, HEIGHT);
  return { canvas, ctx };
}

/** En-tête : marque + date du jour, filet or. */
function drawHeader(ctx: CanvasRenderingContext2D): void {
  ctx.textBaseline = "alphabetic";
  ctx.fillStyle = COLORS.or;
  ctx.font = `600 46px ${SERIF}`;
  ctx.textAlign = "left";
  ctx.fillText("Glaux", MARGIN, 112);
  const brandWidth = ctx.measureText("Glaux").width;
  ctx.fillStyle = COLORS.cremeDoux;
  ctx.font = `28px ${SANS}`;
  ctx.fillText(t().tagline, MARGIN + brandWidth + 18, 110);
  ctx.textAlign = "right";
  ctx.fillText(
    new Intl.DateTimeFormat(locale(), { day: "numeric", month: "long", year: "numeric" }).format(new Date()),
    WIDTH - MARGIN,
    110,
  );
  ctx.fillStyle = COLORS.or;
  ctx.fillRect(MARGIN, 150, WIDTH - 2 * MARGIN, 2);
}

/** Titre et sous-titre (2 lignes au plus). */
function drawTitle(ctx: CanvasRenderingContext2D, title: string, subtitle: string): void {
  ctx.textAlign = "left";
  ctx.fillStyle = COLORS.creme;
  ctx.font = `600 70px ${SERIF}`;
  ctx.fillText(title, MARGIN, 255);
  ctx.fillStyle = COLORS.cremeDoux;
  ctx.font = `28px ${SANS}`;
  wrap(ctx, subtitle, WIDTH - 2 * MARGIN, 2).forEach((line, i) => {
    ctx.fillText(line, MARGIN, 305 + i * 38);
  });
}

/** Période de la courbe (« SUR 5 ANS · depuis novembre 2021 ») puis la courbe. */
function drawChartBlock(ctx: CanvasRenderingContext2D, series: ChartSeries[], display: Display, months: number): void {
  ctx.textAlign = "left";
  ctx.fillStyle = COLORS.cremeDoux;
  ctx.font = `600 28px ${SANS}`;
  const firstDate = series
    .flatMap((s) => s.points.slice(0, 1).map((p) => p.date))
    .sort()[0];
  const first = firstDate ? parsePeriod(firstDate) : null;
  const since = first ? new Intl.DateTimeFormat(locale(), { month: "long", year: "numeric" }).format(first) : "";
  const period = t().overYears(Math.max(1, Math.round(months / 12)));
  ctx.fillText(`${period.toUpperCase()}${since ? `  ·  ${t().sinceMonth(since)}` : ""}`, MARGIN, 692);
  drawChartOnCanvas(
    ctx,
    series,
    display,
    months,
    { left: MARGIN, top: 750, width: WIDTH - 2 * MARGIN, height: 275 },
    `26px ${SANS}`,
  );
}

/** Explication (3 lignes au plus) puis pied de page : source et marque. */
function drawFooter(ctx: CanvasRenderingContext2D, explanation: string): void {
  ctx.textAlign = "left";
  ctx.fillStyle = COLORS.creme;
  ctx.font = `30px ${SANS}`;
  wrap(ctx, explanation, WIDTH - 2 * MARGIN, 3).forEach((line, i) => {
    ctx.fillText(line, MARGIN, 1145 + i * 42);
  });

  ctx.fillStyle = COLORS.ligne;
  ctx.fillRect(MARGIN, HEIGHT - 82, WIDTH - 2 * MARGIN, 2);
  ctx.fillStyle = COLORS.cremeDoux;
  ctx.font = `24px ${SANS}`;
  ctx.fillText(t().source, MARGIN, HEIGHT - 36);
  ctx.textAlign = "right";
  ctx.fillStyle = COLORS.or;
  ctx.font = `600 28px ${SERIF}`;
  ctx.fillText("Glaux", WIDTH - MARGIN, HEIGHT - 36);
}

async function toPng(canvas: HTMLCanvasElement): Promise<Uint8Array> {
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
  if (!blob) throw new Error("conversion en PNG impossible");
  return new Uint8Array(await blob.arrayBuffer());
}

/** Image d'un indicateur : grande valeur, variation, courbe dorée. Renvoie le PNG. */
export async function renderExportImage(
  indicator: IndicatorView,
  display: Display,
  points: Observation[],
  months: number,
  agenda?: AgendaData,
): Promise<Uint8Array> {
  const { canvas, ctx } = newCanvas();
  drawHeader(ctx);
  drawTitle(ctx, display.label, display.hint);

  // Valeur, variation et date de la donnée.
  const latest = indicator.latest;
  ctx.textAlign = "left";
  ctx.fillStyle = COLORS.or;
  ctx.font = `700 150px ${SANS}`;
  ctx.fillText(latest ? formatValue(latest.value, display) : "—", MARGIN - 6, 520);
  ctx.fillStyle = COLORS.creme;
  ctx.font = `34px ${SANS}`;
  const dateText = formatDataDate(indicator, display);
  const change = formatChange(indicator, display);
  ctx.fillText([change, dateText].filter(Boolean).join("  ·  "), MARGIN, 580);

  // Taux de dépôt et inflation : prochain rendez-vous de l'agenda, en or.
  const agendaLine = exportAgendaLine(indicator.id, agenda);
  if (agendaLine) {
    ctx.fillStyle = COLORS.or;
    ctx.font = `600 28px ${SANS}`;
    ctx.fillText(agendaLine, MARGIN, 630);
  }

  drawChartBlock(ctx, [{ points, color: COLORS.or }], display, months);
  drawFooter(ctx, display.explanation);
  return toPng(canvas);
}

/** Un membre de groupe prêt à dessiner : sa couleur, ses données, sa présentation. */
export type ExportMember = { member: GroupMember; indicator: IndicatorView; display: Display };

/**
 * Image d'un groupe (écarts de taux) : une colonne par pays (nom, valeur, variation)
 * dans la couleur de sa courbe, puis les courbes superposées. Renvoie le PNG.
 */
export async function renderGroupExportImage(
  group: GroupDisplay,
  members: ExportMember[],
  series: ChartSeries[],
  months: number,
): Promise<Uint8Array> {
  const { canvas, ctx } = newCanvas();
  drawHeader(ctx);
  drawTitle(ctx, group.label, group.hint);

  const columnWidth = (WIDTH - 2 * MARGIN) / Math.max(1, members.length);
  members.forEach(({ member, indicator, display }, k) => {
    const left = MARGIN + k * columnWidth;
    // Pastille de couleur + nom du pays.
    ctx.fillStyle = member.color;
    ctx.beginPath();
    ctx.arc(left + 11, 409, 11, 0, Math.PI * 2);
    ctx.fill();
    ctx.textAlign = "left";
    ctx.fillStyle = COLORS.creme;
    ctx.font = `600 32px ${SANS}`;
    ctx.fillText(display.label, left + 34, 420);

    // Valeur en points de base : grand nombre, unité plus petite.
    const latest = indicator.latest;
    const value = latest ? formatNumber(latest.value * 100, 0) : "—";
    ctx.fillStyle = member.color;
    ctx.font = `700 104px ${SANS}`;
    ctx.fillText(value, left - 4, 528);
    if (latest) {
      const valueWidth = ctx.measureText(value).width;
      ctx.font = `600 36px ${SANS}`;
      ctx.fillText(t().bpUnit, left + valueWidth + 8, 528);
    }

    // Variation sur 1 mois.
    ctx.fillStyle = COLORS.cremeDoux;
    ctx.font = `28px ${SANS}`;
    ctx.fillText(formatChange(indicator, display), left, 576);
  });

  // Mois des données (les trois séries sont publiées ensemble).
  const dated = members.find((m) => m.indicator.latest);
  if (dated) {
    ctx.fillStyle = COLORS.creme;
    ctx.font = `30px ${SANS}`;
    const month = dated.indicator.latest ? formatFullPeriod(dated.indicator.latest.date, true) : "";
    ctx.fillText(`${t().monthlyAverages}  ·  ${month}`, MARGIN, 630);
  }

  drawChartBlock(ctx, series, (dated ?? members[0]).display, months);
  drawFooter(ctx, group.explanation);
  return toPng(canvas);
}

/** Dessine l'image et l'enregistre (Images › Glaux). Renvoie le chemin du fichier. */
export async function exportIndicator(
  indicator: IndicatorView,
  display: Display,
  points: Observation[],
  months: number,
  agenda?: AgendaData,
): Promise<string> {
  const png = await renderExportImage(indicator, display, points, months, agenda);
  return invoke<string>("save_png", png, { headers: { "x-indicator": indicator.id } });
}

/** Même chose pour un groupe (identifiant du groupe dans le nom du fichier). */
export async function exportGroup(
  groupId: string,
  group: GroupDisplay,
  members: ExportMember[],
  series: ChartSeries[],
  months: number,
): Promise<string> {
  const png = await renderGroupExportImage(group, members, series, months);
  return invoke<string>("save_png", png, { headers: { "x-indicator": groupId } });
}
