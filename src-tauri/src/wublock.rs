//! Blocca gli aggiornamenti di Windows mentre Moka tiene sveglio il PC, così
//! una copia lunga o un render non si trovano un riavvio a metà.
//!
//! Moka scrive gli **stessi valori** che scrive la pagina "Windows Update"
//! delle impostazioni quando si sceglie "Sospendi aggiornamenti", in
//! `HKLM\SOFTWARE\Microsoft\WindowsUpdate\UX\Settings`. Non serve
//! l'amministratore, e non è una scorciatoia: l'elenco di controllo di quella
//! chiave dà `SetValue` a `BUILTIN\Users` apposta, perché è la pagina delle
//! impostazioni — che gira come utente normale — a doverli scrivere
//! (verificato su LPT-MIKI il 2026-10-21, vedi `docs/ROADMAP.md`).
//!
//! Le regole sono quelle del coperchio (`lidoverride`), e per lo stesso
//! motivo: **non deve restare bloccato niente**.
//! - il registro (`updates-block.json`) si scrive e si forza su disco **prima**
//!   di toccare Windows, e si cancella solo a ripristino fatto;
//! - si rimette com'era **solo se il valore è ancora quello scritto da Moka**:
//!   se l'utente ha messo in pausa a modo suo nel frattempo, la sua scelta
//!   vince e il registro si scarta;
//! - finché la pausa è di Moka, `RunOnce` la toglie al prossimo accesso a
//!   Windows anche se Moka venisse uccisa;
//! - e comunque la pausa scritta **scade da sola** dopo un giorno: è l'ultima
//!   rete, quella che regge anche se tutte le altre saltano.
//!
//! Una pausa che c'era già (messa dall'utente o da un criterio aziendale) non
//! si tocca mai: Moka non la accorcia e non la allunga.
//!
//! Windows è dietro il trait [`Registry`]: nei test c'è un registro finto,
//! così ogni caso si prova senza toccare il PC.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use serde_json::Value;

use crate::session::Now;
use crate::settings::write_json_atomic;

pub const FILE_NAME: &str = "updates-block.json";

/// La chiave che scrive anche la pagina "Windows Update" delle impostazioni.
pub const KEY_PATH: &str = r"SOFTWARE\Microsoft\WindowsUpdate\UX\Settings";

/// I sei valori che Windows usa per "Sospendi aggiornamenti". Si scrivono
/// tutti insieme: scriverne solo una parte lascia uno stato che l'interfaccia
/// di Windows non sa più raccontare.
pub const VALUES: [&str; 6] = [
    "PauseUpdatesStartTime",
    "PauseUpdatesExpiryTime",
    "PauseFeatureUpdatesStartTime",
    "PauseFeatureUpdatesEndTime",
    "PauseQualityUpdatesStartTime",
    "PauseQualityUpdatesEndTime",
];

const EXPIRY: &str = "PauseUpdatesExpiryTime";

/// Quanto dura la pausa che Moka scrive. Breve di proposito: è la rete di
/// sicurezza che vale anche se Moka sparisce senza poter rimettere niente.
pub const WINDOW_HOURS: i64 = 24;
/// Sotto questa soglia la pausa si riscrive, così una sessione lunga non la
/// vede scadere sotto i piedi.
const REFRESH_UNDER_HOURS: i64 = 12;
/// Il massimo che Windows accetta. Una scadenza oltre questo limite non è
/// scrivibile da Moka, quindi in un registro è un dato corrotto: si scarta.
const MAX_PAUSE_DAYS: i64 = 35;

/// I valori della chiave: nome → contenuto (`None` = valore assente).
pub type Values = BTreeMap<String, Option<String>>;

/// Com'è andata l'ultima riconciliazione, per dirlo all'utente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Nessuna sessione, o opzione spenta: Moka non ha niente in pausa.
    Idle,
    /// La pausa è di Moka e scade a questo istante (UTC, ms dall'epoch).
    Ours { until_ms: i64 },
    /// Gli aggiornamenti erano **già** in pausa: Moka non li tocca.
    Foreign { until_ms: i64 },
    /// Non si è potuto scrivere (criterio aziendale, antivirus, chiave
    /// protetta): Moka lo dice invece di far credere di aver bloccato.
    Failed(String),
}

impl Status {
    /// L'etichetta che va nell'interfaccia: una parola sola, tradotta in JS.
    pub fn code(&self) -> &'static str {
        match self {
            Status::Idle => "idle",
            Status::Ours { .. } => "ours",
            Status::Foreign { .. } => "foreign",
            Status::Failed(_) => "failed",
        }
    }

    pub fn until_ms(&self) -> Option<i64> {
        match self {
            Status::Ours { until_ms } | Status::Foreign { until_ms } => Some(*until_ms),
            _ => None,
        }
    }

    pub fn error(&self) -> Option<&str> {
        match self {
            Status::Failed(e) => Some(e),
            _ => None,
        }
    }
}

/// Ciò che serve di Windows. Vedi [`WindowsRegistry`].
pub trait Registry {
    fn read(&self) -> Result<Values, String>;
    /// Scrive i valori: `None` cancella.
    fn write(&self, values: &Values) -> Result<(), String>;
    fn set_restore_on_logon(&self, on: bool);
}

fn to_iso(ms: i64) -> String {
    Utc.timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp_nanos(0))
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Legge una scadenza scritta da Windows. Accetta qualunque forma RFC 3339
/// (la pagina delle impostazioni scrive `2026-10-11T20:00:10Z`).
pub fn parse_iso(s: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(s.trim())
        .ok()
        .map(|d| d.with_timezone(&Utc).timestamp_millis())
}

/// Fino a quando gli aggiornamenti risultano in pausa, se lo sono.
pub fn paused_until(values: &Values) -> Option<i64> {
    values.get(EXPIRY)?.as_deref().and_then(parse_iso)
}

/// I sei valori per una pausa che parte adesso e scade a `until_ms`.
fn wanted(now_ms: i64, until_ms: i64) -> Values {
    let start = to_iso(now_ms);
    let end = to_iso(until_ms);
    VALUES
        .iter()
        .map(|name| {
            let v = if name.ends_with("StartTime") {
                start.clone()
            } else {
                end.clone()
            };
            ((*name).to_owned(), Some(v))
        })
        .collect()
}

/// `true` se i valori sono ancora quelli che Moka aveva scritto. Si guardano
/// solo i sei nostri: Windows può aggiungere altro nella stessa chiave.
fn still_ours(current: &Values, written: &Values) -> bool {
    VALUES
        .iter()
        .all(|n| current.get(*n).unwrap_or(&None) == written.get(*n).unwrap_or(&None))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Record {
    original: Values,
    written: Values,
}

impl Record {
    fn to_json(&self) -> Value {
        serde_json::json!({
            "version": 1,
            "original": map_to_json(&self.original),
            "written": map_to_json(&self.written),
        })
    }

    /// Legge il registro come se fosse ostile: senza i sei valori scritti, o
    /// con una scadenza che Moka non avrebbe mai potuto scrivere, non si sa
    /// cosa rimettere — quindi non si indovina, si scarta.
    fn from_json(v: &Value, now_ms: i64) -> Option<Record> {
        let original = map_from_json(v.get("original")?)?;
        let written = map_from_json(v.get("written")?)?;
        if VALUES.iter().any(|n| !written.contains_key(*n)) {
            return None;
        }
        let until = paused_until(&written)?;
        if until > now_ms + MAX_PAUSE_DAYS * 24 * 3_600_000 {
            return None;
        }
        Some(Record { original, written })
    }
}

fn map_to_json(m: &Values) -> Value {
    Value::Object(
        m.iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    v.clone().map(Value::String).unwrap_or(Value::Null),
                )
            })
            .collect(),
    )
}

fn map_from_json(v: &Value) -> Option<Values> {
    let obj = v.as_object()?;
    let mut out = Values::new();
    for (k, val) in obj {
        let item = match val {
            Value::Null => None,
            Value::String(s) => Some(s.clone()),
            // Un tipo che non è né testo né nullo non è roba nostra.
            _ => return None,
        };
        out.insert(k.clone(), item);
    }
    Some(out)
}

pub struct Block<R: Registry> {
    registry: R,
    path: PathBuf,
    record: Option<Record>,
    status: Status,
}

impl<R: Registry> Block<R> {
    pub fn load(registry: R, path: PathBuf) -> Self {
        let now_ms = Utc::now().timestamp_millis();
        let record = fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| Record::from_json(&v, now_ms));
        Block {
            registry,
            path,
            record,
            status: Status::Idle,
        }
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn is_active(&self) -> bool {
        self.record.is_some()
    }

    /// Porta il blocco allo stato voluto: lo mette se `want`, lo toglie se no.
    /// Non restituisce mai un errore: un blocco che non si può mettere non
    /// deve impedire a Moka di tenere sveglio il PC, si racconta e basta.
    pub fn apply(&mut self, want: bool, now: Now) {
        let now_ms = now.wall_ms;
        if !want {
            self.restore();
            self.status = Status::Idle;
            return;
        }
        let current = match self.registry.read() {
            Ok(v) => v,
            Err(err) => {
                self.status = Status::Failed(err);
                return;
            }
        };
        let target = now_ms + WINDOW_HOURS * 3_600_000;

        // Una pausa non nostra non si tocca mai: né per accorciarla né per
        // allungarla. Vale anche quando è più corta di quella che useremmo:
        // è una scelta di chi usa il PC, e la si rispetta.
        if self.record.is_none() {
            if let Some(until) = paused_until(&current) {
                if until > now_ms {
                    self.status = Status::Foreign { until_ms: until };
                    return;
                }
            }
        }

        if let Some(rec) = &self.record {
            if still_ours(&current, &rec.written) {
                let until = paused_until(&rec.written).unwrap_or(0);
                if until - now_ms > REFRESH_UNDER_HOURS * 3_600_000 {
                    // C'è già, e dura abbastanza: non si riscrive.
                    self.status = Status::Ours { until_ms: until };
                    return;
                }
            } else {
                // L'ha cambiata qualcun altro: la sua scelta vince, il nostro
                // registro non vale più niente.
                self.forget();
                self.status = Status::Foreign {
                    until_ms: paused_until(&current).unwrap_or(now_ms),
                };
                return;
            }
        }

        let original = match &self.record {
            // Si riscrive una pausa già nostra: l'originale resta quello vero,
            // non quello che ci abbiamo messo noi la volta prima.
            Some(rec) => rec.original.clone(),
            None => current
                .iter()
                .filter(|(k, _)| VALUES.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .chain(
                    VALUES
                        .iter()
                        .filter(|n| !current.contains_key(**n))
                        .map(|n| ((*n).to_owned(), None)),
                )
                .collect(),
        };
        let written = wanted(now_ms, target);
        let record = Record { original, written };

        // Il registro su disco **prima** di toccare Windows: se la corrente
        // va via fra le due righe, al prossimo avvio si sa cosa rimettere.
        if let Err(err) = self.persist(Some(&record)) {
            self.status = Status::Failed(err);
            return;
        }
        match self.registry.write(&record.written) {
            Ok(()) => {
                self.record = Some(record);
                self.status = Status::Ours { until_ms: target };
            }
            Err(err) => {
                // Niente è cambiato in Windows: via anche il registro, o
                // resterebbe a dire il falso.
                let _ = self.persist(None);
                self.status = Status::Failed(err);
            }
        }
    }

    /// Rimette com'era, se la pausa è ancora quella di Moka. `true` se l'ha
    /// tolta davvero.
    pub fn restore(&mut self) -> bool {
        let Some(rec) = self.record.take() else {
            let _ = self.persist(None);
            return false;
        };
        let restored = match self.registry.read() {
            Ok(current) if still_ours(&current, &rec.written) => {
                self.registry.write(&rec.original).is_ok()
            }
            // Cambiata da altri (o illeggibile): non si tocca.
            _ => false,
        };
        let _ = self.persist(None);
        restored
    }

    /// Dimentica la modifica senza toccare Windows.
    fn forget(&mut self) {
        self.record = None;
        let _ = self.persist(None);
    }

    fn persist(&self, record: Option<&Record>) -> Result<(), String> {
        match record {
            None => {
                let _ = fs::remove_file(&self.path);
                self.registry.set_restore_on_logon(false);
                Ok(())
            }
            Some(rec) => {
                write_json_atomic(&self.path, &rec.to_json()).map_err(|e| e.to_string())?;
                self.registry.set_restore_on_logon(true);
                Ok(())
            }
        }
    }
}

/// `moka --restore-updates`: toglie una pausa lasciata lì da un'esecuzione
/// precedente. Lo chiamano `RunOnce` al prossimo accesso e il disinstallatore.
pub fn restore_from_disk(dir: PathBuf) -> bool {
    Block::load(WindowsRegistry, dir.join(FILE_NAME)).restore()
}

/// Il secondo strato: dice a Windows che adesso **non** è il momento di
/// riavviare, con un motivo che l'utente legge nella schermata di arresto.
/// La pausa degli aggiornamenti impedisce che ne arrivino di nuovi; questo
/// copre quelli già scaricati, che aspettano solo il riavvio.
///
/// ⚠️ `ShutdownBlockReasonCreate` si può chiamare **solo dal thread che ha
/// creato la finestra** (altrimenti `ERROR_ACCESS_DENIED`): per questo passa
/// da `refresh`, che gira sul thread principale. E non è una garanzia: un
/// arresto forzato chiude lo stesso. Serve a non farsi riavviare di nascosto,
/// non a rendere il PC inarrestabile.
pub fn set_shutdown_block(hwnd: isize, reason: Option<&str>) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Shutdown::{ShutdownBlockReasonCreate, ShutdownBlockReasonDestroy};
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    unsafe {
        match reason {
            Some(text) => {
                let w = wide(text);
                ShutdownBlockReasonCreate(hwnd, windows::core::PCWSTR(w.as_ptr())).is_ok()
            }
            None => ShutdownBlockReasonDestroy(hwnd).is_ok(),
        }
    }
}

// ---------------------------------------------------------------- Windows ---

/// Il Windows vero.
pub struct WindowsRegistry;

const RUN_ONCE: windows::core::PCWSTR =
    windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce");

/// Il nome del valore in `RunOnce`, **uno per eseguibile**: due Moka insieme
/// (in sviluppo succede sempre) non devono cancellarsi la rete di sicurezza a
/// vicenda. Stessa impronta di `lidoverride`, nome diverso.
fn run_once_value() -> Vec<u16> {
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in exe.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("MokaRestoreUpdates-{hash:016x}")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

impl Registry for WindowsRegistry {
    fn read(&self) -> Result<Values, String> {
        use windows::Win32::System::Registry::{
            RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
            REG_VALUE_TYPE,
        };
        let path = wide(KEY_PATH);
        let mut key = HKEY::default();
        unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                windows::core::PCWSTR(path.as_ptr()),
                Some(0),
                KEY_READ,
                &mut key,
            )
            .ok()
            .map_err(|e| format!("chiave di Windows Update non leggibile: {e}"))?;
        }
        let mut out = Values::new();
        for name in VALUES {
            let n = wide(name);
            let mut kind = REG_VALUE_TYPE::default();
            let mut size: u32 = 0;
            let got = unsafe {
                RegQueryValueExW(
                    key,
                    windows::core::PCWSTR(n.as_ptr()),
                    None,
                    Some(&mut kind),
                    None,
                    Some(&mut size),
                )
            };
            if got.is_err() || size == 0 {
                out.insert(name.to_owned(), None);
                continue;
            }
            let mut buf = vec![0u8; size as usize];
            let got = unsafe {
                RegQueryValueExW(
                    key,
                    windows::core::PCWSTR(n.as_ptr()),
                    None,
                    Some(&mut kind),
                    Some(buf.as_mut_ptr()),
                    Some(&mut size),
                )
            };
            if got.is_err() {
                out.insert(name.to_owned(), None);
                continue;
            }
            let units: Vec<u16> = buf
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .take_while(|u| *u != 0)
                .collect();
            out.insert(name.to_owned(), Some(String::from_utf16_lossy(&units)));
        }
        unsafe {
            let _ = RegCloseKey(key);
        }
        Ok(out)
    }

    fn write(&self, values: &Values) -> Result<(), String> {
        use windows::Win32::System::Registry::{
            RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_LOCAL_MACHINE,
            KEY_SET_VALUE, REG_SZ,
        };
        let path = wide(KEY_PATH);
        let mut key = HKEY::default();
        unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                windows::core::PCWSTR(path.as_ptr()),
                Some(0),
                KEY_SET_VALUE,
                &mut key,
            )
            .ok()
            .map_err(|e| format!("chiave di Windows Update non scrivibile: {e}"))?;
        }
        let mut failed: Option<String> = None;
        for name in VALUES {
            let n = wide(name);
            let n = windows::core::PCWSTR(n.as_ptr());
            let res = match values.get(name).and_then(|v| v.clone()) {
                Some(text) => unsafe {
                    let w = wide(&text);
                    let bytes = std::slice::from_raw_parts(w.as_ptr() as *const u8, w.len() * 2);
                    RegSetValueExW(key, n, None, REG_SZ, Some(bytes)).ok()
                },
                None => unsafe {
                    // Cancellare un valore che non c'è è il risultato voluto.
                    let _ = RegDeleteValueW(key, n);
                    Ok(())
                },
            };
            if let Err(err) = res {
                failed.get_or_insert_with(|| format!("{name}: {err}"));
            }
        }
        unsafe {
            let _ = RegCloseKey(key);
        }
        match failed {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    fn set_restore_on_logon(&self, on: bool) {
        use windows::Win32::System::Registry::{
            RegCloseKey, RegCreateKeyExW, RegDeleteKeyValueW, RegSetValueExW, HKEY,
            HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
        };
        let value = run_once_value();
        let value = windows::core::PCWSTR(value.as_ptr());
        unsafe {
            if !on {
                let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_ONCE, value);
                return;
            }
            let exe = match std::env::current_exe() {
                Ok(exe) => exe,
                Err(err) => {
                    eprintln!("moka: RunOnce non scritto, eseguibile ignoto: {err}");
                    return;
                }
            };
            let command = format!("\"{}\" --restore-updates", exe.display());
            let w = wide(&command);
            let bytes = std::slice::from_raw_parts(w.as_ptr() as *const u8, w.len() * 2);
            let mut key = HKEY::default();
            if let Err(err) = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                RUN_ONCE,
                None,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut key,
                None,
            )
            .ok()
            {
                eprintln!("moka: RunOnce non aperto: {err}");
                return;
            }
            if let Err(err) = RegSetValueExW(key, value, None, REG_SZ, Some(bytes)).ok() {
                eprintln!("moka: RunOnce non scritto: {err}");
            }
            let _ = RegCloseKey(key);
        }
    }
}

// ------------------------------------------------------------------ test ---

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const HOUR: i64 = 3_600_000;

    fn now(ms: i64) -> Now {
        Now {
            tick_ms: 0,
            wall_ms: ms,
        }
    }

    #[derive(Default)]
    struct Fake {
        values: RefCell<Values>,
        writable: bool,
        readable: bool,
        run_once: RefCell<bool>,
        writes: RefCell<usize>,
    }

    impl Fake {
        fn new() -> Fake {
            Fake {
                values: RefCell::new(VALUES.iter().map(|n| ((*n).to_owned(), None)).collect()),
                writable: true,
                readable: true,
                ..Default::default()
            }
        }
        fn paused_until(&self, iso: &str) -> &Fake {
            let mut v = self.values.borrow_mut();
            for name in VALUES {
                v.insert(
                    name.to_owned(),
                    Some(if name.ends_with("StartTime") {
                        "2020-01-01T00:00:00Z".to_owned()
                    } else {
                        iso.to_owned()
                    }),
                );
            }
            drop(v);
            self
        }
    }

    impl Registry for &Fake {
        fn read(&self) -> Result<Values, String> {
            if !self.readable {
                return Err("chiusa".into());
            }
            Ok(self.values.borrow().clone())
        }
        fn write(&self, values: &Values) -> Result<(), String> {
            if !self.writable {
                return Err("criterio aziendale".into());
            }
            *self.writes.borrow_mut() += 1;
            let mut cur = self.values.borrow_mut();
            for (k, v) in values {
                cur.insert(k.clone(), v.clone());
            }
            Ok(())
        }
        fn set_restore_on_logon(&self, on: bool) {
            *self.run_once.borrow_mut() = on;
        }
    }

    fn block<'a>(fake: &'a Fake, dir: &tempdir::Dir) -> Block<&'a Fake> {
        Block::load(fake, dir.path().join(FILE_NAME))
    }

    /// Una cartella temporanea minima: il repo non ha `tempfile`.
    mod tempdir {
        use std::path::{Path, PathBuf};
        pub struct Dir(PathBuf);
        impl Dir {
            pub fn new(name: &str) -> Dir {
                let p = std::env::temp_dir().join(format!("moka-wublock-{name}"));
                let _ = std::fs::remove_dir_all(&p);
                std::fs::create_dir_all(&p).unwrap();
                Dir(p)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn mette_la_pausa_e_la_toglie_rimettendo_com_era() {
        let dir = tempdir::Dir::new("giro");
        let fake = Fake::new();
        let mut b = block(&fake, &dir);

        b.apply(true, now(1_000_000_000_000));
        assert_eq!(
            b.status(),
            &Status::Ours {
                until_ms: 1_000_000_000_000 + WINDOW_HOURS * HOUR
            }
        );
        assert!(b.is_active());
        assert!(*fake.run_once.borrow(), "la rete di sicurezza è armata");
        assert!(dir.path().join(FILE_NAME).exists());
        assert_eq!(
            fake.values.borrow().get(EXPIRY).unwrap().as_deref(),
            Some("2001-09-10T01:46:40Z")
        );

        b.apply(false, now(1_000_000_000_000 + HOUR));
        assert_eq!(b.status(), &Status::Idle);
        assert!(!b.is_active());
        assert!(!*fake.run_once.borrow());
        assert!(!dir.path().join(FILE_NAME).exists());
        // Tutti e sei tornati assenti, com'erano.
        assert!(fake.values.borrow().values().all(Option::is_none));
    }

    #[test]
    fn una_pausa_gia_presente_non_si_tocca() {
        let dir = tempdir::Dir::new("altrui");
        let fake = Fake::new();
        fake.paused_until("2030-01-01T00:00:00Z");
        let mut b = block(&fake, &dir);

        b.apply(true, now(1_000_000_000_000));
        assert!(matches!(b.status(), Status::Foreign { .. }));
        assert!(!b.is_active(), "niente registro: non abbiamo scritto noi");
        assert_eq!(*fake.writes.borrow(), 0);
        assert_eq!(
            fake.values.borrow().get(EXPIRY).unwrap().as_deref(),
            Some("2030-01-01T00:00:00Z")
        );
    }

    #[test]
    fn una_pausa_scaduta_non_conta_come_pausa() {
        let dir = tempdir::Dir::new("scaduta");
        let fake = Fake::new();
        fake.paused_until("2001-01-01T00:00:00Z");
        let mut b = block(&fake, &dir);

        b.apply(true, now(1_000_000_000_000));
        assert!(matches!(b.status(), Status::Ours { .. }));
        // E uscendo si rimette esattamente quella scaduta, non "niente".
        b.apply(false, now(1_000_000_000_000));
        assert_eq!(
            fake.values.borrow().get(EXPIRY).unwrap().as_deref(),
            Some("2001-01-01T00:00:00Z")
        );
    }

    #[test]
    fn chi_cambia_la_pausa_a_mano_vince() {
        let dir = tempdir::Dir::new("mano");
        let fake = Fake::new();
        let mut b = block(&fake, &dir);
        b.apply(true, now(1_000_000_000_000));
        assert!(b.is_active());

        // L'utente mette la sua pausa dalle impostazioni di Windows.
        fake.paused_until("2030-01-01T00:00:00Z");
        b.apply(true, now(1_000_000_000_000 + HOUR));
        assert!(matches!(b.status(), Status::Foreign { .. }));
        assert!(!b.is_active(), "il nostro registro si scarta");

        // E spegnendo la sessione la pausa dell'utente resta la sua.
        b.apply(false, now(1_000_000_000_000 + 2 * HOUR));
        assert_eq!(
            fake.values.borrow().get(EXPIRY).unwrap().as_deref(),
            Some("2030-01-01T00:00:00Z")
        );
    }

    #[test]
    fn non_riscrive_finche_la_pausa_dura_abbastanza() {
        let dir = tempdir::Dir::new("riscrittura");
        let fake = Fake::new();
        let mut b = block(&fake, &dir);
        b.apply(true, now(1_000_000_000_000));
        assert_eq!(*fake.writes.borrow(), 1);

        // Un'ora dopo: ne restano 23, non si tocca niente.
        b.apply(true, now(1_000_000_000_000 + HOUR));
        assert_eq!(*fake.writes.borrow(), 1);

        // A 13 ore ne restano 11: si riscrive, e l'originale resta quello vero.
        b.apply(true, now(1_000_000_000_000 + 13 * HOUR));
        assert_eq!(*fake.writes.borrow(), 2);
        b.apply(false, now(1_000_000_000_000 + 14 * HOUR));
        assert!(fake.values.borrow().values().all(Option::is_none));
    }

    #[test]
    fn se_windows_non_si_lascia_scrivere_lo_dice() {
        let dir = tempdir::Dir::new("negata");
        let fake = Fake {
            writable: false,
            ..Fake::new()
        };
        let mut b = block(&fake, &dir);
        b.apply(true, now(1_000_000_000_000));
        assert!(matches!(b.status(), Status::Failed(_)));
        assert!(!b.is_active());
        assert!(
            !dir.path().join(FILE_NAME).exists(),
            "niente registro se non è cambiato niente"
        );
        assert!(!*fake.run_once.borrow());
    }

    #[test]
    fn se_la_chiave_non_si_legge_non_si_inventa_niente() {
        let dir = tempdir::Dir::new("illeggibile");
        let fake = Fake {
            readable: false,
            ..Fake::new()
        };
        let mut b = block(&fake, &dir);
        b.apply(true, now(1_000_000_000_000));
        assert!(matches!(b.status(), Status::Failed(_)));
        assert_eq!(*fake.writes.borrow(), 0);
    }

    #[test]
    fn un_registro_lasciato_li_si_rilegge_e_si_ripristina() {
        let dir = tempdir::Dir::new("crash");
        let fake = Fake::new();
        {
            let mut b = block(&fake, &dir);
            b.apply(true, now(1_000_000_000_000));
        } // Moka muore qui, senza ripristinare niente.
        assert!(fake.values.borrow().get(EXPIRY).unwrap().is_some());

        // Al prossimo avvio (o da `--restore-updates`).
        let mut b = block(&fake, &dir);
        assert!(b.is_active());
        assert!(b.restore());
        assert!(fake.values.borrow().values().all(Option::is_none));
        assert!(!dir.path().join(FILE_NAME).exists());
    }

    #[test]
    fn un_registro_con_una_scadenza_impossibile_si_scarta() {
        let dir = tempdir::Dir::new("corrotto");
        let path = dir.path().join(FILE_NAME);
        let rec = serde_json::json!({
            "version": 1,
            "original": { "PauseUpdatesExpiryTime": null },
            "written": VALUES.iter().map(|n| ((*n).to_owned(), Value::String("2999-01-01T00:00:00Z".into()))).collect::<serde_json::Map<_, _>>(),
        });
        std::fs::write(&path, serde_json::to_string(&rec).unwrap()).unwrap();
        let fake = Fake::new();
        let b = Block::load(&fake, path);
        assert!(
            !b.is_active(),
            "una pausa di mille anni non l'ha scritta Moka"
        );
    }

    #[test]
    fn il_nome_in_run_once_e_solo_nostro() {
        let name = String::from_utf16_lossy(&run_once_value());
        let name = name.trim_end_matches('\0');
        assert!(name.starts_with("MokaRestoreUpdates-"), "{name}");
        assert_eq!(name.len(), "MokaRestoreUpdates-".len() + 16);
    }

    #[test]
    fn le_date_vanno_e_tornano_nel_formato_di_windows() {
        let ms = parse_iso("2026-10-11T20:00:10Z").unwrap();
        assert_eq!(to_iso(ms), "2026-10-11T20:00:10Z");
        // Anche la forma con l'offset, che la specifica ammette.
        assert_eq!(parse_iso("2026-10-11T22:00:10+02:00"), Some(ms));
        assert_eq!(parse_iso("non una data"), None);
    }
}
