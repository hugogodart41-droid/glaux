// Agenda : décisions de la BCE et estimations rapides de l'inflation.
// Le bandeau de l'interface et les rappels du jour J lisent tous les deux le calendrier
// en vigueur (`Calendar`) :
// - dates intégrées ci-dessous (relevées le 24/11/2026) ;
// - remplacées par celles du fichier agenda.json du dépôt GitHub dès qu'il va plus loin
//   (ou corrige une date) : les apps installées suivent sans nouvelle version.
//
// Sources officielles :
// - réunions de politique monétaire (jour de la décision) :
//   https://www.ecb.europa.eu/press/calendars/mgcgc/html/index.en.html
// - estimations rapides de l'inflation :
//   https://www.ecb.europa.eu/press/calendars/statscal/ges/html/sthicp.en.html
//
// À METTRE À JOUR chaque année dans agenda.json (la BCE publie ses dates environ un an
// à l'avance) ; les dates intégrées, à la prochaine version de Glaux.

use chrono::{Datelike, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

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

/// Adresse du calendrier publié avec le code (fichier agenda.json à la racine du dépôt).
pub const REMOTE_URL: &str =
    "https://raw.githubusercontent.com/hugogodart41-droid/glaux/main/agenda.json";

/// Dates par liste au plus (garde-fou contre un fichier aberrant).
const MAX_DATES: usize = 60;

/// Calendrier en vigueur : décisions de la BCE et estimations rapides de l'inflation,
/// dates « AAAA-MM-JJ » triées. Même forme que le fichier agenda.json.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Calendar {
    #[serde(rename = "ecb_decisions")]
    pub ecb: Vec<String>,
    #[serde(rename = "hicp_flash")]
    pub hicp: Vec<String>,
}

impl Calendar {
    /// Dates intégrées à cette version de Glaux.
    pub fn builtin() -> Self {
        Self {
            ecb: ECB_DECISIONS.iter().map(|d| d.to_string()).collect(),
            hicp: HICP_FLASH.iter().map(|d| d.to_string()).collect(),
        }
    }

    /// Lit un fichier agenda.json (champs en plus ignorés, ex. « comment ») et le vérifie :
    /// listes non vides, dates valides entre 2024 et 2100, strictement croissantes.
    pub fn parse(json: &str) -> Result<Self, String> {
        let calendar: Self =
            serde_json::from_str(json).map_err(|e| format!("agenda.json illisible : {e}"))?;
        for list in [&calendar.ecb, &calendar.hicp] {
            if list.is_empty() || list.len() > MAX_DATES {
                return Err("agenda.json : liste vide ou trop longue".into());
            }
            let mut previous: Option<NaiveDate> = None;
            for text in list {
                let date = NaiveDate::parse_from_str(text, "%Y-%m-%d")
                    .map_err(|_| format!("agenda.json : date invalide « {text} »"))?;
                if !(2024..=2100).contains(&date.year()) || previous.is_some_and(|p| p >= date) {
                    return Err(format!("agenda.json : date hors ordre « {text} »"));
                }
                previous = Some(date);
            }
        }
        Ok(calendar)
    }

    /// Garde, liste par liste, celle qui va le plus loin dans le temps ; à égalité, celle de
    /// `other` (le fichier publié peut corriger une date sans en ajouter).
    pub fn merged_with(&self, other: &Self) -> Self {
        let pick = |mine: &Vec<String>, theirs: &Vec<String>| {
            if theirs.last() >= mine.last() {
                theirs.clone()
            } else {
                mine.clone()
            }
        };
        Self {
            ecb: pick(&self.ecb, &other.ecb),
            hicp: pick(&self.hicp, &other.hicp),
        }
    }
}

/// Télécharge le calendrier publié sur GitHub (texte brut, vérifié ensuite par `parse`).
pub fn fetch_remote(agent: &ureq::Agent) -> Result<String, String> {
    let mut response = agent
        .get(REMOTE_URL)
        .call()
        .map_err(|e| format!("calendrier injoignable ({e})"))?;
    if response.status().as_u16() != 200 {
        return Err(format!(
            "calendrier : code HTTP {}",
            response.status().as_u16()
        ));
    }
    response
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("calendrier interrompu ({e})"))
}

/// Dates à venir (aujourd'hui compris), envoyées à l'interface.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgendaView {
    pub ecb: Vec<String>,
    pub hicp: Vec<String>,
}

fn upcoming(dates: &[String], today: NaiveDate) -> Vec<String> {
    dates
        .iter()
        .filter(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok_and(|date| date >= today))
        .cloned()
        .collect()
}

pub fn agenda_view(calendar: &Calendar, today: NaiveDate) -> AgendaView {
    AgendaView {
        ecb: upcoming(&calendar.ecb, today),
        hicp: upcoming(&calendar.hicp, today),
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
pub fn due_reminders(
    calendar: &Calendar,
    today: NaiveDate,
    now: NaiveTime,
    lang: Lang,
) -> Vec<Reminder> {
    let iso = today.format("%Y-%m-%d").to_string();
    let mut reminders = Vec::new();

    if calendar.ecb.contains(&iso) && now >= REMINDER_FROM && now < ECB_UNTIL {
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
                indicator: Some("deposit_rate"),
            },
        });
    }

    if calendar.hicp.contains(&iso) && now >= REMINDER_FROM && now < HICP_UNTIL {
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
                indicator: Some("hicp"),
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

    fn cal() -> Calendar {
        Calendar::builtin()
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
        let view = agenda_view(&cal(), day("2026-12-17"));
        assert_eq!(view.ecb.first().map(String::as_str), Some("2026-12-17"));
        assert_eq!(view.hicp.first().map(String::as_str), Some("2027-01-06"));
        let empty = agenda_view(&cal(), day("2028-02-01"));
        assert!(empty.ecb.is_empty() && empty.hicp.is_empty());
    }

    #[test]
    fn calendrier_publie_lu_et_verifie() {
        let json = r#"{"comment": "test", "ecb_decisions": ["2027-02-04", "2028-02-03"],
                       "hicp_flash": ["2027-01-06", "2028-01-05"]}"#;
        let remote = Calendar::parse(json).unwrap();
        assert_eq!(remote.ecb.len(), 2);
        assert!(Calendar::parse(r#"{"ecb_decisions": [], "hicp_flash": ["2027-01-06"]}"#).is_err());
        assert!(Calendar::parse(
            r#"{"ecb_decisions": ["2027-13-01"], "hicp_flash": ["2027-01-06"]}"#
        )
        .is_err());
        assert!(Calendar::parse(
            r#"{"ecb_decisions": ["2027-03-01", "2027-02-01"], "hicp_flash": ["2027-01-06"]}"#
        )
        .is_err());
        assert!(Calendar::parse("pas du json").is_err());

        // Le fichier publié va plus loin (2028) : il remplace les dates intégrées.
        let merged = cal().merged_with(&remote);
        assert_eq!(merged.ecb, remote.ecb);
        let reminders = due_reminders(&merged, day("2028-02-03"), at(9, 0), Lang::Fr);
        assert_eq!(reminders.len(), 1);
        // Un fichier plus ancien que les dates intégrées est ignoré.
        let old =
            Calendar::parse(r#"{"ecb_decisions": ["2026-12-17"], "hicp_flash": ["2026-12-01"]}"#)
                .unwrap();
        assert_eq!(cal().merged_with(&old), cal());
        // À égalité de dernière date : le fichier publié (une date corrigée).
        let mut fixed = cal();
        fixed.ecb[0] = "2026-12-18".into();
        assert_eq!(cal().merged_with(&fixed).ecb[0], "2026-12-18");
        // Le fichier du dépôt est valide.
        let repo = include_str!("../../agenda.json");
        assert_eq!(Calendar::parse(repo).unwrap(), cal());
    }

    #[test]
    fn rappel_bce_le_matin_du_jour_j_seulement() {
        let today = day("2026-12-17");
        assert!(due_reminders(&cal(), today, at(7, 30), Lang::Fr).is_empty()); // trop tôt
        let morning = due_reminders(&cal(), today, at(9, 0), Lang::Fr);
        assert_eq!(morning.len(), 1);
        assert_eq!(morning[0].key, "ecb:2026-12-17");
        assert!(due_reminders(&cal(), today, at(15, 0), Lang::Fr).is_empty()); // après l'annonce
        assert!(due_reminders(&cal(), day("2026-12-16"), at(9, 0), Lang::Fr).is_empty());
        // la veille
    }

    #[test]
    fn rappel_inflation_avec_le_mois_concerne() {
        let december = due_reminders(&cal(), day("2026-12-01"), at(8, 30), Lang::Fr);
        assert_eq!(december[0].key, "hicp:2026-12-01");
        assert!(december[0].alert.body.contains("novembre"));
        // Publication de janvier : inflation de décembre.
        let january = due_reminders(&cal(), day("2027-01-06"), at(10, 0), Lang::Fr);
        assert!(january[0].alert.body.contains("décembre"));
        assert!(due_reminders(&cal(), day("2026-12-01"), at(12, 30), Lang::Fr).is_empty());
        // Élision : « de l'inflation d'octobre » (publication de novembre).
        let november = due_reminders(&cal(), day("2027-11-04"), at(9, 0), Lang::Fr);
        assert!(november[0].alert.body.contains("l'inflation d'octobre"));
    }

    #[test]
    fn rappels_en_anglais_avec_la_meme_cle() {
        let fr = due_reminders(&cal(), day("2026-12-17"), at(9, 0), Lang::Fr);
        let en = due_reminders(&cal(), day("2026-12-17"), at(9, 0), Lang::En);
        assert_eq!(fr[0].key, en[0].key);
        assert_eq!(en[0].alert.title, "ECB decision today");
        let hicp = due_reminders(&cal(), day("2026-12-01"), at(9, 0), Lang::En);
        assert_eq!(
            hicp[0].alert.body,
            "Flash estimate of November inflation in the euro area."
        );
    }
}
