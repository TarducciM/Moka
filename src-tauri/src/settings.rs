//! Impostazioni (`settings.json`) e memoria dell'app (`state.json`), in
//! `%APPDATA%\com.moka.app\`.
//!
//! Tutto si legge **come se fosse ostile**: un file troncato, un campo del tipo
//! sbagliato o fuori intervallo tornano al valore predefinito, campo per campo,
//! senza mai rompere il resto. E una scadenza che il codice non avrebbe mai
//! potuto scrivere è un dato corrotto e si scarta: non basta "limitarla" in
//! visualizzazione (la lezione di QuietCycle: il clamp va sul dato, non su ciò
//! che si mostra).

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::session::{parse_duration, Mode, Now, Session, Spec, MAX_MINUTES};

pub const DEFAULT_DURATIONS: [u32; 5] = [15, 30, 60, 120, 240];
pub const MAX_DURATIONS: usize = 6;
/// Protezione zaino: dopo quanti minuti, a batteria e a coperchio chiuso senza
/// monitor, Moka fa ciò che Windows avrebbe fatto. 0 = mai.
pub const BACKPACK_CHOICES: [u32; 6] = [0, 10, 15, 30, 60, 120];
pub const DEFAULT_BACKPACK: u32 = 30;
/// Soglia batteria in percentuale: sotto, la sessione finisce. 0 = spenta.
pub const BATTERY_CHOICES: [u32; 8] = [0, 10, 15, 20, 25, 30, 40, 50];
pub const DEFAULT_BATTERY: u32 = 20;

/// Tolleranza nel riconoscere lo stesso avvio di Windows: copre le piccole
/// correzioni dell'orologio (NTP) fra un salvataggio e la rilettura.
const SAME_BOOT_TOLERANCE_MS: i64 = 120_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LangSetting {
    #[default]
    Auto,
    It,
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LeftClick {
    /// Il clic sinistro apre il pannello.
    #[default]
    Popover,
    /// Il clic sinistro accende o spegne con l'ultima scelta.
    Toggle,
}

/// Durante una sessione, chiudendo il coperchio…
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LidMode {
    /// …il PC va in sospensione, come sempre: Moka non tocca niente.
    Windows,
    /// …resta acceso, solo se è in carica (consigliato).
    #[default]
    Ac,
    /// …resta acceso anche a batteria.
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub language: LangSetting,
    pub left_click: LeftClick,
    /// Durate rapide in minuti: ordinate, senza doppioni, al massimo 6.
    pub durations: Vec<u32>,
    /// La domanda sul coperchio ha avuto risposta. Finché no, Moka non tocca
    /// l'impostazione di Windows: non la cambia senza consenso.
    pub lid_asked: bool,
    pub lid_mode: LidMode,
    /// Con un monitor esterno collegato, il coperchio chiuso non sospende mai
    /// il PC, anche senza una sessione.
    pub desk_mode: bool,
    /// Alla riapertura del coperchio, se Moka l'aveva tenuto acceso, blocca il PC.
    pub lock_on_lid_open: bool,
    pub backpack_minutes: u32,
    pub battery_threshold: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: LangSetting::Auto,
            left_click: LeftClick::Popover,
            durations: DEFAULT_DURATIONS.to_vec(),
            lid_asked: false,
            lid_mode: LidMode::Ac,
            desk_mode: false,
            lock_on_lid_open: true,
            backpack_minutes: DEFAULT_BACKPACK,
            battery_threshold: DEFAULT_BATTERY,
        }
    }
}

impl Settings {
    pub fn from_value(v: &Value) -> Settings {
        let d = Settings::default();
        Settings {
            language: field(v, "language").unwrap_or(d.language),
            left_click: field(v, "leftClick").unwrap_or(d.left_click),
            durations: v
                .get("durations")
                .and_then(Value::as_array)
                .map(|list| normalize_durations(list.iter().filter_map(Value::as_u64)))
                .unwrap_or(d.durations),
            lid_asked: bool_field(v, "lidAsked").unwrap_or(d.lid_asked),
            lid_mode: field(v, "lidMode").unwrap_or(d.lid_mode),
            desk_mode: bool_field(v, "deskMode").unwrap_or(d.desk_mode),
            lock_on_lid_open: bool_field(v, "lockOnLidOpen").unwrap_or(d.lock_on_lid_open),
            backpack_minutes: choice(v, "backpackMinutes", &BACKPACK_CHOICES)
                .unwrap_or(d.backpack_minutes),
            battery_threshold: choice(v, "batteryThreshold", &BATTERY_CHOICES)
                .unwrap_or(d.battery_threshold),
        }
    }

    /// Il modo del coperchio che vale davvero: finché la domanda non ha avuto
    /// risposta, "come sempre".
    pub fn effective_lid_mode(&self) -> LidMode {
        if self.lid_asked {
            self.lid_mode
        } else {
            LidMode::Windows
        }
    }

    pub fn load(path: &Path) -> Settings {
        Settings::from_value(&read_json(path))
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        write_json_atomic(path, &serde_json::to_value(self).expect("serializzabile"))
    }
}

/// Ordina, toglie doppioni e valori fuori intervallo, tiene le prime 6.
/// Una lista che resta vuota torna a quella predefinita.
pub fn normalize_durations(values: impl IntoIterator<Item = u64>) -> Vec<u32> {
    let mut list: Vec<u32> = values
        .into_iter()
        .filter(|m| (1..=u64::from(MAX_MINUTES)).contains(m))
        .map(|m| m as u32)
        .collect();
    list.sort_unstable();
    list.dedup();
    list.truncate(MAX_DURATIONS);
    if list.is_empty() {
        DEFAULT_DURATIONS.to_vec()
    } else {
        list
    }
}

/// Legge "15m, 45m, 1h30m". In caso di errore restituisce il pezzo che non si
/// capisce, per poterlo mostrare all'utente.
pub fn parse_durations_text(text: &str) -> Result<Vec<u32>, String> {
    let mut minutes = Vec::new();
    for part in text.split([',', ';']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match parse_duration(part) {
            Some(m) => minutes.push(u64::from(m)),
            None => return Err(part.to_owned()),
        }
    }
    if minutes.is_empty() {
        return Err(text.trim().to_owned());
    }
    Ok(normalize_durations(minutes))
}

/// Una sessione salvata su disco, con ciò che serve a capire se riprenderla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSession {
    pub session: Session,
    /// Vedi [`Now::boot_wall_ms`].
    pub boot_wall_ms: i64,
    /// Identificativo della sessione di accesso a Windows (LUID). Serve perché
    /// con l'avvio rapido "Arresta il sistema" non riavvia il kernel: il tick
    /// continua a contare e l'avvio sembrerebbe lo stesso. Il LUID invece cambia
    /// a ogni accesso.
    pub logon_id: u64,
}

impl SavedSession {
    pub fn new(session: Session, now: Now, logon_id: u64) -> SavedSession {
        SavedSession {
            session,
            boot_wall_ms: now.boot_wall_ms(),
            logon_id,
        }
    }

    /// La sessione si riprende solo dopo un riavvio **dell'app** (crash,
    /// aggiornamento): stesso avvio di Windows, stesso accesso, non scaduta, e
    /// con una scadenza che il codice avrebbe potuto scrivere.
    pub fn resume(&self, now: Now, logon_id: u64) -> Option<Session> {
        let same_boot = (self.boot_wall_ms - now.boot_wall_ms()).abs() <= SAME_BOOT_TOLERANCE_MS;
        if !same_boot || self.logon_id != logon_id {
            return None;
        }
        let s = self.session;
        if !spec_is_valid(s.spec) {
            return None;
        }
        // "Fino alle" può arrivare fino a ~24 h più il salto dell'ora legale.
        let max_ms = u64::from(MAX_MINUTES) * 60_000 + 25 * 3_600_000;
        match s.remaining_ms(now) {
            Some(0) => None,
            Some(r) if r > max_ms => None,
            _ => Some(s),
        }
    }
}

/// Ciò che l'app ricorda fra un avvio e l'altro, a parte le impostazioni.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    /// Ultima modalità scelta: la usa la prossima accensione.
    pub last_mode: Mode,
    /// Ultima scelta di "anche a coperchio chiuso" nel pannello.
    pub last_lid: bool,
    /// Ultima durata scelta: la usa l'accensione con un clic.
    pub last_spec: Spec,
    /// Il benvenuto al primo avvio è già stato chiuso.
    pub welcome_done: bool,
    pub session: Option<SavedSession>,
}

impl Default for Memory {
    fn default() -> Self {
        Memory {
            last_mode: Mode::default(),
            last_lid: true,
            last_spec: Spec::default(),
            welcome_done: false,
            session: None,
        }
    }
}

impl Memory {
    pub fn from_value(v: &Value) -> Memory {
        Memory {
            last_mode: field(v, "lastMode").unwrap_or_default(),
            last_lid: bool_field(v, "lastLid").unwrap_or(true),
            last_spec: field(v, "lastSpec")
                .filter(|s| spec_is_valid(*s))
                .unwrap_or_default(),
            welcome_done: v
                .get("welcomeDone")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            session: field(v, "session"),
        }
    }

    pub fn load(path: &Path) -> Memory {
        Memory::from_value(&read_json(path))
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        write_json_atomic(path, &serde_json::to_value(self).expect("serializzabile"))
    }
}

pub fn spec_is_valid(spec: Spec) -> bool {
    match spec {
        Spec::Never => true,
        Spec::Minutes { minutes } => (1..=MAX_MINUTES).contains(&minutes),
        Spec::Until { hour, minute } => hour < 24 && minute < 60,
    }
}

fn bool_field(v: &Value, key: &str) -> Option<bool> {
    v.get(key).and_then(Value::as_bool)
}

/// Un numero accettato solo se è una delle scelte offerte dall'interfaccia.
fn choice(v: &Value, key: &str, allowed: &[u32]) -> Option<u32> {
    v.get(key)
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| allowed.contains(n))
}

fn field<T: serde::de::DeserializeOwned>(v: &Value, key: &str) -> Option<T> {
    v.get(key)
        .and_then(|x| serde_json::from_value(x.clone()).ok())
}

fn read_json(path: &Path) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_object)
        .unwrap_or(Value::Object(Default::default()))
}

/// Scrive su un file temporaneo, lo forza su disco e poi lo rinomina: un
/// crash a metà lascia il file vecchio, mai uno troncato.
pub(crate) fn write_json_atomic(path: &Path, value: &Value) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::End;
    use serde_json::json;

    fn now(tick_ms: u64, wall_ms: i64) -> Now {
        Now { tick_ms, wall_ms }
    }

    #[test]
    fn hostile_settings_fall_back_field_by_field() {
        let s = Settings::from_value(&json!({
            "language": "klingon",
            "leftClick": "toggle",
            "durations": "15,30"
        }));
        assert_eq!(s.language, LangSetting::Auto);
        assert_eq!(s.left_click, LeftClick::Toggle);
        assert_eq!(s.durations, DEFAULT_DURATIONS.to_vec());

        assert_eq!(Settings::from_value(&json!(null)), Settings::default());
        assert_eq!(Settings::from_value(&json!([1, 2])), Settings::default());
    }

    #[test]
    fn lid_settings_are_normalized() {
        let s = Settings::from_value(&json!({
            "lidAsked": true,
            "lidMode": "always",
            "deskMode": "sì",
            "lockOnLidOpen": false,
            "backpackMinutes": 7,
            "batteryThreshold": 10
        }));
        assert!(s.lid_asked);
        assert_eq!(s.lid_mode, LidMode::Always);
        assert!(!s.desk_mode, "una stringa non è un booleano");
        assert!(!s.lock_on_lid_open);
        assert_eq!(
            s.backpack_minutes, DEFAULT_BACKPACK,
            "7 non è fra le scelte"
        );
        assert_eq!(s.battery_threshold, 10);
    }

    #[test]
    fn lid_mode_needs_consent() {
        let mut s = Settings {
            lid_mode: LidMode::Always,
            ..Settings::default()
        };
        assert_eq!(s.effective_lid_mode(), LidMode::Windows);
        s.lid_asked = true;
        assert_eq!(s.effective_lid_mode(), LidMode::Always);
    }

    #[test]
    fn durations_are_normalized() {
        let s = Settings::from_value(&json!({
            "durations": [240, 15, 15, 0, -3, 99999999, "30", 45, 60, 90, 120, 180]
        }));
        assert_eq!(s.durations, vec![15, 45, 60, 90, 120, 180]);
    }

    #[test]
    fn durations_text() {
        assert_eq!(
            parse_durations_text("1h30m, 15m,45 ; 2h"),
            Ok(vec![15, 45, 90, 120])
        );
        assert_eq!(parse_durations_text("15m, boh"), Err("boh".to_owned()));
        assert_eq!(parse_durations_text(" , "), Err(",".to_owned()));
    }

    fn sample_session() -> Session {
        Session {
            mode: Mode::Display,
            end: End::AfterTick {
                at_tick_ms: 3_600_000,
                total_ms: 3_600_000,
            },
            spec: Spec::Minutes { minutes: 60 },
            started_wall_ms: 1_000_000,
            lid: true,
        }
    }

    #[test]
    fn resume_only_in_same_boot_and_logon() {
        let saved = SavedSession::new(sample_session(), now(0, 1_000_000), 42);
        // Riavvio dell'app dieci minuti dopo: si riprende.
        let later = now(600_000, 1_600_000);
        assert_eq!(saved.resume(later, 42), Some(sample_session()));
        // Altro accesso a Windows (disconnessione, o arresto con avvio rapido).
        assert_eq!(saved.resume(later, 43), None);
        // Riavvio del PC: il tick è ripartito, l'avvio non coincide più.
        assert_eq!(saved.resume(now(60_000, 1_600_000), 42), None);
        // Scaduta nel frattempo.
        assert_eq!(saved.resume(now(3_600_000, 4_600_000), 42), None);
    }

    #[test]
    fn impossible_deadline_is_discarded() {
        let mut s = sample_session();
        s.end = End::AfterTick {
            at_tick_ms: u64::MAX / 4,
            total_ms: 3_600_000,
        };
        let saved = SavedSession::new(s, now(0, 1_000_000), 1);
        assert_eq!(saved.resume(now(1_000, 1_001_000), 1), None);
    }

    #[test]
    fn hostile_memory() {
        let m = Memory::from_value(&json!({
            "lastMode": "display",
            "lastSpec": { "kind": "minutes", "minutes": 0 },
            "welcomeDone": "yes",
            "session": { "session": 12 }
        }));
        assert_eq!(m.last_mode, Mode::Display);
        assert_eq!(m.last_spec, Spec::Never);
        assert!(!m.welcome_done);
        assert_eq!(m.session, None);
    }

    #[test]
    fn memory_roundtrip_on_disk() {
        let dir = std::env::temp_dir().join(format!("moka-test-{}", std::process::id()));
        let path = dir.join("state.json");
        let m = Memory {
            last_mode: Mode::Display,
            last_lid: false,
            last_spec: Spec::Until {
                hour: 18,
                minute: 30,
            },
            welcome_done: true,
            session: Some(SavedSession::new(sample_session(), now(5, 6), 7)),
        };
        m.save(&path).unwrap();
        assert_eq!(Memory::load(&path), m);
        // Un file troncato non rompe niente.
        fs::write(&path, "{\"lastMode\": \"disp").unwrap();
        assert_eq!(Memory::load(&path), Memory::default());
        let _ = fs::remove_dir_all(&dir);
    }
}
