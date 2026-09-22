//! Diagnostica: "perché il PC non dorme?" e "perché si è svegliato?".
//!
//! Moka legge ciò che Windows sa già e lo spiega a parole, senza cambiare
//! niente. Quasi tutto si legge **senza amministratore**:
//!
//! - gli ultimi periodi di standby e sospensione dal registro di sistema
//!   (`Kernel-Power` 506/507 per lo standby moderno, `Power-Troubleshooter` 1
//!   per sospensione e ibernazione), letti come XML: i motivi sono **codici**,
//!   quindi non dipendono dalla lingua di Windows;
//! - i dispositivi che possono svegliare il PC (`DevicePowerEnumDevices`, la
//!   stessa fonte di `powercfg /devicequery wake_armed`);
//! - dopo quanto Windows sospende il PC, e se i timer di risveglio sono attivi.
//!
//! **Chi** tiene sveglio il PC adesso lo sa solo `powercfg /requests`, che
//! vuole l'amministratore: si esegue solo quando l'utente lo chiede, con il
//! prompt di Windows (trappola 18).

use std::collections::HashMap;

use chrono::{DateTime, Local, TimeZone};
use serde::Serialize;
use windows::core::{w, GUID, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows::Win32::Globalization::{MultiByteToWideChar, CP_OEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS};
use windows::Win32::System::EventLog::{
    EvtClose, EvtNext, EvtQuery, EvtQueryChannelPath, EvtQueryReverseDirection, EvtRender,
    EvtRenderEventXml, EVT_HANDLE,
};
use windows::Win32::System::Power::{
    DevicePowerClose, DevicePowerEnumDevices, DevicePowerOpen, DEVICEPOWER_FILTER_DEVICES_PRESENT,
    DEVICEPOWER_FILTER_HARDWARE, DEVICEPOWER_FILTER_WAKEENABLED,
};
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

use crate::i18n::{duration_label, t, tv, Lang};
use crate::lid;

const KERNEL_POWER: &str = "Microsoft-Windows-Kernel-Power";
const TROUBLESHOOTER: &str = "Microsoft-Windows-Power-Troubleshooter";
/// Uno standby più breve di così (lo schermo spento per qualche secondo) non
/// dice niente a nessuno.
const MIN_REST_MS: i64 = 60_000;

// ------------------------------------------------------------------ eventi

/// Un evento del registro di sistema, ridotto a ciò che serve.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub provider: String,
    pub id: u32,
    pub time_ms: i64,
    pub data: HashMap<String, String>,
}

impl Event {
    fn num(&self, name: &str) -> Option<i64> {
        self.data.get(name)?.trim().parse().ok()
    }

    fn flag(&self, name: &str) -> Option<bool> {
        match self.data.get(name)?.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }

    fn text(&self, name: &str) -> Option<String> {
        let v = self.data.get(name)?.trim();
        (!v.is_empty()).then(|| v.to_owned())
    }
}

/// L'XML di un evento come lo restituisce `EvtRender` (lo stesso di
/// `wevtutil qe /f:xml`). Tollera virgolette semplici e doppie.
pub fn parse_event(xml: &str) -> Option<Event> {
    let provider = attr_after(xml, "<Provider ", "Name")?;
    let id_start = xml.find("<EventID")?;
    let id_text = &xml[id_start..];
    let id: u32 = id_text[id_text.find('>')? + 1..id_text.find("</EventID>")?]
        .trim()
        .parse()
        .ok()?;
    let time = attr_after(xml, "<TimeCreated ", "SystemTime")?;
    let time_ms = parse_time(&time)?;

    let mut data = HashMap::new();
    let mut rest = xml;
    while let Some(pos) = rest.find("<Data Name=") {
        rest = &rest[pos + "<Data Name=".len()..];
        let quote = rest.chars().next()?;
        let name_end = rest[1..].find(quote)? + 1;
        let name = rest[1..name_end].to_owned();
        let after = &rest[name_end + 1..];
        if after.trim_start().starts_with("/>") {
            data.insert(name, String::new());
            continue;
        }
        let open = after.find('>')? + 1;
        let close = after.find("</Data>")?;
        data.insert(name, unescape(&after[open..close]));
        rest = &after[close..];
    }
    Some(Event {
        provider,
        id,
        time_ms,
        data,
    })
}

fn attr_after(xml: &str, tag: &str, name: &str) -> Option<String> {
    let start = xml.find(tag)? + tag.len();
    let tail = &xml[start..];
    let end = tail.find('>')?;
    let tag_body = &tail[..end];
    let key = format!("{name}=");
    let at = tag_body.find(&key)? + key.len();
    let quote = tag_body[at..].chars().next()?;
    let value = &tag_body[at + 1..];
    Some(unescape(&value[..value.find(quote)?]))
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn parse_time(s: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(s.trim())
        .ok()
        .map(|d| d.timestamp_millis())
}

/// Gli ultimi eventi di standby e sospensione, dal più recente.
pub fn recent_power_events(max: usize) -> Vec<Event> {
    let query = w!("*[System[(Provider[@Name='Microsoft-Windows-Kernel-Power'] and (EventID=506 or EventID=507)) or (Provider[@Name='Microsoft-Windows-Power-Troubleshooter'] and EventID=1)]]");
    let mut out = Vec::new();
    unsafe {
        let Ok(results) = EvtQuery(
            None,
            w!("System"),
            query,
            EvtQueryChannelPath.0 | EvtQueryReverseDirection.0,
        ) else {
            return out;
        };
        loop {
            let mut handles = [0isize; 32];
            let mut returned = 0u32;
            if EvtNext(results, &mut handles, 2000, 0, &mut returned).is_err() || returned == 0 {
                break;
            }
            for &h in &handles[..returned as usize] {
                let ev = EVT_HANDLE(h);
                if out.len() < max {
                    if let Some(e) = render_xml(ev).as_deref().and_then(parse_event) {
                        out.push(e);
                    }
                }
                let _ = EvtClose(ev);
            }
            if out.len() >= max {
                break;
            }
        }
        let _ = EvtClose(results);
    }
    out
}

unsafe fn render_xml(ev: EVT_HANDLE) -> Option<String> {
    let mut used = 0u32;
    let mut count = 0u32;
    // Prima chiamata: quanto spazio serve (fallisce apposta).
    let _ = EvtRender(
        None,
        ev,
        EvtRenderEventXml.0,
        0,
        None,
        &mut used,
        &mut count,
    );
    if used == 0 {
        return None;
    }
    let mut buf = vec![0u16; used as usize / 2 + 1];
    EvtRender(
        None,
        ev,
        EvtRenderEventXml.0,
        (buf.len() * 2) as u32,
        Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
        &mut used,
        &mut count,
    )
    .ok()?;
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

// ------------------------------------------------------------ riposi

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestKind {
    Standby,
    Sleep,
    Hibernate,
}

/// Un periodo in cui il PC ha "dormito", con il perché è entrato e uscito.
#[derive(Debug, Clone, PartialEq)]
pub struct Rest {
    pub kind: RestKind,
    pub start_ms: i64,
    pub end_ms: i64,
    /// `POWER_MONITOR_REQUEST_REASON` dell'uscita dallo standby moderno.
    pub wake_reason: Option<u32>,
    /// Lo stesso codice all'entrata.
    pub enter_reason: Option<u32>,
    /// Sospensione classica: il dispositivo che l'ha svegliato, come lo scrive Windows.
    pub wake_source: Option<String>,
    /// Sospensione classica: chi aveva il timer di risveglio.
    pub wake_timer: Option<String>,
    /// Quanta parte dello standby è stata davvero a basso consumo.
    pub low_power_pct: Option<u32>,
    /// C'era un audio in riproduzione (tiene sveglio anche a schermo spento).
    pub audio: bool,
}

/// Dagli eventi (dal più recente) ai periodi di riposo, dal più recente.
pub fn rests(events: &[Event]) -> Vec<Rest> {
    let mut out = Vec::new();
    for (i, e) in events.iter().enumerate() {
        if e.provider == KERNEL_POWER && e.id == 507 {
            let Some(duration_us) = e.num("DurationInUs").filter(|d| *d > 0) else {
                continue;
            };
            let scenario = e.num("ScenarioInstanceIdV2");
            // L'entrata corrispondente è più vecchia, quindi più avanti.
            let enter = events[i + 1..]
                .iter()
                .filter(|o| o.provider == KERNEL_POWER && o.id == 506)
                .find(|o| scenario.is_none() || o.num("ScenarioInstanceIdV2") == scenario);
            let drips = e.num("DripsResidencyInUs").unwrap_or(0).max(0);
            out.push(Rest {
                kind: RestKind::Standby,
                start_ms: e.time_ms - duration_us / 1000,
                end_ms: e.time_ms,
                wake_reason: e.num("Reason").map(|r| r as u32),
                enter_reason: enter.and_then(|o| o.num("Reason")).map(|r| r as u32),
                wake_source: None,
                wake_timer: None,
                low_power_pct: Some(((drips * 100) / duration_us).clamp(0, 100) as u32),
                audio: e.flag("AudioPlaying").unwrap_or(false),
            });
        } else if e.provider == TROUBLESHOOTER && e.id == 1 {
            let (Some(start), Some(end)) = (
                e.text("SleepTime").and_then(|s| parse_time(&s)),
                e.text("WakeTime").and_then(|s| parse_time(&s)),
            ) else {
                continue;
            };
            out.push(Rest {
                kind: if e.num("TargetState") == Some(5) {
                    RestKind::Hibernate
                } else {
                    RestKind::Sleep
                },
                start_ms: start,
                end_ms: end,
                wake_reason: None,
                enter_reason: None,
                wake_source: e.text("WakeSourceText"),
                wake_timer: e.text("WakeTimerOwner"),
                low_power_pct: None,
                audio: false,
            });
        }
    }
    out.retain(|r| r.end_ms - r.start_ms >= MIN_REST_MS);
    out
}

/// Perché è uscito dallo standby, a parole. Codici di
/// `POWER_MONITOR_REQUEST_REASON` (winnt.h); quelli che non conosciamo con
/// certezza restano un numero, invece di un'ipotesi.
pub fn wake_reason_key(code: u32) -> Option<&'static str> {
    Some(match code {
        1 => "diag.woke_power_button",
        2 | 26 | 51 => "diag.woke_remote",
        4 | 36 | 37 | 39 => "diag.woke_input",
        5 | 28 => "diag.woke_power_source",
        8 | 48 => "diag.woke_app_display",
        10 => "diag.woke_unlock",
        14 => "diag.woke_sleep_button",
        15 => "diag.woke_lid",
        16 | 49 => "diag.woke_battery",
        18 => "diag.woke_device",
        22 | 52 => "diag.woke_presence",
        30 => "diag.woke_app",
        31 => "diag.woke_keyboard",
        32 => "diag.woke_mouse",
        33 => "diag.woke_touchpad",
        34 => "diag.woke_pen",
        44 => "diag.woke_fingerprint",
        54 => "diag.woke_touch",
        _ => return None,
    })
}

/// Perché è entrato in standby.
pub fn enter_reason_key(code: u32) -> Option<&'static str> {
    Some(match code {
        1 => "diag.entered_power_button",
        7 => "diag.entered_system",
        11 => "diag.entered_screen_off",
        12 | 21 => "diag.entered_idle",
        14 => "diag.entered_sleep_button",
        15 => "diag.entered_lid",
        23 => "diag.entered_thermal",
        53 => "diag.entered_battery",
        _ => return None,
    })
}

// -------------------------------------------------- possono svegliarlo

/// I dispositivi che possono svegliare il PC (come `powercfg /devicequery
/// wake_armed`, senza amministratore). Nomi come li mostra Windows.
pub fn wake_devices() -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        if !DevicePowerOpen(None) {
            return out;
        }
        let flags = DEVICEPOWER_FILTER_DEVICES_PRESENT
            | DEVICEPOWER_FILTER_HARDWARE
            | DEVICEPOWER_FILTER_WAKEENABLED;
        for index in 0..256 {
            let mut buf = [0u16; 512];
            let mut size = (buf.len() * 2) as u32;
            if !DevicePowerEnumDevices(
                index,
                flags,
                0,
                Some(buf.as_mut_ptr() as *mut u8),
                &mut size,
            ) {
                break;
            }
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            let name = String::from_utf16_lossy(&buf[..len]).trim().to_owned();
            if !name.is_empty() && !out.contains(&name) {
                out.push(name);
            }
        }
        let _ = DevicePowerClose();
    }
    out
}

const GUID_SLEEP_SUBGROUP: GUID = GUID::from_u128(0x238c9fa8_0aad_41ed_83f4_97be242c8f20);
const GUID_STANDBY_TIMEOUT: GUID = GUID::from_u128(0x29f6c1db_86da_48c5_9fdb_f2b67b1f44da);
const GUID_ALLOW_RTC_WAKE: GUID = GUID::from_u128(0xbd3b718a_0680_4d9d_8ab2_e1d2b4ac806d);

/// Le impostazioni dello schema attivo che spiegano "non dorme": dopo
/// quanti secondi di inattività sospende (0 = mai), e i timer di risveglio
/// (0 disattivati, 1 attivi, 2 solo quelli importanti).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SleepSettings {
    pub standby: Option<(u32, u32)>,
    pub wake_timers: Option<(u32, u32)>,
}

pub fn sleep_settings() -> SleepSettings {
    let Some(scheme) = lid::active_scheme() else {
        return SleepSettings::default();
    };
    SleepSettings {
        standby: lid::read_pair(&scheme, &GUID_SLEEP_SUBGROUP, &GUID_STANDBY_TIMEOUT),
        wake_timers: lid::read_pair(&scheme, &GUID_SLEEP_SUBGROUP, &GUID_ALLOW_RTC_WAKE),
    }
}

// ------------------------------------------- powercfg /requests (admin)

/// Una richiesta di `powercfg /requests`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// DISPLAY, SYSTEM, AWAYMODE, EXECUTION, PERFBOOST, ACTIVELOCKSCREEN.
    pub category: String,
    /// PROCESS, DRIVER, SERVICE (o altro, come lo scrive Windows).
    pub kind: String,
    pub name: String,
    pub reason: String,
}

/// L'uscita di `powercfg /requests`. Categorie (`SYSTEM:`) e tipi
/// (`[PROCESS]`) non sono tradotti da Windows; le righe "nessuno" sì, e si
/// ignorano senza doverle riconoscere.
pub fn parse_requests(text: &str) -> Vec<Request> {
    let mut out: Vec<Request> = Vec::new();
    let mut category = String::new();
    let mut current: Option<Request> = None;
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        if let Some(head) = l
            .strip_suffix(':')
            .filter(|h| h.len() >= 4 && h.chars().all(|c| c.is_ascii_uppercase()))
        {
            out.extend(current.take());
            category = head.to_owned();
            continue;
        }
        if let Some((kind, name)) = l
            .strip_prefix('[')
            .and_then(|rest| rest.split_once(']'))
            .filter(|(kind, _)| kind.chars().all(|c| c.is_ascii_uppercase()))
        {
            out.extend(current.take());
            current = Some(Request {
                category: category.clone(),
                kind: kind.to_owned(),
                name: name.trim().to_owned(),
                reason: String::new(),
            });
            continue;
        }
        if let Some(r) = current.as_mut() {
            if !r.reason.is_empty() {
                r.reason.push(' ');
            }
            r.reason.push_str(l);
        }
    }
    out.extend(current.take());
    out
}

/// Il nome da mostrare: il file per un processo, il nome del servizio fra
/// parentesi, il driver senza l'identificativo hardware.
pub fn display_name(r: &Request) -> String {
    match r.kind.as_str() {
        "PROCESS" => file_name(&r.name),
        "SERVICE" => match (r.name.rfind('('), r.name.ends_with(')')) {
            (Some(open), true) => r.name[open + 1..r.name.len() - 1].trim().to_owned(),
            _ => file_name(&r.name),
        },
        _ => match r.name.rfind(" (") {
            Some(cut) if r.name[cut..].contains('\\') => r.name[..cut].trim().to_owned(),
            _ => r.name.clone(),
        },
    }
}

fn file_name(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .trim()
        .to_owned()
}

/// L'uscita di un comando rediretto su file: UTF-8 se lo è (con `chcp
/// 65001`), altrimenti la tabella OEM della console (850 in italiano).
pub fn decode_output(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_owned();
    }
    unsafe {
        let len = MultiByteToWideChar(CP_OEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, None);
        if len <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut wide = vec![0u16; len as usize];
        MultiByteToWideChar(
            CP_OEMCP,
            MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
            bytes,
            Some(&mut wide),
        );
        String::from_utf16_lossy(&wide)
    }
}

#[derive(Debug)]
pub enum ElevatedError {
    /// L'utente ha detto no al prompt di Windows.
    Cancelled,
    /// Il prompt non è arrivato in tempo, o il comando non ha scritto niente.
    Failed(String),
}

/// `powercfg /requests` e `/waketimers` con l'amministratore: un solo
/// prompt di Windows per entrambi. Blocca finché l'utente risponde (al
/// massimo due minuti): va chiamata fuori dal thread principale.
pub fn elevated_powercfg() -> Result<(String, String), ElevatedError> {
    let dir = std::env::temp_dir().join(format!("moka-diag-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| ElevatedError::Failed(e.to_string()))?;
    let requests = dir.join("requests.txt");
    let timers = dir.join("waketimers.txt");
    let _ = std::fs::remove_file(&requests);
    let _ = std::fs::remove_file(&timers);
    let params = format!(
        "/d /c chcp 65001 >nul & powercfg /requests > \"{}\" 2>&1 & powercfg /waketimers > \"{}\" 2>&1",
        requests.display(),
        timers.display()
    );
    let params_w: Vec<u16> = params.encode_utf16().chain(std::iter::once(0)).collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: w!("runas"),
        lpFile: w!("cmd.exe"),
        lpParameters: PCWSTR(params_w.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe {
        if let Err(err) = ShellExecuteExW(&mut info) {
            // 0x800704C7: "L'operazione è stata annullata dall'utente".
            return Err(if err.code().0 as u32 == 0x8007_04C7 {
                ElevatedError::Cancelled
            } else {
                ElevatedError::Failed(err.message())
            });
        }
        if !info.hProcess.is_invalid() {
            let done = WaitForSingleObject(info.hProcess, 120_000) == WAIT_OBJECT_0;
            let _ = CloseHandle(info.hProcess);
            if !done {
                return Err(ElevatedError::Failed("timeout".into()));
            }
        }
    }
    let read = |p: &std::path::Path| std::fs::read(p).map(|b| decode_output(&b));
    let out = match (read(&requests), read(&timers)) {
        (Ok(r), Ok(t)) => Ok((r, t)),
        (Err(e), _) | (_, Err(e)) => Err(ElevatedError::Failed(e.to_string())),
    };
    let _ = std::fs::remove_dir_all(&dir);
    out
}

// ------------------------------------------------------------- a parole

/// "oggi alle 13:08", "ieri alle 22:14", "14/09 alle 06:31".
pub fn when_label(lang: Lang, wall_ms: i64) -> String {
    let Some(at) = Local.timestamp_millis_opt(wall_ms).single() else {
        return String::new();
    };
    let today = Local::now().date_naive();
    let time = at.format("%H:%M").to_string();
    let date = at.date_naive();
    if date == today {
        tv(lang, "diag.when_today", &[("time", &time)])
    } else if Some(date) == today.pred_opt() {
        tv(lang, "diag.when_yesterday", &[("time", &time)])
    } else {
        // "14/09" in italiano, "Sep 14" in inglese.
        let date = match lang {
            Lang::It => at.format("%d/%m").to_string(),
            Lang::En => at.format("%b %-d").to_string(),
        };
        tv(lang, "diag.when_date", &[("date", &date), ("time", &time)])
    }
}

/// "43 min", "5 h 3 min", "1 giorno", "3 giorni"; sotto il minuto "meno di 1 min".
pub fn span_label(lang: Lang, ms: i64) -> String {
    let minutes = (ms / 60_000).max(0) as u32;
    let days = minutes / (24 * 60);
    match (minutes, days) {
        (0, _) => t(lang, "time.less_than_minute"),
        (_, 0) => duration_label(lang, minutes),
        (_, 1) => t(lang, "diag.days_one"),
        (_, d) => tv(lang, "diag.days", &[("d", &d.to_string())]),
    }
}

/// Una frase della sezione "Adesso". `warn`: è probabilmente la risposta
/// alla domanda "perché non dorme?".
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NoteDto {
    pub text: String,
    pub warn: bool,
}

pub fn note(text: String, warn: bool) -> NoteDto {
    NoteDto { text, warn }
}

/// Dopo quanto Windows sospende il PC. "Mai" è la prima cosa da sapere
/// quando un PC non dorme: non c'entra nessun programma.
pub fn sleep_notes(lang: Lang, s: &SleepSettings, has_battery: bool) -> Vec<NoteDto> {
    let Some((ac, dc)) = s.standby else {
        return Vec::new();
    };
    let after = |secs: u32| duration_label(lang, secs.div_ceil(60).max(1));
    let mut out = Vec::new();
    if !has_battery {
        out.push(if ac == 0 {
            note(t(lang, "diag.now_never"), true)
        } else {
            note(
                tv(lang, "diag.now_sleep_after", &[("time", &after(ac))]),
                false,
            )
        });
        return out;
    }
    out.push(if ac == 0 {
        note(t(lang, "diag.now_never_ac"), true)
    } else {
        note(
            tv(lang, "diag.now_sleep_after_ac", &[("time", &after(ac))]),
            false,
        )
    });
    out.push(if dc == 0 {
        note(t(lang, "diag.now_never_dc"), true)
    } else {
        note(
            tv(lang, "diag.now_sleep_after_dc", &[("time", &after(dc))]),
            false,
        )
    });
    out
}

/// Se negli ultimi standby il PC è rimasto attivo per un audio aperto, lo
/// dice: è la causa più comune di un portatile caldo nello zaino.
pub fn audio_note(lang: Lang, rests: &[Rest]) -> Option<NoteDto> {
    let recent: Vec<&Rest> = rests
        .iter()
        .filter(|r| r.kind == RestKind::Standby)
        .take(3)
        .collect();
    let stuck = recent
        .iter()
        .filter(|r| r.audio && r.low_power_pct.is_some_and(|p| p < 50))
        .count();
    (!recent.is_empty() && stuck * 2 > recent.len()).then(|| note(t(lang, "diag.now_audio"), true))
}

/// Timer di risveglio: vale il più permissivo fra in carica e a batteria.
pub fn timers_note(lang: Lang, s: &SleepSettings) -> Option<String> {
    let (ac, dc) = s.wake_timers?;
    let key = if ac == 1 || dc == 1 {
        "diag.timers_on"
    } else if ac == 2 || dc == 2 {
        "diag.timers_important"
    } else {
        "diag.timers_off"
    };
    Some(t(lang, key))
}

/// Un riposo come lo vede la pagina.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RestDto {
    /// "Oggi alle 13:08, dopo 42 min di standby".
    pub title: String,
    /// "Si è svegliato: hai mosso il mouse."
    pub woke: String,
    /// "Era entrato in standby per inattività."
    pub entered: Option<String>,
    /// "Solo lo 0% del tempo a basso consumo: c'era un audio in riproduzione."
    pub note: Option<String>,
}

pub fn describe_rest(lang: Lang, r: &Rest) -> RestDto {
    let state = t(
        lang,
        match r.kind {
            RestKind::Standby => "diag.state_standby",
            RestKind::Sleep => "diag.state_sleep",
            RestKind::Hibernate => "diag.state_hibernate",
        },
    );
    let title = tv(
        lang,
        "diag.rest_title",
        &[
            ("when", &when_label(lang, r.end_ms)),
            ("duration", &span_label(lang, r.end_ms - r.start_ms)),
            ("state", &state),
        ],
    );
    let why = if let Some(owner) = &r.wake_timer {
        tv(lang, "diag.woke_timer", &[("owner", &file_name(owner))])
    } else if let Some(device) = &r.wake_source {
        tv(lang, "diag.woke_source", &[("device", device)])
    } else if let Some(code) = r.wake_reason {
        match wake_reason_key(code) {
            Some(key) => t(lang, key),
            None => tv(lang, "diag.woke_code", &[("code", &code.to_string())]),
        }
    } else {
        t(lang, "diag.woke_unknown")
    };
    let entered = r
        .enter_reason
        .and_then(enter_reason_key)
        .map(|key| tv(lang, "diag.rest_entered", &[("why", &t(lang, key))]));
    let long = r.end_ms - r.start_ms >= 10 * 60_000;
    let note = r.low_power_pct.filter(|p| long && *p < 50).map(|p| {
        let pct = p.to_string();
        if r.audio {
            tv(lang, "diag.rest_audio", &[("pct", &pct)])
        } else {
            tv(lang, "diag.rest_active", &[("pct", &pct)])
        }
    });
    RestDto {
        title,
        woke: tv(lang, "diag.rest_woke", &[("why", &why)]),
        entered,
        note,
    }
}

/// Una richiesta di `powercfg /requests` come la vede la pagina.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RequestDto {
    pub who: String,
    /// "programma", "driver", "servizio di Windows".
    pub kind: String,
    /// "tiene sveglio il PC", "tiene acceso lo schermo"…
    pub what: String,
    /// Il motivo come lo scrive chi ha fatto la richiesta.
    pub reason: String,
    pub hint: Option<String>,
    pub moka: bool,
}

const BROWSERS: [&str; 6] = [
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "opera.exe",
    "brave.exe",
    "vivaldi.exe",
];

/// `None` per le richieste che non c'entrano con il sonno (PERFBOOST).
pub fn describe_request(lang: Lang, r: &Request) -> Option<RequestDto> {
    let what = match r.category.as_str() {
        "DISPLAY" => t(lang, "diag.req_display"),
        "SYSTEM" => t(lang, "diag.req_system"),
        "AWAYMODE" => t(lang, "diag.req_away"),
        "EXECUTION" => t(lang, "diag.req_execution"),
        "ACTIVELOCKSCREEN" => t(lang, "diag.req_lockscreen"),
        "PERFBOOST" => return None,
        other => other.to_lowercase(),
    };
    let name = display_name(r);
    let lower = name.to_lowercase();
    let moka = r.kind == "PROCESS" && lower == "moka.exe";
    let kind = match r.kind.as_str() {
        "PROCESS" => t(lang, "diag.kind_process"),
        "DRIVER" => t(lang, "diag.kind_driver"),
        "SERVICE" => t(lang, "diag.kind_service"),
        other => other.to_lowercase(),
    };
    let audio = r.kind == "DRIVER"
        && (lower.contains("audio") || r.reason.to_lowercase().contains("audio"));
    let hint = if audio {
        Some(t(lang, "diag.hint_audio"))
    } else if BROWSERS.contains(&lower.as_str()) {
        Some(t(lang, "diag.hint_browser"))
    } else {
        None
    };
    Some(RequestDto {
        who: if moka { t(lang, "diag.req_moka") } else { name },
        kind,
        what,
        reason: r.reason.clone(),
        hint,
        moka,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTER_XML: &str = "<Event xmlns='http://schemas.microsoft.com/win/2004/08/events/event'><System><Provider Name='Microsoft-Windows-Kernel-Power' Guid='{331c3b3a-2005-44c2-ac5e-77220c37d6b4}'/><EventID>506</EventID><TimeCreated SystemTime='2026-09-22T10:25:52.4268844Z'/></System><EventData><Data Name='Reason'>12</Data><Data Name='LidOpenState'>true</Data><Data Name='ScenarioInstanceId'>73</Data><Data Name='ScenarioInstanceIdV2'>73</Data></EventData></Event>";
    const EXIT_XML: &str = "<Event xmlns='http://schemas.microsoft.com/win/2004/08/events/event'><System><Provider Name='Microsoft-Windows-Kernel-Power' Guid='{331c3b3a-2005-44c2-ac5e-77220c37d6b4}'/><EventID>507</EventID><TimeCreated SystemTime='2026-09-22T11:08:36.0120153Z'/></System><EventData><Data Name='DripsResidencyInUs'>0</Data><Data Name='DurationInUs'>2563560158</Data><Data Name='AudioPlaying'>true</Data><Data Name='Reason'>32</Data><Data Name='PowerStateAc'>true</Data><Data Name='ScenarioInstanceId'>75</Data><Data Name='ScenarioInstanceIdV2'>73</Data></EventData></Event>";
    const HIBERNATE_XML: &str = r#"<Event><System><Provider Name="Microsoft-Windows-Power-Troubleshooter"/><EventID Qualifiers="0">1</EventID><TimeCreated SystemTime="2026-09-14T08:31:39.6260000Z"/></System><EventData><Data Name="SleepTime">2026-09-14T01:28:41.3580309Z</Data><Data Name="WakeTime">2026-09-14T06:31:39.1206129Z</Data><Data Name="TargetState">5</Data><Data Name="WakeSourceType">0</Data><Data Name="WakeSourceText">
</Data><Data Name="WakeTimerOwner">\Device\HarddiskVolume3\Windows\System32\svchost.exe &amp; co</Data><Data Name="Empty"/></EventData></Event>"#;

    #[test]
    fn parses_event_xml() {
        let e = parse_event(EXIT_XML).unwrap();
        assert_eq!(e.provider, KERNEL_POWER);
        assert_eq!(e.id, 507);
        assert_eq!(e.num("Reason"), Some(32));
        assert_eq!(e.flag("AudioPlaying"), Some(true));
        assert_eq!(
            e.time_ms,
            parse_time("2026-09-22T11:08:36.012Z").unwrap(),
            "frazioni a 7 cifre"
        );

        let h = parse_event(HIBERNATE_XML).unwrap();
        assert_eq!(h.provider, TROUBLESHOOTER);
        assert_eq!(h.id, 1, "EventID con attributi");
        assert_eq!(h.text("WakeSourceText"), None, "solo spazi: vuoto");
        assert_eq!(
            h.text("WakeTimerOwner").as_deref(),
            Some(r"\Device\HarddiskVolume3\Windows\System32\svchost.exe & co")
        );
        assert_eq!(h.data.get("Empty").map(String::as_str), Some(""));
        assert!(parse_event("<Event/>").is_none());
    }

    #[test]
    fn pairs_standby_enter_and_exit() {
        // Dal più recente, come li legge Windows all'indietro.
        let events = [
            parse_event(EXIT_XML).unwrap(),
            parse_event(ENTER_XML).unwrap(),
            parse_event(HIBERNATE_XML).unwrap(),
        ];
        let r = rests(&events);
        assert_eq!(r.len(), 2);
        let standby = &r[0];
        assert_eq!(standby.kind, RestKind::Standby);
        assert_eq!(standby.wake_reason, Some(32));
        assert_eq!(
            standby.enter_reason,
            Some(12),
            "stesso ScenarioInstanceIdV2"
        );
        assert_eq!(standby.end_ms - standby.start_ms, 2_563_560);
        assert_eq!(standby.low_power_pct, Some(0));
        assert!(standby.audio);
        assert_eq!(r[1].kind, RestKind::Hibernate);
        assert!(r[1].wake_timer.is_some());
        assert_eq!(wake_reason_key(32), Some("diag.woke_mouse"));
        assert_eq!(enter_reason_key(12), Some("diag.entered_idle"));
        assert_eq!(wake_reason_key(999), None);
    }

    #[test]
    fn short_rests_are_noise() {
        let short = EXIT_XML.replace("2563560158", "20000000");
        assert!(rests(&[parse_event(&short).unwrap()]).is_empty());
    }

    const REQUESTS_EN: &str = "DISPLAY:\r\n[PROCESS] \\Device\\HarddiskVolume3\\Program Files\\Google\\Chrome\\Application\\chrome.exe\r\nVideo Wake Lock\r\n\r\nSYSTEM:\r\n[DRIVER] Realtek High Definition Audio(SST) (HDAUDIO\\FUNC_01&VEN_10EC&DEV_0256\\5&2c6b6ee&0&0001)\r\nAn audio stream is currently in use.\r\n[PROCESS] \\Device\\HarddiskVolume3\\Users\\me\\AppData\\Local\\Moka\\moka.exe\r\nMoka: sveglio per 2 h\r\n[SERVICE] \\Device\\HarddiskVolume3\\Windows\\System32\\svchost.exe (wuauserv)\r\nWindows Update\r\n\r\nAWAYMODE:\r\nNone.\r\n\r\nEXECUTION:\r\nNone.\r\n\r\nPERFBOOST:\r\nNone.\r\n\r\nACTIVELOCKSCREEN:\r\nNone.\r\n";

    #[test]
    fn parses_powercfg_requests() {
        let r = parse_requests(REQUESTS_EN);
        assert_eq!(r.len(), 4);
        assert_eq!(r[0].category, "DISPLAY");
        assert_eq!(display_name(&r[0]), "chrome.exe");
        assert_eq!(r[0].reason, "Video Wake Lock");
        assert_eq!(r[1].kind, "DRIVER");
        assert_eq!(display_name(&r[1]), "Realtek High Definition Audio(SST)");
        assert_eq!(display_name(&r[2]), "moka.exe");
        assert_eq!(r[2].category, "SYSTEM");
        assert_eq!(display_name(&r[3]), "wuauserv");
        // In italiano cambiano solo le righe "nessuno", che non servono.
        let it = REQUESTS_EN.replace("None.", "Nessuna.");
        assert_eq!(parse_requests(&it), r);
        assert!(parse_requests("DISPLAY:\nNessuna.\n\nSYSTEM:\nNessuna.\n").is_empty());
    }

    #[test]
    fn explains_rests_and_requests() {
        let events = [
            parse_event(EXIT_XML).unwrap(),
            parse_event(ENTER_XML).unwrap(),
        ];
        let r = describe_rest(Lang::It, &rests(&events)[0]);
        assert!(r.title.contains("42 min"), "{}", r.title);
        assert!(r.woke.contains("mouse"), "{}", r.woke);
        assert!(r.entered.unwrap().contains("inattività"));
        assert!(r.note.unwrap().contains("audio"));

        let reqs = parse_requests(REQUESTS_EN);
        let d: Vec<RequestDto> = reqs
            .iter()
            .filter_map(|q| describe_request(Lang::It, q))
            .collect();
        assert_eq!(d.len(), 4);
        assert!(d[0].hint.is_some(), "il browser ha il suo suggerimento");
        assert!(d[1].hint.is_some(), "il driver audio anche");
        assert!(d[2].moka);
        let boost = Request {
            category: "PERFBOOST".into(),
            kind: "PROCESS".into(),
            name: "x.exe".into(),
            reason: String::new(),
        };
        assert!(describe_request(Lang::It, &boost).is_none());
    }

    #[test]
    fn sleep_settings_in_words() {
        let never_ac = SleepSettings {
            standby: Some((0, 7200)),
            wake_timers: Some((0, 0)),
        };
        let n = sleep_notes(Lang::It, &never_ac, true);
        assert_eq!(n.len(), 2);
        assert!(n[0].warn, "mai in carica: è la risposta");
        assert!(n[1].text.contains("2 h"), "{}", n[1].text);
        assert_eq!(sleep_notes(Lang::It, &never_ac, false).len(), 1);
        assert!(timers_note(Lang::It, &never_ac).is_some());
        assert!(sleep_notes(Lang::It, &SleepSettings::default(), true).is_empty());
        assert_eq!(span_label(Lang::It, 5519 * 60_000), "3 giorni");
        assert_eq!(span_label(Lang::It, 1500 * 60_000), "1 giorno");
    }

    #[test]
    fn decodes_command_output() {
        assert_eq!(decode_output("perché".as_bytes()), "perché");
        assert_eq!(decode_output(b"\xEF\xBB\xBFok"), "ok");
        // "è" nella tabella OEM italiana (850) è 0x8A: non è UTF-8 valido.
        assert!(!decode_output(b"caff\x8A").contains('\u{FFFD}'));
    }
}
