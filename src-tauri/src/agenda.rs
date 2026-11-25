// Agenda : décisions de la BCE et estimations rapides de l'inflation.
// Source unique du calendrier : le bandeau de l'interface et les rappels du jour J
// lisent tous les deux ces listes.
//
// Dates officielles, relevées le 24/11/2026 :
// - réunions de politique monétaire (jour de la décision) :
//   https://www.ecb.europa.eu/press/calendars/mgcgc/html/index.en.html
// - estimations rapides de l'inflation :
//   https://www.ecb.europa.eu/press/calendars/statscal/ges/html/sthicp.en.html
//
// À METTRE À JOUR chaque année (la BCE publie ses dates environ un an à l'avance).
// Une fois la dernière date passée, le bandeau disparaît et plus aucun rappel n'est envoyé.

use chrono::{Datelike, NaiveDate, NaiveTime};
use serde::Serialize;

use crate::alerts::Alert;
use crate::i18n::{fr_of_month, month_name, Lang};

/// Décisions de politique monétaire (annonce à 14 h 15, conférence de presse à 14 h 45).
pub const ECB_DECISIONS: [&str; 9] = [
    "2026-12-17",
    "2027-02-04",
    "2027-03-18",
    "2027-04-29",
    "2027-06-10",
    "2027-07-22",
    "2027-09-09",
    "2027-10-28",
    "2027-12-16",
];

/// Estimations rapides de l'inflation de la zone euro (elles portent sur le mois précédent).
pub const HICP_FLASH: [&str; 14] = [
    "2026-12-01",
    "2027-01-06",
    "2027-02-02",
    "2027-03-02",
    "2027-04-01",
    "2027-05-03",
    "2027-06-01",
    "2027-07-01",
    "2027-08-03",
    "2027-09-01",
    "2027-10-01",
    "2027-11-04",
    "2027-12-01",
    "2028-01-05",
];

/// Dates à venir (aujourd'hui compris), envoyées à l'interface.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgendaView {
    pub ecb: Vec<&'static str>,
    pub hicp: Vec<&'static str>,
}

fn upcoming(dates: &[&'static str], today: NaiveDate) -> Vec<&'static str> {
    dates
        .iter()
        .copied()
        .filter(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok_and(|date| date >= today))
        .collect()
}

pub fn agenda_view(today: NaiveDate) -> AgendaView {
    AgendaView {
        ecb: upcoming(&ECB_DECISIONS, today),
        hicp: upcoming(&HICP_FLASH, today),
    }
}

/// Un rappel du jour J, avec une clé unique pour ne l'envoyer qu'une fois.
#[derive(Clone, Debug, PartialEq)]
pub struct Reminder {
    pub key: String,
    pub alert: Alert,
}

/// Les rappels ne partent qu'à partir de 8 h…
const REMINDER_FROM: NaiveTime = NaiveTime::from_hms_opt(8, 0, 0).unwrap();
/// …et avant l'annonce de la BCE (14 h 15)…
const ECB_UNTIL: NaiveTime = NaiveTime::from_hms_opt(14, 15, 0).unwrap();
/// …ou avant midi pour l'inflation (publiée par Eurostat en fin de matinée).
const HICP_UNTIL: NaiveTime = NaiveTime::from_hms_opt(12, 0, 0).unwrap();

/// Rappels à envoyer maintenant : événement du jour, dans la bonne plage horaire.
/// (Mac réveillé à 16 h un jour de décision : pas de rappel « aujourd'hui à 14 h 15 » après coup.)
/// La clé ne dépend pas de la langue : changer de langue ne renvoie pas un rappel déjà envoyé.
pub fn due_reminders(today: NaiveDate, now: NaiveTime, lang: Lang) -> Vec<Reminder> {
    let iso = today.format("%Y-%m-%d").to_string();
    let mut reminders = Vec::new();

    if ECB_DECISIONS.contains(&iso.as_str()) && now >= REMINDER_FROM && now < ECB_UNTIL {
        reminders.push(Reminder {
            key: format!("ecb:{iso}"),
            alert: Alert {
                title: lang
                    .pick("Décision de la BCE aujourd'hui", "ECB decision today")
                    .into(),
                body: lang
                    .pick(
                        "Annonce des taux à 14 h 15, conférence de presse à 14 h 45.",
                        "Rates announced at 14:15 CET, press conference at 14:45.",
                    )
                    .into(),
            },
        });
    }

    if HICP_FLASH.contains(&iso.as_str()) && now >= REMINDER_FROM && now < HICP_UNTIL {
        // L'estimation porte sur le mois précédent la publication.
        let month = if today.month() == 1 {
            12
        } else {
            today.month() - 1
        };
        reminders.push(Reminder {
            key: format!("hicp:{iso}"),
            alert: Alert {
                title: lang
                    .pick(
                        "Inflation publiée aujourd'hui",
                        "Euro area inflation out today",
                    )
                    .into(),
                body: lang.pick(
                    format!(
                        "Estimation rapide de l'inflation {} dans la zone euro.",
                        fr_of_month(month)
                    ),
                    format!(
                        "Flash estimate of {} inflation in the euro area.",
                        month_name(month, lang)
                    ),
                ),
            },
        });
    }

    reminders
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn at(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn calendrier_trie_et_valide() {
        for list in [&ECB_DECISIONS[..], &HICP_FLASH[..]] {
            let dates: Vec<NaiveDate> = list.iter().map(|d| day(d)).collect();
            assert!(
                dates.windows(2).all(|w| w[0] < w[1]),
                "dates non triées : {list:?}"
            );
        }
    }

    #[test]
    fn agenda_garde_les_dates_a_venir_aujourdhui_compris() {
        let view = agenda_view(day("2026-12-17"));
        assert_eq!(view.ecb.first(), Some(&"2026-12-17"));
        assert_eq!(view.hicp.first(), Some(&"2027-01-06"));
        let empty = agenda_view(day("2028-02-01"));
        assert!(empty.ecb.is_empty() && empty.hicp.is_empty());
    }

    #[test]
    fn rappel_bce_le_matin_du_jour_j_seulement() {
        let today = day("2026-12-17");
        assert!(due_reminders(today, at(7, 30), Lang::Fr).is_empty()); // trop tôt
        let morning = due_reminders(today, at(9, 0), Lang::Fr);
        assert_eq!(morning.len(), 1);
        assert_eq!(morning[0].key, "ecb:2026-12-17");
        assert!(due_reminders(today, at(15, 0), Lang::Fr).is_empty()); // après l'annonce
        assert!(due_reminders(day("2026-12-16"), at(9, 0), Lang::Fr).is_empty());
        // la veille
    }

    #[test]
    fn rappel_inflation_avec_le_mois_concerne() {
        let december = due_reminders(day("2026-12-01"), at(8, 30), Lang::Fr);
        assert_eq!(december[0].key, "hicp:2026-12-01");
        assert!(december[0].alert.body.contains("novembre"));
        // Publication de janvier : inflation de décembre.
        let january = due_reminders(day("2027-01-06"), at(10, 0), Lang::Fr);
        assert!(january[0].alert.body.contains("décembre"));
        assert!(due_reminders(day("2026-12-01"), at(12, 30), Lang::Fr).is_empty());
        // Élision : « de l'inflation d'octobre » (publication de novembre).
        let november = due_reminders(day("2027-11-04"), at(9, 0), Lang::Fr);
        assert!(november[0].alert.body.contains("l'inflation d'octobre"));
    }

    #[test]
    fn rappels_en_anglais_avec_la_meme_cle() {
        let fr = due_reminders(day("2026-12-17"), at(9, 0), Lang::Fr);
        let en = due_reminders(day("2026-12-17"), at(9, 0), Lang::En);
        assert_eq!(fr[0].key, en[0].key);
        assert_eq!(en[0].alert.title, "ECB decision today");
        let hicp = due_reminders(day("2026-12-01"), at(9, 0), Lang::En);
        assert_eq!(
            hicp[0].alert.body,
            "Flash estimate of November inflation in the euro area."
        );
    }
}
