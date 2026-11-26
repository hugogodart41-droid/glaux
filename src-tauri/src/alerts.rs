// Textes des notifications (français ou anglais) et détection des nouvelles données
// qui méritent une alerte : changement du taux de dépôt, nouvelle inflation publiée.

use chrono::Datelike;

use crate::data::{parse_period, Observation};
use crate::i18n::{fr_of_month, month_name, number, Lang};

/// Une notification à afficher.
#[derive(Clone, Debug, PartialEq)]
pub struct Alert {
    pub title: String,
    pub body: String,
    /// Indicateur ouvert par un clic sur la notification (Mac) : « deposit_rate », « hicp » ;
    /// None : simple ouverture du panneau.
    pub indicator: Option<&'static str>,
}

/// Alerte si le taux de dépôt a changé entre l'ancien et le nouveau téléchargement.
pub fn rate_change_alert(old: &Observation, new: &Observation, lang: Lang) -> Option<Alert> {
    let change = new.value - old.value;
    if change.abs() < 1e-9 {
        return None;
    }
    let title = match (lang, change > 0.0) {
        (Lang::Fr, true) => "La BCE relève ses taux",
        (Lang::Fr, false) => "La BCE baisse ses taux",
        (Lang::En, true) => "The ECB raises rates",
        (Lang::En, false) => "The ECB cuts rates",
    };
    // Date d'entrée en vigueur : « 23 décembre » / « 23 December ».
    let effective = parse_period(&new.date)
        .map(|d| {
            let date = format!("{} {}", d.day(), month_name(d.month(), lang));
            lang.pick(
                format!(", en vigueur depuis le {date}"),
                format!(", effective {date}"),
            )
        })
        .unwrap_or_default();
    let rate = number(new.value, 2, false, lang);
    let change = number(change, 2, true, lang);
    Some(Alert {
        title: title.into(),
        body: lang.pick(
            format!("Taux de dépôt : {rate} % ({change} pt){effective}."),
            format!("Deposit rate: {rate}% ({change} pp){effective}."),
        ),
        indicator: Some("deposit_rate"),
    })
}

/// Alerte si un nouveau mois d'inflation est apparu. `latest` est la série à jour
/// (triée), `previous_latest` la dernière valeur connue avant le téléchargement.
pub fn new_inflation_alert(
    previous_latest: &Observation,
    latest: &[Observation],
    lang: Lang,
) -> Option<Alert> {
    let newest = latest.last()?;
    let new_month = parse_period(&newest.date)?;
    if new_month <= parse_period(&previous_latest.date)? {
        return None;
    }
    let before = latest.iter().rev().nth(1);
    let comparison = before
        .and_then(|b| {
            let month = month_name(parse_period(&b.date)?.month(), lang);
            let change = number(newest.value - b.value, 1, true, lang);
            Some(lang.pick(
                format!(" ({change} pt par rapport à {month})"),
                format!(" ({change} pp vs {month})"),
            ))
        })
        .unwrap_or_default();
    let value = number(newest.value, 1, false, lang);
    Some(Alert {
        title: lang.pick(
            format!("Inflation {} : {value} %", fr_of_month(new_month.month())),
            format!(
                "{} inflation: {value}%",
                month_name(new_month.month(), lang)
            ),
        ),
        body: lang.pick(
            format!("Zone euro, sur 1 an{comparison}."),
            format!("Euro area, year on year{comparison}."),
        ),
        indicator: Some("hicp"),
    })
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

    #[test]
    fn hausse_et_baisse_de_taux() {
        let up =
            rate_change_alert(&obs("2026-09-16", 2.5), &obs("2026-12-23", 2.75), Lang::Fr).unwrap();
        assert_eq!(up.title, "La BCE relève ses taux");
        assert_eq!(
            up.body,
            "Taux de dépôt : 2,75 % (+0,25 pt), en vigueur depuis le 23 décembre."
        );
        let down =
            rate_change_alert(&obs("2026-09-16", 2.5), &obs("2026-12-23", 2.25), Lang::Fr).unwrap();
        assert_eq!(down.title, "La BCE baisse ses taux");
        assert!(
            rate_change_alert(&obs("2026-09-16", 2.5), &obs("2026-11-24", 2.5), Lang::Fr).is_none()
        );
    }

    #[test]
    fn nouvelle_inflation() {
        let series = [obs("2026-10", 4.0), obs("2026-11", 4.1)];
        let alert = new_inflation_alert(&obs("2026-10", 4.0), &series, Lang::Fr).unwrap();
        assert_eq!(alert.title, "Inflation de novembre : 4,1 %");
        assert_eq!(
            alert.body,
            "Zone euro, sur 1 an (+0,1 pt par rapport à octobre)."
        );
        // Même mois qu'avant : pas d'alerte.
        assert!(new_inflation_alert(&obs("2026-11", 4.1), &series, Lang::Fr).is_none());
        // Élision : « d'octobre ».
        let october = new_inflation_alert(&obs("2026-09", 3.8), &series[..1], Lang::Fr).unwrap();
        assert_eq!(october.title, "Inflation d'octobre : 4,0 %");
    }

    #[test]
    fn alertes_en_anglais() {
        let up =
            rate_change_alert(&obs("2026-09-16", 2.5), &obs("2026-12-23", 2.75), Lang::En).unwrap();
        assert_eq!(up.title, "The ECB raises rates");
        assert_eq!(
            up.body,
            "Deposit rate: 2.75% (+0.25 pp), effective 23 December."
        );
        let down =
            rate_change_alert(&obs("2026-09-16", 2.5), &obs("2026-12-23", 2.25), Lang::En).unwrap();
        assert_eq!(down.title, "The ECB cuts rates");

        let series = [obs("2026-10", 4.0), obs("2026-11", 4.1)];
        let alert = new_inflation_alert(&obs("2026-10", 4.0), &series, Lang::En).unwrap();
        assert_eq!(alert.title, "November inflation: 4.1%");
        assert_eq!(alert.body, "Euro area, year on year (+0.1 pp vs October).");
    }
}
