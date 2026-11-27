// Modèle de données et calculs affichés (dernière valeur, variation sur 1 mois).
// Aucune dépendance réseau ici : tout est testable avec `cargo test`.

use chrono::{Months, NaiveDate};
use serde::{Deserialize, Serialize};

/// Fréquence de publication d'une série.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frequency {
    /// Une valeur par jour (ouvré ou calendaire) : dates « 2026-11-21 ».
    Daily,
    /// Une valeur par mois : dates « 2026-10 ».
    Monthly,
    /// Une valeur par trimestre : dates « 2026-Q3 » (PIB).
    Quarterly,
}

impl Frequency {
    /// Écart entre deux publications, en mois (variation affichée : sur 1 mois ou 1 trimestre).
    pub fn months(self) -> u32 {
        match self {
            Frequency::Quarterly => 3,
            _ => 1,
        }
    }
}

/// Une observation : une date (au format BCE) et une valeur.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub date: String,
    pub value: f64,
}

/// Résumé envoyé à l'interface pour un indicateur.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Dernière valeur publiée.
    pub latest: Option<Observation>,
    /// Valeur en vigueur un mois plus tôt (base de la variation).
    pub reference: Option<Observation>,
    /// Taux « en escalier » (taux directeur) : date d'entrée en vigueur
    /// de la valeur actuelle. `None` pour les autres séries.
    pub since: Option<String>,
}

/// Convertit une période BCE en date : « 2026-11-21 », « 2026-10 » (→ 1er du mois) ou
/// « 2026-Q3 » (→ 1er jour du trimestre : 1er juillet).
pub fn parse_period(period: &str) -> Option<NaiveDate> {
    let period = period.trim();
    if let Some((year, quarter)) = period.split_once("-Q") {
        let quarter: u32 = quarter.parse().ok().filter(|q| (1..=4).contains(q))?;
        return NaiveDate::from_ymd_opt(year.parse().ok()?, quarter * 3 - 2, 1);
    }
    NaiveDate::parse_from_str(period, "%Y-%m-%d")
        .ok()
        .or_else(|| NaiveDate::parse_from_str(&format!("{period}-01"), "%Y-%m-%d").ok())
}

/// Valeur « en vigueur » à chaque date de `dates` (croissantes) : la dernière
/// observation datée au plus tard de cette date. Un seul passage sur les deux
/// listes (rapide même sur 5 ans de données quotidiennes).
pub(crate) fn values_at_dates(
    observations: &[Observation],
    dates: &[NaiveDate],
) -> Vec<Option<f64>> {
    let parsed: Vec<(NaiveDate, f64)> = observations
        .iter()
        .filter_map(|o| Some((parse_period(&o.date)?, o.value)))
        .collect();
    let mut next = 0;
    let mut current = None;
    dates
        .iter()
        .map(|&date| {
            while next < parsed.len() && parsed[next].0 <= date {
                current = Some(parsed[next].1);
                next += 1;
            }
            current
        })
        .collect()
}

/// Dernière observation datée au plus tard de `date` (valeur « en vigueur » à cette date).
fn value_at(observations: &[Observation], date: NaiveDate) -> Option<&Observation> {
    observations
        .iter()
        .rev()
        .find(|o| parse_period(&o.date).is_some_and(|d| d <= date))
}

/// Calcule le résumé d'une série. `observations` doit être triée par date croissante.
///
/// Variation sur 1 mois (1 trimestre pour une série trimestrielle) :
/// - série mensuelle ou trimestrielle : dernière période publiée comparée à la précédente ;
/// - série quotidienne : aujourd'hui comparé à il y a un mois (marche aussi pour
///   un taux directeur qui n'a pas bougé depuis longtemps).
pub fn summarize(
    observations: &[Observation],
    frequency: Frequency,
    step: bool,
    today: NaiveDate,
) -> Summary {
    let Some(latest) = observations.last() else {
        return Summary {
            latest: None,
            reference: None,
            since: None,
        };
    };

    let anchor = match frequency {
        Frequency::Monthly | Frequency::Quarterly => parse_period(&latest.date).unwrap_or(today),
        Frequency::Daily => today,
    };
    let reference = anchor
        .checked_sub_months(Months::new(frequency.months()))
        .and_then(|ref_date| value_at(observations, ref_date))
        .cloned();

    // Taux en escalier : on remonte tant que la valeur est identique à la dernière.
    let since = step.then(|| {
        observations
            .iter()
            .rev()
            .take_while(|o| (o.value - latest.value).abs() < 1e-9)
            .last()
            .map(|o| o.date.clone())
            .unwrap_or_else(|| latest.date.clone())
    });

    Summary {
        latest: Some(latest.clone()),
        reference,
        since,
    }
}

/// Nombre d'intervalles de la mini-courbe d'une série quotidienne : 52 semaines.
const SPARK_WEEKS: u32 = 52;

/// Points de la mini-courbe : la dernière année, à intervalles réguliers.
/// - série quotidienne : une valeur par semaine (53 points), ce qui lisse le bruit
///   et marche aussi pour un taux en escalier (valeur en vigueur chaque semaine) ;
/// - série mensuelle : une valeur par mois (13 points, du même mois l'an dernier
///   au dernier mois publié) ;
/// - série trimestrielle : une valeur par trimestre (5 points sur un an).
///
/// Les points sont équidistants : l'interface n'a besoin que des valeurs.
pub fn sparkline(observations: &[Observation], frequency: Frequency, today: NaiveDate) -> Vec<f64> {
    let Some(latest) = observations.last() else {
        return Vec::new();
    };

    let grid: Vec<NaiveDate> = match frequency {
        Frequency::Daily => (0..=SPARK_WEEKS)
            .rev()
            .filter_map(|weeks_ago| today.checked_sub_days(chrono::Days::new(7 * weeks_ago as u64)))
            .collect(),
        Frequency::Monthly | Frequency::Quarterly => {
            let last = parse_period(&latest.date).unwrap_or(today);
            let step = frequency.months();
            (0..=12 / step)
                .rev()
                .filter_map(|periods_ago| last.checked_sub_months(Months::new(periods_ago * step)))
                .collect()
        }
    };

    // Les dates antérieures au début de l'historique n'ont pas de valeur : ignorées.
    values_at_dates(observations, &grid)
        .into_iter()
        .flatten()
        .collect()
}

/// Historique affiché dans la vue détail, sur les `months` derniers mois.
/// - série quotidienne : 1 point par jour jusqu'à 1 an, 1 point par semaine au-delà
///   (≈ 260 points sur 5 ans : courbe nette et légère) ; chaque point est la valeur
///   en vigueur ce jour-là, ce qui garde la forme en escalier du taux directeur ;
/// - série mensuelle ou trimestrielle : les périodes publiées sur la période.
pub fn history(
    observations: &[Observation],
    frequency: Frequency,
    months: u32,
    today: NaiveDate,
) -> Vec<Observation> {
    let Some(latest) = observations.last() else {
        return Vec::new();
    };

    match frequency {
        Frequency::Daily => {
            let start = today
                .checked_sub_months(Months::new(months))
                .unwrap_or(today);
            let step = if months > 12 { 7 } else { 1 };
            let mut grid = Vec::new();
            let mut date = today;
            // On part d'aujourd'hui et on recule : le dernier point est toujours aujourd'hui.
            while date >= start {
                grid.push(date);
                match date.checked_sub_days(chrono::Days::new(step)) {
                    Some(previous) => date = previous,
                    None => break,
                }
            }
            grid.reverse();
            let values = values_at_dates(observations, &grid);
            grid.iter()
                .zip(values)
                .filter_map(|(d, value)| {
                    value.map(|value| Observation {
                        date: d.format("%Y-%m-%d").to_string(),
                        value,
                    })
                })
                .collect()
        }
        Frequency::Monthly | Frequency::Quarterly => {
            let last = parse_period(&latest.date).unwrap_or(today);
            let start = last.checked_sub_months(Months::new(months)).unwrap_or(last);
            observations
                .iter()
                .filter(|o| parse_period(&o.date).is_some_and(|d| d >= start))
                .cloned()
                .collect()
        }
    }
}

// ---------- Indicateurs calculés par Glaux ----------

/// Arrondi à 4 décimales : évite les « 0,149999… » des soustractions.
fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

/// Taux réel = taux directeur − inflation, mois par mois.
/// Pour chaque mois d'inflation publié, on prend le taux en vigueur le dernier jour du mois.
pub fn real_rate(policy_rate: &[Observation], inflation: &[Observation]) -> Vec<Observation> {
    let month_ends: Vec<NaiveDate> = inflation
        .iter()
        .map(|o| {
            parse_period(&o.date)
                .and_then(|first| first.checked_add_months(Months::new(1)))
                .and_then(|next| next.pred_opt())
                .unwrap_or(NaiveDate::MIN)
        })
        .collect();
    let rates = values_at_dates(policy_rate, &month_ends);
    inflation
        .iter()
        .zip(rates)
        .filter_map(|(month, rate)| {
            rate.map(|rate| Observation {
                date: month.date.clone(),
                value: round4(rate - month.value),
            })
        })
        .collect()
}

/// Écart entre deux taux (ex. pente = 10 ans − €STR), à chaque date de la série longue.
/// La série courte est prise à sa dernière valeur connue ce jour-là.
pub fn spread(long_rate: &[Observation], short_rate: &[Observation]) -> Vec<Observation> {
    let dates: Vec<NaiveDate> = long_rate
        .iter()
        .map(|o| parse_period(&o.date).unwrap_or(NaiveDate::MIN))
        .collect();
    let shorts = values_at_dates(short_rate, &dates);
    long_rate
        .iter()
        .zip(shorts)
        .filter_map(|(long, short)| {
            short.map(|short| Observation {
                date: long.date.clone(),
                value: round4(long.value - short),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(date: &str, value: f64) -> Observation {
        Observation {
            date: date.to_string(),
            value,
        }
    }

    fn day(s: &str) -> NaiveDate {
        parse_period(s).unwrap()
    }

    #[test]
    fn parse_les_deux_formats_de_date() {
        assert_eq!(
            parse_period("2026-11-21"),
            NaiveDate::from_ymd_opt(2026, 11, 21)
        );
        assert_eq!(
            parse_period("2026-10"),
            NaiveDate::from_ymd_opt(2026, 10, 1)
        );
        assert_eq!(parse_period("n'importe quoi"), None);
    }

    #[test]
    fn serie_vide() {
        let s = summarize(&[], Frequency::Daily, false, day("2026-11-24"));
        assert_eq!(s.latest, None);
        assert_eq!(s.reference, None);
    }

    #[test]
    fn mensuelle_compare_au_mois_precedent() {
        let data = [
            obs("2026-08", 2.0),
            obs("2026-09", 2.1),
            obs("2026-10", 2.3),
        ];
        let s = summarize(&data, Frequency::Monthly, false, day("2026-11-24"));
        assert_eq!(s.latest, Some(obs("2026-10", 2.3)));
        assert_eq!(s.reference, Some(obs("2026-09", 2.1)));
        assert_eq!(s.since, None);
    }

    #[test]
    fn quotidienne_compare_a_il_y_a_un_mois() {
        let data = [
            obs("2026-10-22", 1.05),
            obs("2026-10-23", 1.06),
            obs("2026-10-26", 1.07), // 24 et 25 octobre : week-end
            obs("2026-11-23", 1.10),
        ];
        // Il y a un mois = 24/10 (samedi) → dernière valeur connue : vendredi 23/10.
        let s = summarize(&data, Frequency::Daily, false, day("2026-11-24"));
        assert_eq!(s.latest, Some(obs("2026-11-23", 1.10)));
        assert_eq!(s.reference, Some(obs("2026-10-23", 1.06)));
    }

    #[test]
    fn taux_directeur_dates_de_changement_seulement() {
        // Série qui ne contient que les dates de changement.
        let data = [
            obs("2024-06-12", 3.75),
            obs("2025-03-12", 2.5),
            obs("2025-06-11", 2.0),
        ];
        let s = summarize(&data, Frequency::Daily, true, day("2026-11-24"));
        assert_eq!(s.latest, Some(obs("2025-06-11", 2.0)));
        // Il y a un mois, le taux en vigueur était déjà 2,00 % → variation nulle.
        assert_eq!(s.reference, Some(obs("2025-06-11", 2.0)));
        assert_eq!(s.since.as_deref(), Some("2025-06-11"));
    }

    #[test]
    fn taux_directeur_une_valeur_par_jour() {
        // Même taux, mais publié chaque jour : « depuis » doit rester la date du changement.
        let data = [
            obs("2025-06-09", 2.25),
            obs("2025-06-10", 2.25),
            obs("2025-06-11", 2.0),
            obs("2025-06-12", 2.0),
            obs("2026-11-23", 2.0),
        ];
        let s = summarize(&data, Frequency::Daily, true, day("2026-11-24"));
        assert_eq!(s.since.as_deref(), Some("2025-06-11"));
    }

    #[test]
    fn mini_courbe_mensuelle_13_points() {
        // 15 mois de données : on garde les 13 derniers.
        let data: Vec<_> = (1..=12)
            .map(|m| obs(&format!("2025-{m:02}"), m as f64))
            .chain([
                obs("2026-01", 13.0),
                obs("2026-02", 14.0),
                obs("2026-03", 15.0),
            ])
            .collect();
        let spark = sparkline(&data, Frequency::Monthly, day("2026-11-24"));
        assert_eq!(spark.len(), 13);
        assert_eq!(spark.first(), Some(&3.0)); // mars 2025
        assert_eq!(spark.last(), Some(&15.0)); // mars 2026
    }

    #[test]
    fn mini_courbe_quotidienne_53_points_et_escalier() {
        // Taux en escalier : 2,25 % puis 2,00 % depuis le 11/06/2026.
        let data = [obs("2024-01-01", 2.25), obs("2026-06-11", 2.0)];
        let spark = sparkline(&data, Frequency::Daily, day("2026-11-24"));
        assert_eq!(spark.len(), 53);
        assert_eq!(spark.first(), Some(&2.25));
        assert_eq!(spark.last(), Some(&2.0));
    }

    #[test]
    fn mini_courbe_historique_court() {
        // Données depuis 3 semaines seulement : uniquement les points couverts.
        let data = [obs("2026-11-03", 1.0), obs("2026-11-23", 1.2)];
        let spark = sparkline(&data, Frequency::Daily, day("2026-11-24"));
        // Semaines du 03/11, 10/11, 17/11 puis aujourd'hui (24/11).
        assert_eq!(spark, vec![1.0, 1.0, 1.0, 1.2]);
        assert!(sparkline(&[], Frequency::Daily, day("2026-11-24")).is_empty());
    }

    #[test]
    fn historique_quotidien_1_an_et_5_ans() {
        let data = [obs("2020-01-01", 1.0), obs("2026-11-20", 2.0)];
        let today = day("2026-11-24");

        let one_year = history(&data, Frequency::Daily, 12, today);
        assert_eq!(one_year.len(), 366); // du 24/11/2025 au 24/11/2026 inclus
        assert_eq!(one_year.first().unwrap().date, "2025-11-24");
        assert_eq!(one_year.last().unwrap(), &obs("2026-11-24", 2.0));

        let five_years = history(&data, Frequency::Daily, 60, today);
        assert_eq!(five_years.len(), 261); // un point par semaine
        assert_eq!(five_years.last().unwrap().date, "2026-11-24");
        assert_eq!(five_years.first().unwrap().value, 1.0);
    }

    #[test]
    fn historique_mensuel() {
        let data: Vec<_> = (1..=12)
            .map(|m| obs(&format!("2025-{m:02}"), m as f64))
            .chain([obs("2026-01", 13.0)])
            .collect();
        let h = history(&data, Frequency::Monthly, 6, day("2026-11-24"));
        // De juillet 2025 à janvier 2026 inclus.
        assert_eq!(h.len(), 7);
        assert_eq!(h.first().unwrap().date, "2025-07");
        assert!(history(&[], Frequency::Monthly, 6, day("2026-11-24")).is_empty());
    }

    #[test]
    fn taux_reel_mois_par_mois() {
        // Taux de dépôt : 2,00 % puis 2,25 % à partir du 17/06/2026.
        let policy = [obs("2025-06-11", 2.0), obs("2026-06-17", 2.25)];
        let inflation = [
            obs("2026-05", 3.0),
            obs("2026-06", 3.1),
            obs("2026-07", 3.4),
        ];
        let real = real_rate(&policy, &inflation);
        // Mai : 2,00 − 3,0 ; juin : taux en vigueur le 30/06 = 2,25 − 3,1 ; juillet : 2,25 − 3,4.
        assert_eq!(
            real,
            vec![
                obs("2026-05", -1.0),
                obs("2026-06", -0.85),
                obs("2026-07", -1.15)
            ]
        );
    }

    #[test]
    fn taux_reel_ignore_les_mois_sans_taux() {
        let policy = [obs("2026-06-17", 2.25)];
        let inflation = [obs("2026-05", 3.0), obs("2026-06", 3.1)];
        assert_eq!(real_rate(&policy, &inflation), vec![obs("2026-06", -0.85)]);
    }

    #[test]
    fn pente_10_ans_moins_estr() {
        let long = [obs("2026-11-20", 3.9), obs("2026-11-23", 3.93)];
        // €STR publié pour le 20 seulement : le 23, on garde la dernière valeur connue.
        let short = [obs("2026-11-19", 2.44), obs("2026-11-20", 2.443)];
        assert_eq!(
            spread(&long, &short),
            vec![obs("2026-11-20", 1.457), obs("2026-11-23", 1.487)]
        );
    }

    #[test]
    fn alignement_des_dates() {
        let data = [obs("2026-01-05", 1.0), obs("2026-01-10", 2.0)];
        let dates = [
            day("2026-01-01"),
            day("2026-01-05"),
            day("2026-01-07"),
            day("2026-01-12"),
        ];
        assert_eq!(
            values_at_dates(&data, &dates),
            vec![None, Some(1.0), Some(1.0), Some(2.0)]
        );
    }

    #[test]
    fn pas_de_reference_si_historique_trop_court() {
        let data = [obs("2026-11-20", 1.0), obs("2026-11-23", 1.1)];
        let s = summarize(&data, Frequency::Daily, false, day("2026-11-24"));
        assert_eq!(s.reference, None);
    }

    #[test]
    fn series_trimestrielles() {
        assert_eq!(parse_period("2026-Q3"), Some(day("2026-07-01")));
        assert_eq!(parse_period("2026-Q1"), Some(day("2026-01-01")));
        assert_eq!(parse_period("2026-Q5"), None);
        let data: Vec<Observation> = [
            ("2025-Q3", 0.3),
            ("2025-Q4", 0.2),
            ("2026-Q1", 0.0),
            ("2026-Q2", 0.6),
            ("2026-Q3", 0.4),
        ]
        .iter()
        .map(|(d, v)| Observation {
            date: d.to_string(),
            value: *v,
        })
        .collect();
        // Variation sur 1 trimestre : comparée au trimestre d'avant.
        let s = summarize(&data, Frequency::Quarterly, false, day("2026-11-27"));
        assert_eq!(s.reference.unwrap().date, "2026-Q2");
        // Mini-courbe : 5 trimestres (un an).
        assert_eq!(
            sparkline(&data, Frequency::Quarterly, day("2026-11-27")),
            vec![0.3, 0.2, 0.0, 0.6, 0.4]
        );
        assert_eq!(
            history(&data, Frequency::Quarterly, 6, day("2026-11-27")).len(),
            3
        );
    }
}
