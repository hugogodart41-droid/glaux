// Note macro mensuelle : les faits du mois, tirés uniquement des données (séries BCE et
// indicateurs calculés par Glaux), écrits en phrases toutes faites, en français ou en anglais.
//
// Aucune interprétation ici : chaque phrase dit une valeur, sa date et sa variation. L'analyse
// est écrite par l'utilisateur (zone « Mon analyse », enregistrée par store.rs) et placée en
// tête du texte du post.
//
// Quatre rubriques : Prix ; Taux et politique monétaire ; Emploi, crédit et monnaie ; Marchés.
// Une donnée absente (premier lancement, série en échec) supprime sa phrase, rien d'autre.

use chrono::{Datelike, Months, NaiveDate};
use serde::Serialize;

use crate::agenda::Calendar;
use crate::data::{parse_period, Observation};
use crate::ecb::COUNTRIES;
use crate::i18n::{fr_of_month, month_name, number, Lang};

/// Longueur maximale de l'analyse enregistrée (caractères).
pub const MAX_ANALYSIS_CHARS: usize = 5000;

/// Limite d'un post LinkedIn (caractères), affichée par l'interface.
pub const POST_LIMIT: usize = 3000;

/// Un chiffre clé : carte de la page PDF et puce du post.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NoteFigure {
    pub label: String,
    /// Valeur mise en forme (« 2,50 % », « 147 pb », « 1,0935 »).
    pub value: String,
    /// Date de la donnée (« octobre 2026 », « 24 novembre 2026 », « depuis le 11 juin 2025 »).
    pub date: String,
    /// Variation (« +0,2 pt en un mois »), absente si inconnue.
    pub change: Option<String>,
}

/// Une rubrique de la note.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NoteSection {
    /// « prices », « rates », « activity », « markets ».
    pub id: &'static str,
    pub title: String,
    pub figures: Vec<NoteFigure>,
    pub sentences: Vec<String>,
    /// Lignes en plus dans le post, après les puces (prochaines dates).
    #[serde(skip)]
    pub post_lines: Vec<String>,
}

/// La note du mois, envoyée à l'interface.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteView {
    /// Mois de la note (« 2026-12 ») : clé de l'analyse enregistrée.
    pub month: String,
    /// « Note macro · décembre 2026 ».
    pub title: String,
    /// Chiffres de la couverture : taux de dépôt, inflation, chômage, taux 10 ans.
    pub cover: Vec<NoteFigure>,
    pub sections: Vec<NoteSection>,
    pub analysis: String,
    /// Texte du post LinkedIn : titre, analyse, chiffres, sources.
    pub post: String,
    pub post_limit: usize,
}

// ---------- Mise en forme ----------

/// « 2,50 % » (espace insécable) / « 2.50% ».
fn pct(value: f64, decimals: usize, signed: bool, lang: Lang) -> String {
    let n = number(value, decimals, signed, lang);
    lang.pick(format!("{n}\u{a0}%"), format!("{n}%"))
}

/// Écart exprimé en points de pourcentage → « 147 pb » / « 147 bp ».
fn bp(points: f64, signed: bool, lang: Lang) -> String {
    let n = number(points * 100.0, 0, signed, lang);
    lang.pick(format!("{n}\u{a0}pb"), format!("{n}\u{a0}bp"))
}

/// Variation en points de pourcentage : « +0,2 pt » / « +0.2 pp ».
fn pt(diff: f64, decimals: usize, lang: Lang) -> String {
    let n = number(diff, decimals, true, lang);
    lang.pick(format!("{n}\u{a0}pt"), format!("{n}\u{a0}pp"))
}

/// Valeur arrondie à la précision affichée : deux valeurs affichées pareil sont « stables ».
fn rounded(value: f64, decimals: usize) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    (value * factor).round() / factor
}

/// « octobre 2026 » / « October 2026 ».
fn month_year(date: NaiveDate, lang: Lang) -> String {
    format!("{} {}", month_name(date.month(), lang), date.year())
}

/// « octobre », ou « décembre 2025 » si l'année diffère de `reference`.
fn month_near(date: NaiveDate, reference: NaiveDate, lang: Lang) -> String {
    if date.year() == reference.year() {
        month_name(date.month(), lang).to_string()
    } else {
        month_year(date, lang)
    }
}

/// « 24 novembre 2026 », « 1er décembre 2026 » / « 24 November 2026 » (jour et mois insécables).
fn full_date(date: NaiveDate, lang: Lang) -> String {
    let day = if lang == Lang::Fr && date.day() == 1 {
        "1er".to_string()
    } else {
        date.day().to_string()
    };
    // Espace insécable : « 1er » et « décembre » restent sur la même ligne.
    format!("{day}\u{a0}{}", month_year(date, lang))
}

/// Nombre de mois entre deux dates (mois calendaires).
fn months_between(from: NaiveDate, to: NaiveDate) -> i32 {
    (to.year() - from.year()) * 12 + to.month() as i32 - from.month() as i32
}

/// « en un mois » / « over one month », après une variation.
fn over_month(change: String, lang: Lang) -> String {
    lang.pick(
        format!("{change} en un mois"),
        format!("{change} over one month"),
    )
}

/// Dernière observation et sa date.
fn last_dated(observations: &[Observation]) -> Option<(NaiveDate, f64)> {
    let last = observations.last()?;
    Some((parse_period(&last.date)?, last.value))
}

/// Série mensuelle : dernière valeur, et valeur du mois juste avant (absente si ce mois manque).
fn last_two_months(observations: &[Observation]) -> Option<(NaiveDate, f64, Option<f64>)> {
    let (date, value) = last_dated(observations)?;
    let previous = observations
        .iter()
        .rev()
        .nth(1)
        .filter(|o| parse_period(&o.date) == date.checked_sub_months(Months::new(1)))
        .map(|o| o.value);
    Some((date, value, previous))
}

/// Série quotidienne : dernière valeur et valeur en vigueur un mois plus tôt.
fn last_and_month_ago(observations: &[Observation]) -> Option<(NaiveDate, f64, Option<f64>)> {
    let (date, value) = last_dated(observations)?;
    let month_ago = date.checked_sub_months(Months::new(1))?;
    let previous = observations
        .iter()
        .rev()
        .find(|o| parse_period(&o.date).is_some_and(|d| d <= month_ago))
        .map(|o| o.value);
    Some((date, value, previous))
}

/// « , son plus haut niveau depuis mars 2024 » si la dernière valeur n'avait pas été atteinte
/// depuis au moins 12 mois (`up` : plus haut ; sinon plus bas). Sans valeur aussi haute dans
/// tout l'historique (au moins 2 ans) : « depuis au moins novembre 2021 ».
fn extreme_note(
    observations: &[Observation],
    decimals: usize,
    up: bool,
    lang: Lang,
) -> Option<String> {
    let (last, earlier) = observations.split_last()?;
    let date = parse_period(&last.date)?;
    let target = rounded(last.value, decimals);
    let reached = earlier.iter().rev().find(|o| {
        let value = rounded(o.value, decimals);
        if up {
            value >= target
        } else {
            value <= target
        }
    });
    let (since, at_least) = match reached {
        Some(o) => (parse_period(&o.date)?, false),
        None => (parse_period(&earlier.first()?.date)?, true),
    };
    let gap = months_between(since, date);
    if gap < if at_least { 24 } else { 12 } {
        return None;
    }
    let when = month_year(since, lang);
    let (fr, en) = if up {
        ("haut", "highest")
    } else {
        ("bas", "lowest")
    };
    Some(match (lang, at_least) {
        (Lang::Fr, false) => format!(", son plus {fr} niveau depuis {when}"),
        (Lang::Fr, true) => format!(", son plus {fr} niveau depuis au moins {when}"),
        (Lang::En, false) => format!(", its {en} level since {when}"),
        (Lang::En, true) => format!(", its {en} level since at least {when}"),
    })
}

// ---------- Séries mensuelles (même phrase type) ----------

/// Textes d'une série mensuelle : (français, anglais).
struct Monthly {
    label: (&'static str, &'static str),
    /// Sujet de la phrase : « L'inflation de la zone euro (sur 1 an) ».
    subject: (&'static str, &'static str),
    up: (&'static str, &'static str),
    down: (&'static str, &'static str),
    /// Accordé avec le sujet : « est restée stable à » / « est resté stable à ».
    flat: (&'static str, &'static str),
    decimals: usize,
    /// Croissance : signe « + » affiché.
    signed: bool,
    /// Signaler un plus haut / plus bas depuis au moins 1 an.
    extremes: bool,
}

const HICP: Monthly = Monthly {
    label: ("Inflation (sur 1 an)", "Inflation (y/y)"),
    subject: (
        "L'inflation de la zone euro (sur 1 an)",
        "Euro area inflation (y/y)",
    ),
    up: ("a accéléré à", "rose to"),
    down: ("a ralenti à", "eased to"),
    flat: ("est restée stable à", "was unchanged at"),
    decimals: 1,
    signed: false,
    extremes: true,
};

const CORE: Monthly = Monthly {
    label: ("Inflation sous-jacente", "Core inflation"),
    subject: (
        "L'inflation sous-jacente (hors énergie, alimentation, alcool et tabac)",
        "Core inflation (excluding energy, food, alcohol and tobacco)",
    ),
    ..HICP
};

const SERVICES: Monthly = Monthly {
    label: ("Inflation des services", "Services inflation"),
    subject: ("L'inflation des services", "Services inflation"),
    ..HICP
};

const UNEMPLOYMENT: Monthly = Monthly {
    label: ("Chômage", "Unemployment"),
    subject: (
        "Le taux de chômage de la zone euro",
        "The euro area unemployment rate",
    ),
    up: ("est monté à", "rose to"),
    down: ("est descendu à", "fell to"),
    flat: ("est resté stable à", "was unchanged at"),
    decimals: 1,
    signed: false,
    extremes: true,
};

const REAL_RATE: Monthly = Monthly {
    label: ("Taux directeur réel", "Real policy rate"),
    subject: (
        "Le taux directeur réel (taux de dépôt − inflation, calcul Glaux)",
        "The real policy rate (deposit rate − inflation, Glaux calculation)",
    ),
    up: ("est remonté à", "rose to"),
    down: ("est descendu à", "fell to"),
    flat: ("est resté stable à", "was unchanged at"),
    decimals: 2,
    signed: false,
    extremes: false,
};

const M3: Monthly = Monthly {
    label: ("M3 (sur 1 an)", "M3 (y/y)"),
    subject: (
        "La croissance de la masse monétaire M3 sur 1 an",
        "Annual M3 money supply growth",
    ),
    up: ("a accéléré à", "picked up to"),
    down: ("a ralenti à", "slowed to"),
    flat: ("est restée stable à", "was unchanged at"),
    decimals: 1,
    signed: true,
    extremes: false,
};

const LOANS_NFC: Monthly = Monthly {
    label: ("Crédit aux entreprises (sur 1 an)", "Loans to firms (y/y)"),
    subject: (
        "La croissance sur 1 an du crédit aux entreprises",
        "Annual growth in loans to firms",
    ),
    ..M3
};

const LOANS_HH: Monthly = Monthly {
    label: ("Crédit aux ménages (sur 1 an)", "Loans to households (y/y)"),
    subject: (
        "La croissance sur 1 an du crédit aux ménages",
        "Annual growth in loans to households",
    ),
    ..M3
};

/// Chiffre et phrase d'une série mensuelle.
fn monthly_fact(
    observations: &[Observation],
    def: &Monthly,
    lang: Lang,
) -> Option<(NoteFigure, String)> {
    let (date, value, previous) = last_two_months(observations)?;
    let d = def.decimals;
    let shown = pct(value, d, def.signed, lang);
    let when = month_year(date, lang);
    let subject = lang.pick(def.subject.0, def.subject.1);
    let diff = previous.map(|p| rounded(value, d) - rounded(p, d));

    let sentence = match (previous, diff) {
        (Some(p), Some(diff)) => {
            let before = pct(p, d, def.signed, lang);
            let previous_month = date
                .checked_sub_months(Months::new(1))
                .map(|m| month_near(m, date, lang))
                .unwrap_or_default();
            if diff.abs() < 1e-9 {
                let flat = lang.pick(def.flat.0, def.flat.1);
                lang.pick(
                    format!("{subject} {flat} {shown} en {when}."),
                    format!("{subject} {flat} {shown} in {when}."),
                )
            } else {
                let up = diff > 0.0;
                let verb = if up {
                    lang.pick(def.up.0, def.up.1)
                } else {
                    lang.pick(def.down.0, def.down.1)
                };
                let extreme = def
                    .extremes
                    .then(|| extreme_note(observations, d, up, lang))
                    .flatten()
                    .unwrap_or_default();
                lang.pick(
                    format!("{subject} {verb} {shown} en {when}, après {before} en {previous_month}{extreme}."),
                    format!("{subject} {verb} {shown} in {when}, from {before} in {previous_month}{extreme}."),
                )
            }
        }
        _ => lang.pick(
            format!("{subject} s'établit à {shown} en {when}."),
            format!("{subject} stood at {shown} in {when}."),
        ),
    };

    let figure = NoteFigure {
        label: lang.pick(def.label.0, def.label.1).to_string(),
        value: shown,
        date: when,
        change: diff.map(|diff| {
            if diff.abs() < 1e-9 {
                lang.pick("stable sur un mois", "unchanged over one month")
                    .to_string()
            } else {
                over_month(pt(diff, d, lang), lang)
            }
        }),
    };
    Some((figure, sentence))
}

// ---------- Séries quotidiennes et cas particuliers ----------

/// Taux de dépôt : niveau, date d'entrée en vigueur, changement récent (moins de 60 jours).
fn deposit_fact(
    observations: &[Observation],
    today: NaiveDate,
    lang: Lang,
) -> Option<(NoteFigure, String)> {
    let latest = observations.last()?;
    // Début du palier actuel, puis la valeur d'avant ce palier.
    let same = observations
        .iter()
        .rev()
        .take_while(|o| (o.value - latest.value).abs() < 1e-9)
        .count();
    let start = &observations[observations.len() - same];
    let since = parse_period(&start.date)?;
    let before = observations
        .len()
        .checked_sub(same + 1)
        .map(|i| observations[i].value);
    let shown = pct(latest.value, 2, false, lang);
    let since_text = full_date(since, lang);
    let recent = (today - since).num_days() <= 60;

    let (sentence, change) = match before.filter(|_| recent) {
        Some(before) => {
            let diff = latest.value - before;
            let verb = match (lang, diff > 0.0) {
                (Lang::Fr, true) => "relevé",
                (Lang::Fr, false) => "abaissé",
                (Lang::En, true) => "raised",
                (Lang::En, false) => "cut",
            };
            let size = bp(diff.abs(), false, lang);
            (
                lang.pick(
                    format!("La BCE a {verb} son taux de dépôt de {size}, à {shown}, avec effet le {since_text}."),
                    format!("The ECB {verb} its deposit rate by {size} to {shown}, effective {since_text}."),
                ),
                Some(bp(diff, true, lang)),
            )
        }
        None => (
            lang.pick(
                format!(
                    "Le taux de dépôt de la BCE est de {shown}, inchangé depuis le {since_text}."
                ),
                format!("The ECB deposit rate is {shown}, unchanged since {since_text}."),
            ),
            None,
        ),
    };
    let figure = NoteFigure {
        label: lang
            .pick("Taux de dépôt BCE", "ECB deposit rate")
            .to_string(),
        value: shown,
        date: lang.pick(
            format!("depuis le {since_text}"),
            format!("since {since_text}"),
        ),
        change,
    };
    Some((figure, sentence))
}

/// €STR : taux au jour le jour (3 décimales), variation en pb sur un mois.
fn estr_fact(observations: &[Observation], lang: Lang) -> Option<(NoteFigure, String)> {
    let (date, value, previous) = last_and_month_ago(observations)?;
    let shown = pct(value, 3, false, lang);
    let when = full_date(date, lang);
    let change =
        previous.map(|p| over_month(bp(rounded(value, 3) - rounded(p, 3), true, lang), lang));
    let tail = change
        .as_ref()
        .map(|c| format!(" ({c})"))
        .unwrap_or_default();
    let sentence = lang.pick(
        format!("L'€STR, taux au jour le jour du marché monétaire en euros, s'établit à {shown} le {when}{tail}."),
        format!("The €STR, the euro overnight money-market rate, stood at {shown} on {when}{tail}."),
    );
    let figure = NoteFigure {
        label: "€STR".to_string(),
        value: shown,
        date: when,
        change,
    };
    Some((figure, sentence))
}

/// Pente 2-10 ans de la courbe AAA (en points de pourcentage dans les données, affichée en pb).
fn slope_fact(observations: &[Observation], lang: Lang) -> Option<(NoteFigure, String)> {
    let (date, value, previous) = last_and_month_ago(observations)?;
    let shown = bp(value, false, lang);
    let when = full_date(date, lang);
    let change =
        previous.map(|p| over_month(bp(rounded(value, 2) - rounded(p, 2), true, lang), lang));
    let tail = change
        .as_ref()
        .map(|c| format!(" ({c})"))
        .unwrap_or_default();
    let inverted = if rounded(value * 100.0, 0) < 0.0 {
        lang.pick(
            " La courbe est inversée : le taux à 2 ans dépasse le taux à 10 ans.",
            " The curve is inverted: the 2-year rate is above the 10-year rate.",
        )
    } else {
        ""
    };
    let sentence = lang.pick(
        format!(
            "La pente 2-10 ans de la courbe AAA (taux à 10 ans − taux à 2 ans, calcul Glaux) est de {shown} le {when}{tail}.{inverted}"
        ),
        format!(
            "The 2-10Y slope of the AAA curve (10-year minus 2-year rate, Glaux calculation) was {shown} on {when}{tail}.{inverted}"
        ),
    );
    let figure = NoteFigure {
        label: lang
            .pick("Pente 2-10 ans (AAA)", "2-10Y slope (AAA)")
            .to_string(),
        value: shown,
        date: when,
        change,
    };
    Some((figure, sentence))
}

/// Taux souverain 10 ans de la zone euro (courbe BCE toutes notations).
fn yield_fact(observations: &[Observation], lang: Lang) -> Option<(NoteFigure, String)> {
    let (date, value, previous) = last_and_month_ago(observations)?;
    let shown = pct(value, 2, false, lang);
    let when = full_date(date, lang);
    let change =
        previous.map(|p| over_month(bp(rounded(value, 2) - rounded(p, 2), true, lang), lang));
    let tail = change
        .as_ref()
        .map(|c| format!(" ({c})"))
        .unwrap_or_default();
    let sentence = lang.pick(
        format!(
            "Le taux souverain à 10 ans de la zone euro (courbe BCE, toutes notations) s'établit à {shown} le {when}{tail}."
        ),
        format!("The euro area 10-year sovereign yield (ECB curve, all ratings) was {shown} on {when}{tail}."),
    );
    let figure = NoteFigure {
        label: lang
            .pick("Taux souverain 10 ans", "10-year sovereign yield")
            .to_string(),
        value: shown,
        date: when,
        change,
    };
    Some((figure, sentence))
}

/// EUR/USD : cours de référence de la BCE, variation en % sur un mois.
fn eurusd_fact(observations: &[Observation], lang: Lang) -> Option<(NoteFigure, String)> {
    let (date, value, previous) = last_and_month_ago(observations)?;
    let shown = number(value, 4, false, lang);
    let when = full_date(date, lang);
    let change = previous
        .filter(|p| *p > 0.0)
        .map(|p| over_month(pct((value / p - 1.0) * 100.0, 1, true, lang), lang));
    let tail = change
        .as_ref()
        .map(|c| format!(" ({c})"))
        .unwrap_or_default();
    // En français, « dollar » reste au singulier en dessous de 2.
    let unit_fr = if value < 2.0 { "dollar" } else { "dollars" };
    let sentence = lang.pick(
        format!(
            "L'euro vaut {shown} {unit_fr} le {when}, selon le cours de référence de la BCE{tail}."
        ),
        format!("One euro was worth {shown} US dollars on {when} at the ECB reference rate{tail}."),
    );
    let figure = NoteFigure {
        label: "EUR/USD".to_string(),
        value: shown,
        date: when,
        change,
    };
    Some((figure, sentence))
}

/// Nom d'un pays (code de `COUNTRIES`).
fn country_name(code: &str, lang: Lang) -> &'static str {
    let names: [(&str, &str, &str); 8] = [
        ("de", "Allemagne", "Germany"),
        ("fr", "France", "France"),
        ("it", "Italie", "Italy"),
        ("es", "Espagne", "Spain"),
        ("gr", "Grèce", "Greece"),
        ("pt", "Portugal", "Portugal"),
        ("be", "Belgique", "Belgium"),
        ("nl", "Pays-Bas", "Netherlands"),
    ];
    names
        .iter()
        .find(|(c, _, _)| *c == code)
        .map(|(_, fr, en)| lang.pick(*fr, *en))
        .unwrap_or("?")
}

/// Pays du même mois que la zone euro (`base` : « hicp », « unemployment ») : du plus bas au plus haut.
fn country_values(
    get: &dyn Fn(&str) -> Vec<Observation>,
    base: &str,
    month: NaiveDate,
) -> Vec<(&'static str, f64)> {
    let mut values: Vec<(&'static str, f64)> = COUNTRIES
        .iter()
        .filter_map(|code| {
            let (date, value) = last_dated(&get(&format!("{base}_{code}")))?;
            (date == month).then_some((*code, value))
        })
        .collect();
    values.sort_by(|a, b| a.1.total_cmp(&b.1));
    values
}

/// « Parmi les 8 pays suivis, l'inflation va de 2,9 % (Allemagne) à 5,7 % (Grèce) en octobre 2026. »
fn country_range(
    get: &dyn Fn(&str) -> Vec<Observation>,
    base: &str,
    what: (&str, &str),
    decimals: usize,
    lang: Lang,
) -> Option<String> {
    let (month, _) = last_dated(&get(base))?;
    let values = country_values(get, base, month);
    let (low, high) = (values.first()?, values.last()?);
    if values.len() < 2 {
        return None;
    }
    let n = values.len();
    let when = month_year(month, lang);
    let (lo, hi) = (
        pct(low.1, decimals, false, lang),
        pct(high.1, decimals, false, lang),
    );
    let (lo_name, hi_name) = (country_name(low.0, lang), country_name(high.0, lang));
    let what = lang.pick(what.0, what.1);
    // Tous les pays n'ont pas forcément publié le même mois que la zone euro.
    Some(match (lang, n == COUNTRIES.len()) {
        (Lang::Fr, true) => format!("Parmi les {n} pays suivis, {what} va de {lo} ({lo_name}) à {hi} ({hi_name}) en {when}."),
        (Lang::Fr, false) => format!(
            "Parmi les {n} pays suivis dont le chiffre {} {} est publié, {what} va de {lo} ({lo_name}) à {hi} ({hi_name}).",
            fr_of_month(month.month()),
            month.year()
        ),
        (Lang::En, true) => format!(
            "Across the {n} countries tracked, {what} ranged from {lo} ({lo_name}) to {hi} ({hi_name}) in {when}."
        ),
        (Lang::En, false) => format!(
            "Across the {n} tracked countries with {when} data, {what} ranged from {lo} ({lo_name}) to {hi} ({hi_name})."
        ),
    })
}

/// Écarts de taux à 10 ans avec l'Allemagne : la liste du mois (du plus large au plus faible)
/// et le chiffre de la France.
fn spreads_facts(
    get: &dyn Fn(&str) -> Vec<Observation>,
    lang: Lang,
) -> (Option<NoteFigure>, Vec<String>) {
    let mut sentences = Vec::new();
    let france = get("spread_fr");
    let Some((month, _)) = last_dated(&france) else {
        return (None, sentences);
    };
    let mut values: Vec<(&'static str, f64)> = COUNTRIES
        .iter()
        .filter(|code| **code != "de")
        .filter_map(|code| {
            let (date, value) = last_dated(&get(&format!("spread_{code}")))?;
            (date == month).then_some((*code, value))
        })
        .collect();
    values.sort_by(|a, b| b.1.total_cmp(&a.1));
    let when = month_year(month, lang);
    let list = values
        .iter()
        .map(|(code, value)| format!("{} {}", country_name(code, lang), bp(*value, false, lang)))
        .collect::<Vec<_>>()
        .join(", ");
    sentences.push(lang.pick(
        format!("Écarts de taux à 10 ans avec l'Allemagne en {when} (calcul Glaux) : {list}."),
        format!("10-year yield spreads over Germany in {when} (Glaux calculation): {list}."),
    ));

    let (_, value, previous) = last_two_months(&france).unwrap_or((month, 0.0, None));
    let shown = bp(value, false, lang);
    let diff = previous.map(|p| rounded(value, 2) - rounded(p, 2));
    if let Some(diff) = diff {
        let size = bp(diff.abs(), false, lang);
        sentences.push(match (lang, diff) {
            (Lang::Fr, d) if d.abs() < 1e-9 => {
                format!("L'écart France–Allemagne est resté stable en un mois, à {shown}.")
            }
            (Lang::Fr, d) if d > 0.0 => {
                format!("L'écart France–Allemagne s'est creusé de {size} en un mois, à {shown}.")
            }
            (Lang::Fr, _) => {
                format!("L'écart France–Allemagne s'est resserré de {size} en un mois, à {shown}.")
            }
            (Lang::En, d) if d.abs() < 1e-9 => {
                format!("The France–Germany spread was unchanged over the month, at {shown}.")
            }
            (Lang::En, d) if d > 0.0 => {
                format!("The France–Germany spread widened by {size} over the month, to {shown}.")
            }
            (Lang::En, _) => {
                format!("The France–Germany spread narrowed by {size} over the month, to {shown}.")
            }
        });
    }
    let figure = NoteFigure {
        label: lang
            .pick("Écart France–Allemagne", "France–Germany spread")
            .to_string(),
        value: shown,
        date: when,
        change: diff.map(|d| over_month(bp(d, true, lang), lang)),
    };
    (Some(figure), sentences)
}

/// Prochaine date (aujourd'hui compris) d'une liste « AAAA-MM-JJ ».
fn next_date(dates: &[String], today: NaiveDate) -> Option<NaiveDate> {
    dates
        .iter()
        .filter_map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .find(|d| *d >= today)
}

// ---------- La note ----------

/// Ajoute chiffre et phrase à une rubrique.
fn push(section: &mut NoteSection, fact: Option<(NoteFigure, String)>) {
    if let Some((figure, sentence)) = fact {
        section.figures.push(figure);
        section.sentences.push(sentence);
    }
}

fn section(id: &'static str, title: &str) -> NoteSection {
    NoteSection {
        id,
        title: title.to_string(),
        figures: Vec::new(),
        sentences: Vec::new(),
        post_lines: Vec::new(),
    }
}

/// Puce du post : « • Inflation (sur 1 an) : 4,0 %, octobre 2026 (+0,2 pt en un mois) ».
fn bullet(figure: &NoteFigure, lang: Lang) -> String {
    let colon = lang.pick(" :", ":");
    let change = figure
        .change
        .as_ref()
        .map(|c| format!(" ({c})"))
        .unwrap_or_default();
    format!(
        "• {}{colon} {}, {}{change}",
        figure.label, figure.value, figure.date
    )
}

/// Texte du post LinkedIn : titre, analyse, chiffres par rubrique, sources.
fn build_post(title: &str, analysis: &str, sections: &[NoteSection], lang: Lang) -> String {
    let mut post = format!("{title}\n");
    let analysis = analysis.trim();
    if !analysis.is_empty() {
        post.push_str(&format!("\n{analysis}\n"));
    }
    post.push_str(&format!(
        "\n{}\n",
        lang.pick("Les chiffres du mois", "This month's figures")
    ));
    for section in sections.iter().filter(|s| !s.figures.is_empty()) {
        post.push_str(&format!("\n{}\n", section.title));
        for figure in &section.figures {
            post.push_str(&bullet(figure, lang));
            post.push('\n');
        }
        for line in &section.post_lines {
            post.push_str(&format!("• {line}\n"));
        }
    }
    post.push_str(&format!(
        "\n{}",
        lang.pick(
            "Source : BCE (données officielles). Calculs Glaux : taux directeur réel, pente 2-10 ans, écarts de taux. Ceci n'est pas un conseil en investissement.",
            "Source: ECB (official data). Glaux calculations: real policy rate, 2-10Y slope, yield spreads. This is not investment advice.",
        )
    ));
    post
}

/// Construit la note du mois de `today`. `get` renvoie les observations d'une série BCE ou
/// d'un indicateur calculé (vide si absente).
pub fn build(
    get: &dyn Fn(&str) -> Vec<Observation>,
    calendar: &Calendar,
    today: NaiveDate,
    analysis: &str,
    lang: Lang,
) -> NoteView {
    let month_key = today.format("%Y-%m").to_string();
    let title = lang.pick(
        format!("Note macro · {}", month_year(today, lang)),
        format!("Macro note · {}", month_year(today, lang)),
    );

    // Prix.
    let mut prices = section("prices", lang.pick("Prix", "Prices"));
    let hicp = monthly_fact(&get("hicp"), &HICP, lang);
    push(&mut prices, hicp.clone());
    push(
        &mut prices,
        monthly_fact(&get("core_inflation"), &CORE, lang),
    );
    push(
        &mut prices,
        monthly_fact(&get("services_inflation"), &SERVICES, lang),
    );
    prices.sentences.extend(country_range(
        get,
        "hicp",
        ("l'inflation", "inflation"),
        1,
        lang,
    ));
    if let Some(date) = next_date(&calendar.hicp, today) {
        let date = full_date(date, lang);
        prices.sentences.push(lang.pick(
            format!("Prochaine estimation rapide de l'inflation (Eurostat) : le {date}."),
            format!("Next flash inflation estimate (Eurostat): {date}."),
        ));
        prices.post_lines.push(lang.pick(
            format!("Prochaine inflation (estimation rapide) : le {date}"),
            format!("Next inflation (flash estimate): {date}"),
        ));
    }

    // Taux et politique monétaire.
    let mut rates = section(
        "rates",
        lang.pick("Taux et politique monétaire", "Rates and monetary policy"),
    );
    let deposit = deposit_fact(&get("deposit_rate"), today, lang);
    push(&mut rates, deposit.clone());
    push(&mut rates, estr_fact(&get("estr"), lang));
    push(
        &mut rates,
        monthly_fact(&get("real_rate"), &REAL_RATE, lang),
    );
    push(&mut rates, slope_fact(&get("slope"), lang));
    if let Some(date) = next_date(&calendar.ecb, today) {
        let date = full_date(date, lang);
        rates.sentences.push(lang.pick(
            format!("Prochaine décision de politique monétaire de la BCE : le {date}."),
            format!("Next ECB monetary policy decision: {date}."),
        ));
        rates.post_lines.push(lang.pick(
            format!("Prochaine décision BCE : le {date}"),
            format!("Next ECB decision: {date}"),
        ));
    }

    // Emploi, crédit et monnaie.
    let mut activity = section(
        "activity",
        lang.pick("Emploi, crédit et monnaie", "Jobs, credit and money"),
    );
    let unemployment = monthly_fact(&get("unemployment"), &UNEMPLOYMENT, lang);
    push(&mut activity, unemployment.clone());
    activity.sentences.extend(country_range(
        get,
        "unemployment",
        ("le taux de chômage", "unemployment"),
        1,
        lang,
    ));
    push(&mut activity, monthly_fact(&get("m3"), &M3, lang));
    push(
        &mut activity,
        monthly_fact(&get("loans_nfc"), &LOANS_NFC, lang),
    );
    push(
        &mut activity,
        monthly_fact(&get("loans_hh"), &LOANS_HH, lang),
    );

    // Marchés.
    let mut markets = section("markets", lang.pick("Marchés", "Markets"));
    let yield_10y = yield_fact(&get("yield_10y"), lang);
    push(&mut markets, yield_10y.clone());
    let (france, spread_sentences) = spreads_facts(get, lang);
    markets.figures.extend(france);
    markets.sentences.extend(spread_sentences);
    push(&mut markets, eurusd_fact(&get("eurusd"), lang));

    let cover = [deposit, hicp, unemployment, yield_10y]
        .into_iter()
        .flatten()
        .map(|(figure, _)| figure)
        .collect();
    let sections = vec![prices, rates, activity, markets];
    let post = build_post(&title, analysis, &sections, lang);
    NoteView {
        month: month_key,
        title,
        cover,
        sections,
        analysis: analysis.to_string(),
        post,
        post_limit: POST_LIMIT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn obs(data: &[(&str, f64)]) -> Vec<Observation> {
        data.iter()
            .map(|(date, value)| Observation {
                date: date.to_string(),
                value: *value,
            })
            .collect()
    }

    /// Mois consécutifs à partir de `start` (« 2021-11 »).
    fn months(start: &str, values: &[f64]) -> Vec<Observation> {
        let first = parse_period(start).unwrap();
        values
            .iter()
            .enumerate()
            .map(|(i, value)| Observation {
                date: first
                    .checked_add_months(Months::new(i as u32))
                    .unwrap()
                    .format("%Y-%m")
                    .to_string(),
                value: *value,
            })
            .collect()
    }

    fn calendar() -> Calendar {
        Calendar {
            ecb: vec!["2026-10-29".into(), "2026-12-17".into()],
            hicp: vec!["2026-12-01".into(), "2027-01-07".into()],
        }
    }

    fn day(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn inflation_en_hausse_avec_plus_haut() {
        // 3,0 → 4,5 en 2024, puis baisse, puis remontée à 4,0 en octobre 2026.
        let mut values = vec![3.0, 4.5, 4.2];
        values.extend(std::iter::repeat_n(3.0, 28));
        values.extend([3.8, 4.0]);
        let series = months("2024-02", &values);
        assert_eq!(series.last().unwrap().date, "2026-10");
        let (figure, sentence) = monthly_fact(&series, &HICP, Lang::Fr).unwrap();
        assert_eq!(
            sentence,
            "L'inflation de la zone euro (sur 1 an) a accéléré à 4,0\u{a0}% en octobre 2026, après 3,8\u{a0}% en septembre, son plus haut niveau depuis avril 2024."
        );
        assert_eq!(figure.change.as_deref(), Some("+0,2\u{a0}pt en un mois"));

        let (_, english) = monthly_fact(&series, &HICP, Lang::En).unwrap();
        assert_eq!(
            english,
            "Euro area inflation (y/y) rose to 4.0% in October 2026, from 3.8% in September, its highest level since April 2024."
        );
    }

    #[test]
    fn valeur_stable_a_la_precision_affichee() {
        let series = obs(&[("2026-09", 6.31), ("2026-10", 6.29)]);
        let (figure, sentence) = monthly_fact(&series, &UNEMPLOYMENT, Lang::Fr).unwrap();
        assert_eq!(
            sentence,
            "Le taux de chômage de la zone euro est resté stable à 6,3\u{a0}% en octobre 2026."
        );
        assert_eq!(figure.change.as_deref(), Some("stable sur un mois"));
    }

    #[test]
    fn mois_precedent_manquant_ou_autre_annee() {
        // Mois d'avant absent : pas de comparaison.
        let series = obs(&[("2026-08", 3.0), ("2026-10", 3.5)]);
        let (figure, sentence) = monthly_fact(&series, &M3, Lang::Fr).unwrap();
        assert_eq!(sentence, "La croissance de la masse monétaire M3 sur 1 an s'établit à +3,5\u{a0}% en octobre 2026.");
        assert_eq!(figure.change, None);
        // Janvier : le mois d'avant est en décembre de l'année précédente.
        let series = obs(&[("2025-12", 6.4), ("2026-01", 6.2)]);
        let (_, sentence) = monthly_fact(&series, &UNEMPLOYMENT, Lang::Fr).unwrap();
        assert!(
            sentence.ends_with("en janvier 2026, après 6,4\u{a0}% en décembre 2025."),
            "{sentence}"
        );
    }

    #[test]
    fn plus_bas_depuis_au_moins_le_debut_de_l_historique() {
        let mut values = vec![7.0; 30];
        values.push(6.0);
        let series = months("2024-01", &values);
        let note = extreme_note(&series, 1, false, Lang::Fr).unwrap();
        assert_eq!(note, ", son plus bas niveau depuis au moins janvier 2024");
        // Moins de 2 ans d'historique : rien.
        assert!(extreme_note(&series[10..], 1, false, Lang::Fr).is_none());
        // Plus haut atteint il y a moins d'un an : rien.
        let series = obs(&[("2026-03", 4.1), ("2026-09", 3.8), ("2026-10", 4.0)]);
        assert!(extreme_note(&series, 1, true, Lang::Fr).is_none());
    }

    #[test]
    fn taux_de_depot_inchange_ou_change() {
        let series = obs(&[("2025-03-12", 2.5), ("2025-06-11", 2.0)]);
        let (figure, sentence) = deposit_fact(&series, day("2026-11-27"), Lang::Fr).unwrap();
        assert_eq!(
            sentence,
            "Le taux de dépôt de la BCE est de 2,00\u{a0}%, inchangé depuis le 11\u{a0}juin 2025."
        );
        assert_eq!(figure.date, "depuis le 11\u{a0}juin 2025");
        assert_eq!(figure.change, None);

        let series = obs(&[
            ("2025-06-11", 2.0),
            ("2026-11-04", 2.25),
            ("2026-11-05", 2.25),
        ]);
        let (figure, sentence) = deposit_fact(&series, day("2026-11-27"), Lang::Fr).unwrap();
        assert_eq!(
            sentence,
            "La BCE a relevé son taux de dépôt de 25\u{a0}pb, à 2,25\u{a0}%, avec effet le 4\u{a0}novembre 2026."
        );
        assert_eq!(figure.change.as_deref(), Some("+25\u{a0}pb"));
        let (_, english) = deposit_fact(&series, day("2026-11-27"), Lang::En).unwrap();
        assert_eq!(
            english,
            "The ECB raised its deposit rate by 25\u{a0}bp to 2.25%, effective 4\u{a0}November 2026."
        );
    }

    #[test]
    fn series_quotidiennes() {
        let eurusd = obs(&[("2026-10-24", 1.0805), ("2026-11-25", 1.0935)]);
        let (figure, sentence) = eurusd_fact(&eurusd, Lang::Fr).unwrap();
        assert_eq!(
            sentence,
            "L'euro vaut 1,0935 dollar le 25\u{a0}novembre 2026, selon le cours de référence de la BCE (+1,2\u{a0}% en un mois)."
        );
        assert_eq!(figure.value, "1,0935");

        // Pente négative : courbe inversée.
        let slope = obs(&[("2026-10-24", -0.10), ("2026-11-24", -0.25)]);
        let (figure, sentence) = slope_fact(&slope, Lang::Fr).unwrap();
        assert_eq!(figure.value, "\u{2212}25\u{a0}pb");
        assert!(
            sentence.contains("(\u{2212}15\u{a0}pb en un mois)"),
            "{sentence}"
        );
        assert!(sentence
            .ends_with("La courbe est inversée : le taux à 2 ans dépasse le taux à 10 ans."));

        // Le 1er du mois en français.
        let estr = obs(&[("2026-12-01", 1.931)]);
        let (figure, _) = estr_fact(&estr, Lang::Fr).unwrap();
        assert_eq!(figure.date, "1er\u{a0}décembre 2026");
        assert_eq!(figure.change, None);
    }

    fn data() -> HashMap<String, Vec<Observation>> {
        let mut map = HashMap::new();
        map.insert("hicp".into(), obs(&[("2026-09", 3.8), ("2026-10", 4.0)]));
        map.insert(
            "unemployment".into(),
            obs(&[("2026-08", 6.4), ("2026-09", 6.5)]),
        );
        for (code, value) in [("de", 2.9), ("fr", 3.7), ("gr", 5.7), ("nl", 3.3)] {
            map.insert(format!("hicp_{code}"), obs(&[("2026-10", value)]));
        }
        // Pays-Bas en avance d'un mois : ignorés pour septembre.
        map.insert("unemployment_es".into(), obs(&[("2026-09", 10.3)]));
        map.insert("unemployment_de".into(), obs(&[("2026-09", 4.1)]));
        map.insert("unemployment_nl".into(), obs(&[("2026-10", 4.0)]));
        map.insert("deposit_rate".into(), obs(&[("2025-06-11", 2.5)]));
        map.insert(
            "yield_10y".into(),
            obs(&[("2026-10-23", 3.80), ("2026-11-24", 3.93)]),
        );
        for (code, value) in [("fr", 1.47), ("it", 1.26), ("nl", 0.12), ("es", 0.65)] {
            map.insert(
                format!("spread_{code}"),
                obs(&[("2026-09", value - 0.05), ("2026-10", value)]),
            );
        }
        map
    }

    #[test]
    fn note_complete_et_post() {
        let data = data();
        let get = |id: &str| data.get(id).cloned().unwrap_or_default();
        let note = build(
            &get,
            &calendar(),
            day("2026-11-27"),
            "  Mon avis.  ",
            Lang::Fr,
        );
        assert_eq!(note.month, "2026-11");
        assert_eq!(note.title, "Note macro · novembre 2026");
        assert_eq!(note.cover.len(), 4);

        let prices = &note.sections[0];
        assert!(prices.sentences.contains(
            &"Parmi les 4 pays suivis dont le chiffre d'octobre 2026 est publié, l'inflation va de 2,9\u{a0}% (Allemagne) à 5,7\u{a0}% (Grèce).".to_string()
        ));
        assert!(prices.sentences.contains(
            &"Prochaine estimation rapide de l'inflation (Eurostat) : le 1er\u{a0}décembre 2026."
                .to_string()
        ));
        let activity = &note.sections[2];
        assert!(activity.sentences.contains(
            &"Parmi les 2 pays suivis dont le chiffre de septembre 2026 est publié, le taux de chômage va de 4,1\u{a0}% (Allemagne) à 10,3\u{a0}% (Espagne).".to_string()
        ));
        let markets = &note.sections[3];
        assert_eq!(
            markets.sentences[1],
            "Écarts de taux à 10 ans avec l'Allemagne en octobre 2026 (calcul Glaux) : France 147\u{a0}pb, Italie 126\u{a0}pb, Espagne 65\u{a0}pb, Pays-Bas 12\u{a0}pb."
        );
        assert_eq!(
            markets.sentences[2],
            "L'écart France–Allemagne s'est creusé de 5\u{a0}pb en un mois, à 147\u{a0}pb."
        );

        assert!(note.post.starts_with(
            "Note macro · novembre 2026\n\nMon avis.\n\nLes chiffres du mois\n\nPrix\n"
        ));
        assert!(note.post.contains(
            "• Inflation (sur 1 an) : 4,0\u{a0}%, octobre 2026 (+0,2\u{a0}pt en un mois)\n"
        ));
        assert!(note
            .post
            .contains("• Taux de dépôt BCE : 2,50\u{a0}%, depuis le 11\u{a0}juin 2025\n• Prochaine décision BCE : le 17\u{a0}décembre 2026\n"));
        assert!(note
            .post
            .ends_with("Ceci n'est pas un conseil en investissement."));
        assert!(note.post.chars().count() < POST_LIMIT);
    }

    #[test]
    fn note_sans_donnees() {
        let get = |_: &str| Vec::new();
        let note = build(&get, &calendar(), day("2026-11-27"), "", Lang::En);
        assert!(note.cover.is_empty());
        assert!(note.sections.iter().all(|s| s.figures.is_empty()));
        assert!(note
            .post
            .starts_with("Macro note · November 2026\n\nThis month's figures\n\nSource: ECB"));
    }
}
