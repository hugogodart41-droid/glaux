// Langue de Glaux : français ou anglais.
// Par défaut : la langue du système (français si le système est en français, anglais sinon).
// L'utilisateur peut la changer dans le panneau ; le choix est enregistré dans prefs.json.
//
// Les textes de l'interface sont traduits côté TypeScript (src/i18n.ts). Ici : ce que Rust
// affiche lui-même (notifications, menu de l'icône) et les formats partagés (nombres, mois).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Fr,
    En,
}

impl Lang {
    /// « fr-FR », « fr_CA », « fr » → français ; toute autre langue → anglais.
    pub fn from_locale(locale: &str) -> Self {
        if locale.to_ascii_lowercase().starts_with("fr") {
            Lang::Fr
        } else {
            Lang::En
        }
    }

    /// Langue préférée du système (Mac : Réglages Système › Général › Langue et région).
    pub fn system() -> Self {
        sys_locale::get_locale()
            .map(|locale| Self::from_locale(&locale))
            .unwrap_or(Lang::En)
    }

    /// Choisit le texte français ou anglais.
    pub fn pick<T>(self, fr: T, en: T) -> T {
        match self {
            Lang::Fr => fr,
            Lang::En => en,
        }
    }
}

const MONTHS_FR: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Nom du mois (1 = janvier).
pub fn month_name(month: u32, lang: Lang) -> &'static str {
    let index = (month.clamp(1, 12) - 1) as usize;
    lang.pick(MONTHS_FR, MONTHS_EN)[index]
}

/// « de novembre », mais « d'avril », « d'août », « d'octobre » (élision française).
pub fn fr_of_month(month: u32) -> String {
    let name = month_name(month, Lang::Fr);
    if name.starts_with(['a', 'o']) {
        format!("d'{name}")
    } else {
        format!("de {name}")
    }
}

/// Nombre : « 2,75 » en français, « 2.75 » en anglais ; vrai signe moins (−), « + » si demandé.
pub fn number(value: f64, decimals: usize, signed: bool, lang: Lang) -> String {
    let mut text = format!("{:.*}", decimals, value.abs());
    if lang == Lang::Fr {
        text = text.replace('.', ",");
    }
    // Un « -0,00 » arrondi n'a pas de signe.
    let is_zero = text.chars().all(|c| c == '0' || c == ',' || c == '.');
    match (value < 0.0 && !is_zero, signed && value > 0.0 && !is_zero) {
        (true, _) => format!("\u{2212}{text}"),
        (_, true) => format!("+{text}"),
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn langue_du_systeme() {
        assert_eq!(Lang::from_locale("fr-FR"), Lang::Fr);
        assert_eq!(Lang::from_locale("fr_CA"), Lang::Fr);
        assert_eq!(Lang::from_locale("FR"), Lang::Fr);
        assert_eq!(Lang::from_locale("en-GB"), Lang::En);
        assert_eq!(Lang::from_locale("de-DE"), Lang::En);
    }

    #[test]
    fn nombres_selon_la_langue() {
        assert_eq!(number(2.75, 2, false, Lang::Fr), "2,75");
        assert_eq!(number(2.75, 2, false, Lang::En), "2.75");
        assert_eq!(number(0.25, 2, true, Lang::Fr), "+0,25");
        assert_eq!(number(-0.25, 2, true, Lang::En), "\u{2212}0.25");
        assert_eq!(number(-0.001, 2, true, Lang::Fr), "0,00");
        assert_eq!(number(-0.001, 2, true, Lang::En), "0.00");
    }

    #[test]
    fn mois_selon_la_langue() {
        assert_eq!(month_name(8, Lang::Fr), "août");
        assert_eq!(month_name(8, Lang::En), "August");
        assert_eq!(month_name(13, Lang::En), "December"); // borné
        assert_eq!(fr_of_month(11), "de novembre");
        assert_eq!(fr_of_month(10), "d'octobre");
    }

    #[test]
    fn langue_enregistree_en_minuscules() {
        assert_eq!(serde_json::to_string(&Lang::En).unwrap(), "\"en\"");
        assert_eq!(serde_json::from_str::<Lang>("\"fr\"").unwrap(), Lang::Fr);
    }
}
