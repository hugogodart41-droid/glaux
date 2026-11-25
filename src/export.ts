// Image exportée aux couleurs de Glaux (1080 × 1350, format portrait 4:5 :
// bien affiché sur LinkedIn, Instagram et TikTok). Dessinée en canvas, sans bibliothèque.

import { invoke } from "@tauri-apps/api/core";

import { exportAgendaLine } from "./agenda";
import { COLORS, drawChartOnCanvas } from "./chart";
import { formatChange, formatDataDate, formatValue, parsePeriod } from "./format";
import { locale, t } from "./i18n";
import type { Display } from "./indicators";
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

/** Dessine l'image et renvoie le PNG. */
export async function renderExportImage(
  indicator: IndicatorView,
  display: Display,
  points: Observation[],
  months: number,
  agenda?: AgendaData,
): Promise<Uint8Array> {
  const canvas = document.createElement("canvas");
  canvas.width = WIDTH;
  canvas.height = HEIGHT;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("dessin impossible");

  // Fond.
  ctx.fillStyle = COLORS.nuit;
  ctx.fillRect(0, 0, WIDTH, HEIGHT);

  // En-tête : marque + date du jour.
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

  // Titre et sous-titre.
  ctx.textAlign = "left";
  ctx.fillStyle = COLORS.creme;
  ctx.font = `600 70px ${SERIF}`;
  ctx.fillText(display.label, MARGIN, 255);
  ctx.fillStyle = COLORS.cremeDoux;
  ctx.font = `28px ${SANS}`;
  wrap(ctx, display.hint, WIDTH - 2 * MARGIN, 2).forEach((line, i) => {
    ctx.fillText(line, MARGIN, 305 + i * 38);
  });

  // Valeur, variation et date de la donnée.
  const latest = indicator.latest;
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

  // Courbe.
  ctx.fillStyle = COLORS.cremeDoux;
  ctx.font = `600 28px ${SANS}`;
  const first = points.length ? parsePeriod(points[0].date) : null;
  const since = first ? new Intl.DateTimeFormat(locale(), { month: "long", year: "numeric" }).format(first) : "";
  const period = t().overYears(Math.max(1, Math.round(months / 12)));
  ctx.fillText(`${period.toUpperCase()}${since ? `  ·  ${t().sinceMonth(since)}` : ""}`, MARGIN, 692);
  drawChartOnCanvas(
    ctx,
    points,
    display,
    months,
    { left: MARGIN, top: 750, width: WIDTH - 2 * MARGIN, height: 275 },
    `26px ${SANS}`,
  );

  // Explication (3 lignes au plus, au-dessus du pied de page).
  ctx.fillStyle = COLORS.creme;
  ctx.font = `30px ${SANS}`;
  wrap(ctx, display.explanation, WIDTH - 2 * MARGIN, 3).forEach((line, i) => {
    ctx.fillText(line, MARGIN, 1145 + i * 42);
  });

  // Pied : source et marque.
  ctx.fillStyle = COLORS.ligne;
  ctx.fillRect(MARGIN, HEIGHT - 82, WIDTH - 2 * MARGIN, 2);
  ctx.fillStyle = COLORS.cremeDoux;
  ctx.font = `24px ${SANS}`;
  ctx.fillText(t().source, MARGIN, HEIGHT - 36);
  ctx.textAlign = "right";
  ctx.fillStyle = COLORS.or;
  ctx.font = `600 28px ${SERIF}`;
  ctx.fillText("Glaux", WIDTH - MARGIN, HEIGHT - 36);

  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
  if (!blob) throw new Error("conversion en PNG impossible");
  return new Uint8Array(await blob.arrayBuffer());
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
