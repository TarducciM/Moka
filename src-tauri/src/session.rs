//! La sessione: cosa tenere acceso e fino a quando. Logica pura, senza
//! Windows: l'orologio arriva da fuori ([`Now`]), così scadenze, ritorno dalla
//! sospensione e cambi d'ora si provano nei test senza aspettare.
//!
//! Due orologi, ognuno per ciò che sa fare:
//! - le **durate** ("per 2 h") usano il tick di sistema (`GetTickCount64`):
//!   monotono, immune ai cambi d'ora, e conta anche il tempo passato in
//!   sospensione. "Per 2 h" dalle 14:00 finisce alle 16:00 anche se in mezzo il
//!   portatile è stato chiuso un'ora;
//! - "**fino alle** HH:MM" usa l'orologio di sistema, perché è un'ora del
//!   giorno e deve seguirlo.

use chrono::{DateTime, Duration, LocalResult, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};

/// Durata massima accettata ovunque (riga di comando, impostazioni): 7 giorni.
pub const MAX_MINUTES: u32 = 7 * 24 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Niente sospensione; lo schermo segue il piano energetico.
    #[default]
    System,
    /// Niente sospensione né schermo spento.
    Display,
}

/// Un istante letto dai due orologi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Now {
    /// Millisecondi dall'avvio di Windows, sospensione inclusa.
    pub tick_ms: u64,
    /// Millisecondi UTC dall'epoch Unix.
    pub wall_ms: i64,
}

impl Now {
    /// Un identificativo dell'avvio di Windows: l'istante (sull'orologio di
    /// sistema) in cui il tick valeva zero. Resta costante per tutto l'avvio,
    /// sospensioni comprese, perché i due orologi avanzano insieme.
    pub fn boot_wall_ms(self) -> i64 {
        self.wall_ms - self.tick_ms as i64
    }
}

/// Come l'utente ha chiesto la sessione. È anche ciò che si ricorda come
/// "ultima scelta", per riaccendere con un clic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Spec {
    /// Finché non la si spegne.
    #[default]
    Never,
    /// Per un certo numero di minuti.
    Minutes { minutes: u32 },
    /// Fino alle HH:MM (la prossima volta che succede).
    Until { hour: u32, minute: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum End {
    Never,
    /// Scadenza sul tick: `total_ms` serve solo a mostrarla ("per 2 h").
    #[serde(rename_all = "camelCase")]
    AfterTick {
        at_tick_ms: u64,
        total_ms: u64,
    },
    /// Scadenza sull'orologio di sistema.
    #[serde(rename_all = "camelCase")]
    AtWall {
        at_wall_ms: i64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub mode: Mode,
    pub end: End,
    pub spec: Spec,
    pub started_wall_ms: i64,
    /// Anche a coperchio chiuso (solo sui portatili, e solo se l'utente ha
    /// acconsentito: vedi `Settings::effective_lid_mode`).
    #[serde(default)]
    pub lid: bool,
}

impl Session {
    /// Crea una sessione che parte adesso. `tz` è il fuso con cui interpretare
    /// "fino alle HH:MM" (in produzione quello locale).
    pub fn start<Tz: TimeZone>(mode: Mode, spec: Spec, now: Now, tz: &Tz) -> Session {
        let end = match spec {
            Spec::Never => End::Never,
            Spec::Minutes { minutes } => {
                let total_ms = u64::from(minutes.clamp(1, MAX_MINUTES)) * 60_000;
                End::AfterTick {
                    at_tick_ms: now.tick_ms + total_ms,
                    total_ms,
                }
            }
            Spec::Until { hour, minute } => End::AtWall {
                at_wall_ms: next_occurrence(now.wall_ms, hour, minute, tz),
            },
        };
        Session {
            mode,
            end,
            spec,
            started_wall_ms: now.wall_ms,
            lid: false,
        }
    }

    /// Millisecondi che mancano alla fine; `None` se non finisce mai.
    pub fn remaining_ms(&self, now: Now) -> Option<u64> {
        match self.end {
            End::Never => None,
            End::AfterTick { at_tick_ms, .. } => Some(at_tick_ms.saturating_sub(now.tick_ms)),
            End::AtWall { at_wall_ms } => Some((at_wall_ms - now.wall_ms).max(0) as u64),
        }
    }

    pub fn is_expired(&self, now: Now) -> bool {
        self.remaining_ms(now) == Some(0)
    }

    /// L'istante di fine sull'orologio di sistema, se esiste: serve per dire
    /// "fino alle 16:12" anche di una sessione a durata.
    pub fn end_wall_ms(&self, now: Now) -> Option<i64> {
        match self.end {
            End::Never => None,
            End::AtWall { at_wall_ms } => Some(at_wall_ms),
            End::AfterTick { .. } => self.remaining_ms(now).map(|r| now.wall_ms + r as i64),
        }
    }
}

/// La prossima volta che l'orologio locale segna `hour:minute`, in ms UTC.
/// Se oggi quell'ora è già passata (o è adesso), è domani.
///
/// Cambio dell'ora legale: un'ora che non esiste (il salto in avanti) diventa
/// il primo istante valido dopo il salto; un'ora che esiste due volte (il salto
/// indietro) vale la prima delle due.
pub fn next_occurrence<Tz: TimeZone>(now_wall_ms: i64, hour: u32, minute: u32, tz: &Tz) -> i64 {
    let hour = hour.min(23);
    let minute = minute.min(59);
    let now = tz
        .timestamp_millis_opt(now_wall_ms)
        .single()
        .expect("un timestamp UTC ha sempre un'ora locale");
    let target = NaiveTime::from_hms_opt(hour, minute, 0).expect("ora valida");
    let mut day = now.date_naive();
    for _ in 0..3 {
        let naive = day.and_time(target);
        let candidate: Option<DateTime<Tz>> = match tz.from_local_datetime(&naive) {
            LocalResult::Single(t) => Some(t),
            LocalResult::Ambiguous(first, _) => Some(first),
            // L'ora non esiste (salto in avanti): si prende un'ora dopo, che esiste.
            LocalResult::None => tz
                .from_local_datetime(&(naive + Duration::hours(1)))
                .earliest(),
        };
        if let Some(t) = candidate {
            if t.timestamp_millis() > now_wall_ms {
                return t.timestamp_millis();
            }
        }
        day = day.succ_opt().expect("data dentro l'intervallo di chrono");
    }
    // Irraggiungibile in pratica: fra oggi, domani e dopodomani c'è sempre l'ora.
    now_wall_ms + 24 * 3_600_000
}

/// Legge una durata scritta a mano: `90`, `90m`, `15min`, `2h`, `1h30`, `1h30m`,
/// `1.5h`. Restituisce i minuti, oppure `None` se non si capisce o se è fuori
/// da 1 minuto … 7 giorni.
pub fn parse_duration(text: &str) -> Option<u32> {
    let s: String = text
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if s.is_empty() {
        return None;
    }

    let minutes: f64 = if let Some((h, rest)) = s.split_once('h') {
        let hours: f64 = h.replace(',', ".").parse().ok()?;
        let rest = rest
            .strip_suffix("min")
            .or_else(|| rest.strip_suffix('m'))
            .unwrap_or(rest);
        let extra: f64 = if rest.is_empty() {
            0.0
        } else {
            let m: u32 = rest.parse().ok()?;
            if m >= 60 {
                return None;
            }
            f64::from(m)
        };
        hours * 60.0 + extra
    } else {
        let digits = s
            .strip_suffix("min")
            .or_else(|| s.strip_suffix('m'))
            .unwrap_or(&s);
        digits.parse::<u32>().ok()? as f64
    };

    if !minutes.is_finite() || minutes.fract() != 0.0 {
        return None;
    }
    let minutes = minutes as i64;
    if minutes < 1 || minutes > i64::from(MAX_MINUTES) {
        return None;
    }
    Some(minutes as u32)
}

/// Scrive una durata nella forma che [`parse_duration`] rilegge: `45m`, `2h`, `1h30m`.
pub fn format_duration_compact(minutes: u32) -> String {
    let (h, m) = (minutes / 60, minutes % 60);
    match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h{m}m"),
    }
}

/// Legge un orario: `18:30`, `8:05`, `18.30` (il punto è comune in italiano), `18`.
pub fn parse_clock(text: &str) -> Option<(u32, u32)> {
    let s = text.trim();
    let (h, m) = match s.split_once([':', '.']) {
        Some((h, m)) => (h, m),
        None => (s, "0"),
    };
    if h.is_empty() || h.len() > 2 || m.is_empty() || m.len() > 2 {
        return None;
    }
    let h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    (h < 24 && m < 60).then_some((h, m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn rome_summer() -> FixedOffset {
        FixedOffset::east_opt(2 * 3600).unwrap()
    }

    /// 2026-09-22 14:00:00 a Roma (UTC+2) = 12:00:00 UTC.
    const WALL_1400: i64 = 1_790_078_400_000;

    fn now(tick_ms: u64, wall_ms: i64) -> Now {
        Now { tick_ms, wall_ms }
    }

    #[test]
    fn wall_constant_is_right() {
        let t = rome_summer().timestamp_millis_opt(WALL_1400).unwrap();
        assert_eq!(t.format("%Y-%m-%d %H:%M").to_string(), "2026-09-22 14:00");
    }

    #[test]
    fn duration_counts_on_tick_and_includes_sleep() {
        let s = Session::start(
            Mode::System,
            Spec::Minutes { minutes: 120 },
            now(1_000, WALL_1400),
            &rome_summer(),
        );
        assert_eq!(s.remaining_ms(now(1_000, WALL_1400)), Some(7_200_000));
        // Un'ora dopo, anche se passata in sospensione (il tick la conta).
        let later = now(1_000 + 3_600_000, WALL_1400 + 3_600_000);
        assert_eq!(s.remaining_ms(later), Some(3_600_000));
        assert!(!s.is_expired(later));
        let end = now(1_000 + 7_200_000, WALL_1400 + 7_200_000);
        assert!(s.is_expired(end));
        // Oltre la scadenza resta scaduta, senza andare sotto zero.
        assert_eq!(s.remaining_ms(now(u64::MAX / 2, 0)), Some(0));
    }

    #[test]
    fn duration_ignores_clock_changes() {
        let s = Session::start(
            Mode::System,
            Spec::Minutes { minutes: 30 },
            now(0, WALL_1400),
            &rome_summer(),
        );
        // L'utente sposta l'orologio avanti di 5 ore: la durata non ne risente.
        let moved = now(60_000, WALL_1400 + 5 * 3_600_000);
        assert_eq!(s.remaining_ms(moved), Some(29 * 60_000));
    }

    #[test]
    fn until_later_today() {
        let tz = rome_summer();
        let at = next_occurrence(WALL_1400, 18, 30, &tz);
        assert_eq!(at - WALL_1400, (4 * 60 + 30) * 60_000);
    }

    #[test]
    fn until_already_passed_means_tomorrow() {
        let tz = rome_summer();
        let at = next_occurrence(WALL_1400, 8, 0, &tz);
        assert_eq!(at - WALL_1400, 18 * 3_600_000);
        // Esattamente adesso: domani, non una sessione di zero secondi.
        let same = next_occurrence(WALL_1400, 14, 0, &tz);
        assert_eq!(same - WALL_1400, 24 * 3_600_000);
    }

    #[test]
    fn until_session_follows_wall_clock() {
        let tz = rome_summer();
        let s = Session::start(
            Mode::Display,
            Spec::Until {
                hour: 15,
                minute: 0,
            },
            now(0, WALL_1400),
            &tz,
        );
        assert_eq!(s.remaining_ms(now(10, WALL_1400)), Some(3_600_000));
        assert!(s.is_expired(now(10, WALL_1400 + 3_600_000)));
    }

    #[test]
    fn boot_id_is_stable_across_sleep() {
        let before = now(10_000, WALL_1400);
        // Un'ora di sospensione: avanzano entrambi gli orologi.
        let after = now(10_000 + 3_600_000, WALL_1400 + 3_600_000);
        assert_eq!(before.boot_wall_ms(), after.boot_wall_ms());
    }

    #[test]
    fn parse_duration_accepts_common_forms() {
        assert_eq!(parse_duration("15"), Some(15));
        assert_eq!(parse_duration("15m"), Some(15));
        assert_eq!(parse_duration(" 15 min "), Some(15));
        assert_eq!(parse_duration("2h"), Some(120));
        assert_eq!(parse_duration("1h30"), Some(90));
        assert_eq!(parse_duration("1h30m"), Some(90));
        assert_eq!(parse_duration("1H 30M"), Some(90));
        assert_eq!(parse_duration("1.5h"), Some(90));
        assert_eq!(parse_duration("1,5h"), Some(90));
        assert_eq!(parse_duration("168h"), Some(MAX_MINUTES));
    }

    #[test]
    fn parse_duration_rejects_nonsense() {
        for bad in [
            "", "0", "0m", "h", "abc", "1h60", "-5", "1.01h", "169h", "1h-5", "5s", "1e3", "infh",
            "nanh",
        ] {
            assert_eq!(parse_duration(bad), None, "{bad:?} doveva essere rifiutata");
        }
    }

    #[test]
    fn compact_format_roundtrips() {
        for minutes in [1, 15, 45, 60, 90, 120, 125, MAX_MINUTES] {
            let text = format_duration_compact(minutes);
            assert_eq!(parse_duration(&text), Some(minutes), "{text}");
        }
    }

    #[test]
    fn parse_clock_forms() {
        assert_eq!(parse_clock("18:30"), Some((18, 30)));
        assert_eq!(parse_clock("8:05"), Some((8, 5)));
        assert_eq!(parse_clock("18.30"), Some((18, 30)));
        assert_eq!(parse_clock("18"), Some((18, 0)));
        assert_eq!(parse_clock(" 07:00 "), Some((7, 0)));
        for bad in [
            "24:00", "12:60", "", ":30", "12:", "123:00", "ab:cd", "1:2:3",
        ] {
            assert_eq!(parse_clock(bad), None, "{bad:?} doveva essere rifiutato");
        }
    }

    #[test]
    fn minutes_are_clamped_on_start() {
        let s = Session::start(
            Mode::System,
            Spec::Minutes { minutes: 0 },
            now(0, WALL_1400),
            &rome_summer(),
        );
        assert_eq!(s.remaining_ms(now(0, WALL_1400)), Some(60_000));
    }
}
