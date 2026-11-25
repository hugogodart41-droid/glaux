// Export Excel des historiques (bouton du panneau) : un fichier .xlsx construit à partir du
// cache, sans réseau, avec trois feuilles :
// - « Quotidien » : taux de dépôt, €STR, taux souverain 10 ans, EUR/USD, pente ;
// - « Mensuel » : inflation, chômage, taux réel, taux 10 ans par pays, écarts avec l'Allemagne ;
// - « Sources » : série BCE ou calcul de chaque colonne, unité, date du téléchargement.
// Vraies dates Excel, formats de nombre par unité, ligne d'en-tête figée et filtres.
// Les titres suivent la langue de Glaux.

use std::collections::{BTreeSet, HashMap};

use chrono::{Datelike, NaiveDate};
use rust_xlsxwriter::{
    Color, ExcelDateTime, Format, FormatAlign, FormatBorder, Workbook, Worksheet, XlsxError,
};

use crate::data::{parse_period, values_at_dates};
use crate::i18n::Lang;
use crate::store::ExportSeries;

/// Unité d'une colonne : en %, en points de base (valeurs reçues en points × 100), ou cours.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Unit {
    Percent,
    BasisPoints,
    Rate,
}

struct ColumnDef {
    id: &'static str,
    fr: &'static str,
    en: &'static str,
    unit: Unit,
    /// Décimales affichées dans Excel (la valeur complète reste dans la cellule).
    decimals: usize,
    /// Calcul fait par Glaux (feuille « Sources ») ; vide pour une série BCE.
    formula_fr: &'static str,
    formula_en: &'static str,
}

const fn col(
    id: &'static str,
    fr: &'static str,
    en: &'static str,
    unit: Unit,
    decimals: usize,
) -> ColumnDef {
    ColumnDef {
        id,
        fr,
        en,
        unit,
        decimals,
        formula_fr: "",
        formula_en: "",
    }
}

const fn calc(def: ColumnDef, formula_fr: &'static str, formula_en: &'static str) -> ColumnDef {
    ColumnDef {
        formula_fr,
        formula_en,
        ..def
    }
}

const DAILY: [ColumnDef; 5] = [
    col(
        "deposit_rate",
        "Taux de dépôt BCE (%)",
        "ECB deposit rate (%)",
        Unit::Percent,
        2,
    ),
    col("estr", "€STR (%)", "€STR (%)", Unit::Percent, 3),
    col(
        "yield_10y",
        "Taux souverain 10 ans, zone euro toutes notations (%)",
        "10-year sovereign yield, euro area all ratings (%)",
        Unit::Percent,
        3,
    ),
    col(
        "eurusd",
        "EUR/USD (dollars pour 1 euro)",
        "EUR/USD (dollars per euro)",
        Unit::Rate,
        4,
    ),
    calc(
        col(
            "slope",
            "Pente 2-10 ans AAA (pb)",
            "2-10 year slope, AAA (bp)",
            Unit::BasisPoints,
            1,
        ),
        "Taux AAA à 10 ans − taux AAA à 2 ans (courbe de la BCE), en points de base",
        "AAA 10-year rate − AAA 2-year rate (ECB yield curve), in basis points",
    ),
];

const MONTHLY: [ColumnDef; 23] = [
    col(
        "hicp",
        "Inflation sur 1 an (%)",
        "Inflation, year on year (%)",
        Unit::Percent,
        1,
    ),
    col(
        "core_inflation",
        "Inflation sous-jacente (%)",
        "Core inflation (%)",
        Unit::Percent,
        1,
    ),
    col(
        "services_inflation",
        "Inflation des services (%)",
        "Services inflation (%)",
        Unit::Percent,
        1,
    ),
    col(
        "unemployment",
        "Chômage (%)",
        "Unemployment (%)",
        Unit::Percent,
        1,
    ),
    col(
        "m3",
        "M3, croissance sur 1 an (%)",
        "M3, annual growth (%)",
        Unit::Percent,
        1,
    ),
    col(
        "loans_nfc",
        "Crédit aux entreprises, croissance sur 1 an (%)",
        "Loans to firms, annual growth (%)",
        Unit::Percent,
        1,
    ),
    col(
        "loans_hh",
        "Crédit aux ménages, croissance sur 1 an (%)",
        "Loans to households, annual growth (%)",
        Unit::Percent,
        1,
    ),
    calc(
        col(
            "real_rate",
            "Taux réel (%)",
            "Real rate (%)",
            Unit::Percent,
            2,
        ),
        "Taux de dépôt en vigueur le dernier jour du mois − inflation sur 1 an",
        "Deposit rate in force on the last day of the month − annual inflation",
    ),
    col(
        "yield_de",
        "Taux 10 ans Allemagne (%)",
        "10-year yield, Germany (%)",
        Unit::Percent,
        3,
    ),
    col(
        "yield_fr",
        "Taux 10 ans France (%)",
        "10-year yield, France (%)",
        Unit::Percent,
        3,
    ),
    col(
        "yield_it",
        "Taux 10 ans Italie (%)",
        "10-year yield, Italy (%)",
        Unit::Percent,
        3,
    ),
    col(
        "yield_es",
        "Taux 10 ans Espagne (%)",
        "10-year yield, Spain (%)",
        Unit::Percent,
        3,
    ),
    col(
        "yield_gr",
        "Taux 10 ans Grèce (%)",
        "10-year yield, Greece (%)",
        Unit::Percent,
        3,
    ),
    col(
        "yield_pt",
        "Taux 10 ans Portugal (%)",
        "10-year yield, Portugal (%)",
        Unit::Percent,
        3,
    ),
    col(
        "yield_be",
        "Taux 10 ans Belgique (%)",
        "10-year yield, Belgium (%)",
        Unit::Percent,
        3,
    ),
    col(
        "yield_nl",
        "Taux 10 ans Pays-Bas (%)",
        "10-year yield, Netherlands (%)",
        Unit::Percent,
        3,
    ),
    calc(
        col(
            "spread_fr",
            "Écart France − Allemagne (pb)",
            "Spread France − Germany (bp)",
            Unit::BasisPoints,
            1,
        ),
        "Taux 10 ans France − taux 10 ans Allemagne, en points de base",
        "10-year yield France − Germany, in basis points",
    ),
    calc(
        col(
            "spread_it",
            "Écart Italie − Allemagne (pb)",
            "Spread Italy − Germany (bp)",
            Unit::BasisPoints,
            1,
        ),
        "Taux 10 ans Italie − taux 10 ans Allemagne, en points de base",
        "10-year yield Italy − Germany, in basis points",
    ),
    calc(
        col(
            "spread_es",
            "Écart Espagne − Allemagne (pb)",
            "Spread Spain − Germany (bp)",
            Unit::BasisPoints,
            1,
        ),
        "Taux 10 ans Espagne − taux 10 ans Allemagne, en points de base",
        "10-year yield Spain − Germany, in basis points",
    ),
    calc(
        col(
            "spread_gr",
            "Écart Grèce − Allemagne (pb)",
            "Spread Greece − Germany (bp)",
            Unit::BasisPoints,
            1,
        ),
        "Taux 10 ans Grèce − taux 10 ans Allemagne, en points de base",
        "10-year yield Greece − Germany, in basis points",
    ),
    calc(
        col(
            "spread_pt",
            "Écart Portugal − Allemagne (pb)",
            "Spread Portugal − Germany (bp)",
            Unit::BasisPoints,
            1,
        ),
        "Taux 10 ans Portugal − taux 10 ans Allemagne, en points de base",
        "10-year yield Portugal − Germany, in basis points",
    ),
    calc(
        col(
            "spread_be",
            "Écart Belgique − Allemagne (pb)",
            "Spread Belgium − Germany (bp)",
            Unit::BasisPoints,
            1,
        ),
        "Taux 10 ans Belgique − taux 10 ans Allemagne, en points de base",
        "10-year yield Belgium − Germany, in basis points",
    ),
    calc(
        col(
            "spread_nl",
            "Écart Pays-Bas − Allemagne (pb)",
            "Spread Netherlands − Germany (bp)",
            Unit::BasisPoints,
            1,
        ),
        "Taux 10 ans Pays-Bas − taux 10 ans Allemagne, en points de base",
        "10-year yield Netherlands − Germany, in basis points",
    ),
];

/// Une feuille de données : dates (lignes) × colonnes, valeurs dans l'unité affichée.
#[derive(Debug, PartialEq)]
struct Table {
    dates: Vec<NaiveDate>,
    /// `values[c][r]` : colonne c, ligne r (None : pas de valeur ce jour-là ou ce mois-là).
    values: Vec<Vec<Option<f64>>>,
}

/// Réunit les dates de toutes les colonnes (ordre chronologique) et place chaque valeur.
/// Un taux en escalier (taux de dépôt) prend la valeur en vigueur à chaque date ;
/// les autres séries n'ont une valeur qu'à leurs propres dates.
fn build_table(defs: &[ColumnDef], series: &[ExportSeries]) -> Table {
    let found: Vec<Option<&ExportSeries>> = defs
        .iter()
        .map(|def| series.iter().find(|s| s.id == def.id))
        .collect();
    let dates: Vec<NaiveDate> = found
        .iter()
        .flatten()
        .flat_map(|s| s.observations.iter().filter_map(|o| parse_period(&o.date)))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let values = defs
        .iter()
        .zip(&found)
        .map(|(def, series)| {
            let Some(series) = series else {
                return vec![None; dates.len()];
            };
            let raw = if series.step {
                values_at_dates(&series.observations, &dates)
            } else {
                let by_date: HashMap<NaiveDate, f64> = series
                    .observations
                    .iter()
                    .filter_map(|o| Some((parse_period(&o.date)?, o.value)))
                    .collect();
                dates.iter().map(|d| by_date.get(d).copied()).collect()
            };
            raw.into_iter()
                .map(|v| v.map(|v| scale(v, def.unit)))
                .collect()
        })
        .collect();
    Table { dates, values }
}

/// Points de base : la valeur arrive en points de pourcentage (1,474 → 147,4 pb).
fn scale(value: f64, unit: Unit) -> f64 {
    match unit {
        Unit::BasisPoints => (value * 1000.0).round() / 10.0,
        _ => value,
    }
}

fn excel_date(date: NaiveDate) -> Result<ExcelDateTime, XlsxError> {
    ExcelDateTime::from_ymd(date.year() as u16, date.month() as u8, date.day() as u8)
}

fn number_format(decimals: usize) -> Format {
    let pattern = if decimals == 0 {
        "0".to_string()
    } else {
        format!("0.{}", "0".repeat(decimals))
    };
    Format::new().set_num_format(pattern)
}

/// En-tête aux couleurs de Glaux : texte crème sur fond nuit, filet or.
fn header_format() -> Format {
    Format::new()
        .set_bold()
        .set_font_color(Color::RGB(0xF5F1E8))
        .set_background_color(Color::RGB(0x0E1A2B))
        .set_border_bottom(FormatBorder::Medium)
        .set_border_bottom_color(Color::RGB(0xC9A24A))
        .set_text_wrap()
        .set_align(FormatAlign::VerticalCenter)
}

/// Écrit une feuille de données (dates en colonne A, une colonne par série).
fn write_table(
    sheet: &mut Worksheet,
    defs: &[ColumnDef],
    table: &Table,
    lang: Lang,
    monthly: bool,
) -> Result<(), XlsxError> {
    let header = header_format();
    let date_format = Format::new().set_num_format(if monthly { "mmm yyyy" } else { "dd/mm/yyyy" });

    sheet.write_string_with_format(0, 0, lang.pick("Date", "Date"), &header)?;
    for (c, def) in defs.iter().enumerate() {
        let col = (c + 1) as u16;
        sheet.write_string_with_format(0, col, lang.pick(def.fr, def.en), &header)?;
        sheet.set_column_width(col, 17)?;
    }
    sheet.set_column_width(0, 12)?;
    sheet.set_row_height(0, 46)?;

    let formats: Vec<Format> = defs.iter().map(|d| number_format(d.decimals)).collect();
    for (r, date) in table.dates.iter().enumerate() {
        let row = (r + 1) as u32;
        sheet.write_datetime_with_format(row, 0, excel_date(*date)?, &date_format)?;
        for (c, column) in table.values.iter().enumerate() {
            if let Some(value) = column[r] {
                sheet.write_number_with_format(row, (c + 1) as u16, value, &formats[c])?;
            }
        }
    }

    sheet.set_freeze_panes(1, 1)?;
    sheet.autofilter(0, 0, table.dates.len() as u32, defs.len() as u16)?;
    Ok(())
}

/// Feuille « Sources » : d'où vient chaque colonne.
fn write_sources(
    sheet: &mut Worksheet,
    series: &[ExportSeries],
    lang: Lang,
    downloaded: &str,
    version: &str,
) -> Result<(), XlsxError> {
    let bold = Format::new().set_bold();
    let header = header_format();
    sheet.write_string_with_format(0, 0, format!("Glaux {version}"), &bold)?;
    sheet.write_string(
        1,
        0,
        lang.pick(
            "Source : Banque centrale européenne, ECB Data Portal (data.ecb.europa.eu).",
            "Source: European Central Bank, ECB Data Portal (data.ecb.europa.eu).",
        ),
    )?;
    sheet.write_string(
        2,
        0,
        format!(
            "{} {downloaded}",
            lang.pick("Données téléchargées le", "Data downloaded on")
        ),
    )?;

    let titles = lang.pick(
        ["Feuille", "Colonne", "Série BCE ou calcul", "Unité"],
        ["Sheet", "Column", "ECB series or calculation", "Unit"],
    );
    for (c, title) in titles.iter().enumerate() {
        sheet.write_string_with_format(4, c as u16, *title, &header)?;
    }
    let sheets = [
        (lang.pick("Quotidien", "Daily"), &DAILY[..]),
        (lang.pick("Mensuel", "Monthly"), &MONTHLY[..]),
        (
            lang.pick("Courbe des taux AAA", "AAA yield curve"),
            &CURVE_COLUMNS[..],
        ),
    ];
    let mut row = 5;
    for (sheet_name, defs) in sheets {
        for def in defs {
            let origin = if def.formula_fr.is_empty() {
                series
                    .iter()
                    .find(|s| s.id == def.id)
                    .and_then(|s| s.key.clone())
                    .unwrap_or_default()
            } else {
                lang.pick(def.formula_fr, def.formula_en).to_string()
            };
            let unit = match def.unit {
                Unit::Percent => "%",
                Unit::BasisPoints => lang.pick("points de base", "basis points"),
                Unit::Rate => lang.pick("dollars pour 1 euro", "dollars per euro"),
            };
            sheet.write_string(row, 0, sheet_name)?;
            sheet.write_string(row, 1, lang.pick(def.fr, def.en))?;
            sheet.write_string(row, 2, origin)?;
            sheet.write_string(row, 3, unit)?;
            row += 1;
        }
    }
    sheet.set_column_width(0, 12)?;
    sheet.set_column_width(1, 38)?;
    sheet.set_column_width(2, 70)?;
    sheet.set_column_width(3, 20)?;
    Ok(())
}

/// Feuille « Courbe des taux AAA » : taux zéro coupon des États notés AAA par échéance (quotidien),
/// dans l'ordre de `CURVE` (store.rs).
const CURVE_COLUMNS: [ColumnDef; 11] = [
    col("yc_3m", "3 mois (%)", "3 months (%)", Unit::Percent, 3),
    col("yc_6m", "6 mois (%)", "6 months (%)", Unit::Percent, 3),
    col("yc_1y", "1 an (%)", "1 year (%)", Unit::Percent, 3),
    col("yc_2y", "2 ans (%)", "2 years (%)", Unit::Percent, 3),
    col("yc_3y", "3 ans (%)", "3 years (%)", Unit::Percent, 3),
    col("yc_5y", "5 ans (%)", "5 years (%)", Unit::Percent, 3),
    col("yc_7y", "7 ans (%)", "7 years (%)", Unit::Percent, 3),
    col("yc_10y", "10 ans (%)", "10 years (%)", Unit::Percent, 3),
    col("yc_15y", "15 ans (%)", "15 years (%)", Unit::Percent, 3),
    col("yc_20y", "20 ans (%)", "20 years (%)", Unit::Percent, 3),
    col("yc_30y", "30 ans (%)", "30 years (%)", Unit::Percent, 3),
];

/// Construit le classeur et renvoie le fichier .xlsx (en mémoire).
/// `downloaded` : date et heure du dernier téléchargement, déjà mises en forme.
pub fn build_workbook(
    series: &[ExportSeries],
    lang: Lang,
    downloaded: &str,
    version: &str,
) -> Result<Vec<u8>, String> {
    let build = || -> Result<Vec<u8>, XlsxError> {
        let mut workbook = Workbook::new();
        let daily = build_table(&DAILY, series);
        let monthly = build_table(&MONTHLY, series);
        let curve = build_table(&CURVE_COLUMNS, series);

        let sheet = workbook.add_worksheet();
        sheet.set_name(lang.pick("Quotidien", "Daily"))?;
        write_table(sheet, &DAILY, &daily, lang, false)?;

        let sheet = workbook.add_worksheet();
        sheet.set_name(lang.pick("Mensuel", "Monthly"))?;
        write_table(sheet, &MONTHLY, &monthly, lang, true)?;

        let sheet = workbook.add_worksheet();
        sheet.set_name(lang.pick("Courbe des taux AAA", "AAA yield curve"))?;
        write_table(sheet, &CURVE_COLUMNS, &curve, lang, false)?;

        let sheet = workbook.add_worksheet();
        sheet.set_name("Sources")?;
        write_sources(sheet, series, lang, downloaded, version)?;

        workbook.save_to_buffer()
    };
    build().map_err(|e| format!("création du fichier Excel impossible : {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Observation;

    fn series(id: &'static str, step: bool, data: &[(&str, f64)]) -> ExportSeries {
        ExportSeries {
            id,
            observations: data
                .iter()
                .map(|(date, value)| Observation {
                    date: date.to_string(),
                    value: *value,
                })
                .collect(),
            step,
            key: Some(format!("TEST.{id}")),
        }
    }

    fn day(s: &str) -> NaiveDate {
        parse_period(s).unwrap()
    }

    #[test]
    fn dates_reunies_et_taux_de_depot_en_vigueur() {
        let data = [
            series("deposit_rate", true, &[("2026-09-16", 2.5)]),
            series(
                "estr",
                false,
                &[("2026-11-20", 2.443), ("2026-11-23", 2.441)],
            ),
            series("slope", false, &[("2026-11-23", 1.487)]),
        ];
        let table = build_table(&DAILY, &data);
        assert_eq!(
            table.dates,
            vec![day("2026-09-16"), day("2026-11-20"), day("2026-11-23")]
        );
        // Taux de dépôt : valeur en vigueur reportée sur chaque date.
        assert_eq!(table.values[0], vec![Some(2.5), Some(2.5), Some(2.5)]);
        // €STR : seulement à ses propres dates.
        assert_eq!(table.values[1], vec![None, Some(2.443), Some(2.441)]);
        // Série absente du cache : colonne vide.
        assert_eq!(table.values[3], vec![None, None, None]);
        // Pente en points de base.
        assert_eq!(table.values[4], vec![None, None, Some(148.7)]);
    }

    #[test]
    fn mensuel_avec_ecarts_en_points_de_base() {
        let data = [
            series("hicp", false, &[("2026-09", 3.8), ("2026-10", 4.0)]),
            series("spread_fr", false, &[("2026-10", 1.474)]),
        ];
        let table = build_table(&MONTHLY, &data);
        assert_eq!(table.dates, vec![day("2026-09"), day("2026-10")]);
        assert_eq!(table.values[0], vec![Some(3.8), Some(4.0)]);
        let fr = MONTHLY.iter().position(|c| c.id == "spread_fr").unwrap();
        assert_eq!(table.values[fr], vec![None, Some(147.4)]);
    }

    #[test]
    fn classeur_xlsx_valide_dans_les_deux_langues() {
        let data = [
            series("deposit_rate", true, &[("2026-09-16", 2.5)]),
            series("hicp", false, &[("2026-10", 4.0)]),
        ];
        for lang in [Lang::Fr, Lang::En] {
            let bytes = build_workbook(&data, lang, "25/11/2026 14:58", "0.2.0").unwrap();
            assert!(bytes.starts_with(b"PK")); // un .xlsx est une archive zip
        }
        // Classeur vide (premier lancement hors ligne) : fichier valide quand même.
        assert!(build_workbook(&[], Lang::Fr, "—", "0.2.0").is_ok());
    }
}
