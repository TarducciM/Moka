//! Testi tradotti lato Rust: menu della tray, tooltip, motivo della richiesta
//! di alimentazione, riga di stato del pannello.
//!
//! Una sola fonte per Rust e pagine: `src/locales/*.json`, incluso qui al
//! momento della compilazione e letto dal pannello con `fetch`. Il test in
//! `tests/i18n.test.mjs` controlla che le lingue abbiano le stesse chiavi e che
//! ogni chiave usata (qui, nell'HTML, nel JS) esista davvero: una chiave
//! mancante non dà errori, mostra la chiave stessa a schermo (trappola 21).

use std::collections::HashMap;
use std::sync::OnceLock;

use chrono::{DateTime, Local, TimeZone};
use serde::Serialize;

use crate::session::{Mode, Now, Session};
use crate::settings::LangSetting;

const IT: &str = include_str!("../../src/locales/it.json");
const EN: &str = include_str!("../../src/locales/en.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    It,
    En,
}

impl Lang {
    pub fn resolve(setting: LangSetting) -> Lang {
        match setting {
            LangSetting::It => Lang::It,
            LangSetting::En => Lang::En,
            LangSetting::Auto => system_lang(),
        }
    }
}

/// La lingua dell'interfaccia di Windows: italiano se è italiano, altrimenti inglese.
pub fn system_lang() -> Lang {
    const LANG_ITALIAN: u16 = 0x10;
    let langid = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
    if langid & 0x3ff == LANG_ITALIAN {
        Lang::It
    } else {
        Lang::En
    }
}

fn dict(lang: Lang) -> &'static HashMap<String, String> {
    static IT_DICT: OnceLock<HashMap<String, String>> = OnceLock::new();
    static EN_DICT: OnceLock<HashMap<String, String>> = OnceLock::new();
    let (cell, src) = match lang {
        Lang::It => (&IT_DICT, IT),
        Lang::En => (&EN_DICT, EN),
    };
    cell.get_or_init(|| {
        serde_json::from_str(src).expect("locales/*.json è un oggetto piatto di stringhe")
    })
}

/// Il testo per `key`. Se manca, prova l'altra lingua e infine restituisce la
/// chiave: meglio una chiave visibile (che il test intercetta) che un crash.
pub fn t(lang: Lang, key: &str) -> String {
    let other = if lang == Lang::It { Lang::En } else { Lang::It };
    dict(lang)
        .get(key)
        .or_else(|| dict(other).get(key))
        .cloned()
        .unwrap_or_else(|| key.to_owned())
}

/// Come [`t`], sostituendo i segnaposto `{nome}`.
pub fn tv(lang: Lang, key: &str, vars: &[(&str, &str)]) -> String {
    let mut text = t(lang, key);
    for (name, value) in vars {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

/// "45 min", "2 h", "1 h 30 min".
pub fn duration_label(lang: Lang, minutes: u32) -> String {
    let (h, m) = (minutes / 60, minutes % 60);
    let (hs, ms) = (h.to_string(), m.to_string());
    match (h, m) {
        (0, _) => tv(lang, "time.minutes", &[("m", &ms)]),
        (_, 0) => tv(lang, "time.hours", &[("h", &hs)]),
        _ => tv(lang, "time.hours_minutes", &[("h", &hs), ("m", &ms)]),
    }
}

/// Il tempo che manca, arrotondato **per eccesso** al minuto: "ancora 1 min"
/// fino all'ultimo secondo, come un conto alla rovescia.
pub fn remaining_label(lang: Lang, remaining_ms: u64) -> String {
    if remaining_ms < 60_000 {
        return t(lang, "time.less_than_minute");
    }
    duration_label(lang, remaining_ms.div_ceil(60_000) as u32)
}

pub fn clock_label(wall_ms: i64) -> String {
    local(wall_ms).format("%H:%M").to_string()
}

fn local(wall_ms: i64) -> DateTime<Local> {
    Local
        .timestamp_millis_opt(wall_ms)
        .single()
        .unwrap_or_else(Local::now)
}

fn is_tomorrow(now: Now, end_wall_ms: i64) -> bool {
    local(end_wall_ms).date_naive() > local(now.wall_ms).date_naive()
}

pub fn mode_label(lang: Lang, mode: Mode) -> String {
    t(
        lang,
        match mode {
            Mode::System => "mode.system",
            Mode::Display => "mode.display",
        },
    )
}

/// "Acceso · ancora 1 h 12 min", "Acceso · fino alle 18:30", "Spento".
pub fn status_label(lang: Lang, session: Option<&Session>, now: Now) -> String {
    let Some(s) = session else {
        return t(lang, "state.off");
    };
    match (s.end, s.remaining_ms(now)) {
        (crate::session::End::Never, _) | (_, None) => t(lang, "state.on_never"),
        (crate::session::End::AfterTick { .. }, Some(r)) => tv(
            lang,
            "state.on_left",
            &[("time", &remaining_label(lang, r))],
        ),
        (crate::session::End::AtWall { at_wall_ms }, Some(_)) => {
            let key = if is_tomorrow(now, at_wall_ms) {
                "state.on_until_tomorrow"
            } else {
                "state.on_until"
            };
            tv(lang, key, &[("time", &clock_label(at_wall_ms))])
        }
    }
}

/// Tooltip dell'icona: "Moka · Solo il PC · ancora 1 h 12 min", con
/// "anche a coperchio chiuso" quando Moka sta tenendo il coperchio.
pub fn tooltip(lang: Lang, session: Option<&Session>, now: Now, lid_held: bool) -> String {
    let status = status_label(lang, session, now);
    match session {
        None => format!("Moka · {status}"),
        Some(s) if lid_held => format!(
            "Moka · {} · {} · {status}",
            mode_label(lang, s.mode),
            t(lang, "lid.tooltip")
        ),
        Some(s) => format!("Moka · {} · {status}", mode_label(lang, s.mode)),
    }
}

/// "Sospendi", "Blocca il PC"… per un "…e poi".
pub fn then_label(lang: Lang, act: crate::session::ThenAct) -> String {
    use crate::session::ThenAct;
    t(
        lang,
        match act {
            ThenAct::None => "then.none",
            ThenAct::ScreenOff => "then.screen_off",
            ThenAct::Lock => "then.lock",
            ThenAct::Sleep => "then.sleep",
            ThenAct::Hibernate => "then.hibernate",
            ThenAct::Shutdown => "then.shutdown",
        },
    )
}

/// "Sospendi", "Iberna"… per un valore dell'azione del coperchio.
pub fn lid_action_label(lang: Lang, value: u32) -> String {
    t(
        lang,
        match value {
            0 => "lid.action_0",
            1 => "lid.action_1",
            2 => "lid.action_2",
            _ => "lid.action_3",
        },
    )
}

/// Il motivo che compare in `powercfg /requests`. Si fissa alla creazione
/// della richiesta, quindi dice quando finisce (un'ora del giorno), non quanto
/// manca (che cambierebbe).
pub fn power_reason(lang: Lang, session: &Session, now: Now) -> String {
    use crate::session::End;
    let mut text = match session.end {
        End::Never => t(lang, "reason.never"),
        End::AfterTick { total_ms, .. } => {
            let end = session.end_wall_ms(now).unwrap_or(now.wall_ms);
            tv(
                lang,
                "reason.after",
                &[
                    ("dur", &duration_label(lang, (total_ms / 60_000) as u32)),
                    ("time", &clock_label(end)),
                ],
            )
        }
        End::AtWall { at_wall_ms } => {
            tv(lang, "reason.until", &[("time", &clock_label(at_wall_ms))])
        }
    };
    if session.mode == Mode::Display {
        text.push_str(&t(lang, "reason.display_suffix"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_locales_parse_and_share_keys() {
        let it = dict(Lang::It);
        let en = dict(Lang::En);
        assert!(!it.is_empty());
        let mut a: Vec<_> = it.keys().collect();
        let mut b: Vec<_> = en.keys().collect();
        a.sort();
        b.sort();
        assert_eq!(a, b);
    }

    #[test]
    fn labels() {
        assert_eq!(duration_label(Lang::It, 45), "45 min");
        assert_eq!(duration_label(Lang::It, 120), "2 h");
        assert_eq!(duration_label(Lang::En, 90), "1 h 30 min");
        assert_eq!(
            remaining_label(Lang::It, 59_999),
            t(Lang::It, "time.less_than_minute")
        );
        // Per eccesso: 1 h 11 min e 1 s si legge "1 h 12 min".
        assert_eq!(remaining_label(Lang::It, 71 * 60_000 + 1_000), "1 h 12 min");
        assert_eq!(remaining_label(Lang::It, 60_000), "1 min");
    }

    #[test]
    fn missing_key_shows_key() {
        assert_eq!(t(Lang::It, "non.esiste"), "non.esiste");
        assert_eq!(tv(Lang::En, "time.minutes", &[("m", "5")]), "5 min");
    }
}
