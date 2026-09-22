//! Lo stato dell'app: impostazioni, memoria, sessione attiva e richiesta di
//! alimentazione, tutti dietro un solo Mutex. Ogni modifica passa da
//! [`Core::apply`], che porta la richiesta di alimentazione e il file su disco
//! allo stato nuovo: non esiste un modo di cambiare la sessione dimenticandosi
//! di uno dei due.

use std::path::PathBuf;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use chrono::{Local, Timelike};
use serde::Serialize;

use crate::i18n::{self, duration_label, Lang};
use crate::power::{Needs, PowerRequest};
use crate::session::{Mode, Now, Session, Spec};
use crate::settings::{Memory, SavedSession, Settings};
use crate::sys;
use crate::tray::{IconState, TrayMenu};

pub struct Paths {
    pub settings: PathBuf,
    pub state: PathBuf,
}

pub struct Core {
    pub settings: Settings,
    pub memory: Memory,
    pub session: Option<Session>,
    pub lang: Lang,
    pub version: String,
    power: PowerRequest,
    paths: Paths,
    logon_id: u64,
    /// Cresce a ogni modifica: il thread del timer lo usa per non perdersi una
    /// notifica arrivata mentre non stava aspettando.
    pub generation: u64,
}

impl Core {
    pub fn load(paths: Paths, version: String, now: Now) -> Core {
        let settings = Settings::load(&paths.settings);
        let memory = Memory::load(&paths.state);
        let logon_id = sys::logon_id();
        let session = memory.session.and_then(|s| s.resume(now, logon_id));
        let mut core = Core {
            lang: Lang::resolve(settings.language),
            settings,
            memory,
            session,
            version,
            power: PowerRequest::new(),
            paths,
            logon_id,
            generation: 0,
        };
        // Riattiva la sessione ripresa, oppure toglie dal file quella scartata.
        core.apply(now);
        core
    }

    /// Porta richiesta di alimentazione e `state.json` allo stato attuale.
    fn apply(&mut self, now: Now) {
        self.generation += 1;
        let (needs, reason) = match &self.session {
            None => (Needs::NONE, String::new()),
            Some(s) => (
                Needs {
                    system: true,
                    display: s.mode == Mode::Display,
                    execution: false,
                },
                i18n::power_reason(self.lang, s, now),
            ),
        };
        if let Err(err) = self.power.apply(needs, &reason) {
            eprintln!("moka: richiesta di alimentazione non applicata: {err}");
        }
        self.memory.session = self
            .session
            .map(|s| SavedSession::new(s, now, self.logon_id));
        self.save_memory();
    }

    fn save_memory(&self) {
        if let Err(err) = self.memory.save(&self.paths.state) {
            eprintln!("moka: state.json non salvato: {err}");
        }
    }

    pub fn save_settings(&mut self) -> std::io::Result<()> {
        self.settings.save(&self.paths.settings)?;
        let lang = Lang::resolve(self.settings.language);
        if lang != self.lang {
            self.lang = lang;
        }
        // Il motivo della richiesta è tradotto: si rifà nella lingua nuova.
        self.apply(sys::now());
        Ok(())
    }

    /// Accende. `None` = l'ultima scelta fatta.
    pub fn start(&mut self, mode: Option<Mode>, spec: Option<Spec>, now: Now) {
        let mode = mode.unwrap_or(self.memory.last_mode);
        let spec = spec.unwrap_or(self.memory.last_spec);
        self.memory.last_mode = mode;
        self.memory.last_spec = spec;
        self.session = Some(Session::start(mode, spec, now, &Local));
        self.apply(now);
    }

    pub fn stop(&mut self, now: Now) {
        self.session = None;
        self.apply(now);
    }

    pub fn toggle(&mut self, now: Now) {
        if self.session.is_some() {
            self.stop(now);
        } else {
            self.start(None, None, now);
        }
    }

    /// Cambia cosa tenere acceso: la sessione in corso, se c'è, e comunque la
    /// scelta per la prossima.
    pub fn set_mode(&mut self, mode: Mode, now: Now) {
        self.memory.last_mode = mode;
        if let Some(s) = &mut self.session {
            s.mode = mode;
        }
        self.apply(now);
    }

    /// Prepara "spegni lo schermo ora": il PC deve restare sveglio, lo schermo
    /// no. Senza sessione ne parte una finché non la si spegne (chi spegne lo
    /// schermo e se ne va non sa quando tornerà); con una sessione "PC e
    /// schermo" questa passa a "solo il PC", altrimenti lo schermo si
    /// riaccenderebbe e resterebbe acceso. La scelta per le prossime volte non
    /// cambia.
    pub fn prepare_screen_off(&mut self, now: Now) {
        match &mut self.session {
            None => {
                self.session = Some(Session::start(Mode::System, Spec::Never, now, &Local));
            }
            Some(s) => s.mode = Mode::System,
        }
        self.apply(now);
    }

    pub fn dismiss_welcome(&mut self) {
        self.memory.welcome_done = true;
        self.generation += 1;
        self.save_memory();
    }

    /// Chiude la sessione se è scaduta. `true` se è successo.
    pub fn expire_if_due(&mut self, now: Now) -> bool {
        if self.session.is_some_and(|s| s.is_expired(now)) {
            self.stop(now);
            return true;
        }
        false
    }

    /// Quanto può dormire il timer: fino alla scadenza o al prossimo cambio
    /// del minuto mostrato, e comunque non più di 15 s (dopo una sospensione
    /// l'attesa del sistema può allungarsi: così ci si riallinea presto).
    pub fn next_wait(&self, now: Now) -> Duration {
        let cap = 15_000;
        let ms = match self.session.and_then(|s| s.remaining_ms(now)) {
            None => cap,
            Some(r) => {
                let to_next_minute = match r % 60_000 {
                    0 => 60_000,
                    x => x,
                };
                r.min(to_next_minute + 20).min(cap)
            }
        };
        Duration::from_millis(ms.max(20))
    }

    pub fn view(&self, now: Now) -> View {
        let lang = self.lang;
        let session = self.session.as_ref();
        let status = i18n::status_label(lang, session, now);
        let active = session.is_some();
        let mode = session.map(|s| s.mode).unwrap_or(self.memory.last_mode);
        let icon = match session.map(|s| s.mode) {
            None => IconState::Off,
            Some(Mode::System) => IconState::System,
            Some(Mode::Display) => IconState::Display,
        };
        View {
            icon,
            tooltip: i18n::tooltip(lang, session, now),
            display: active && mode == Mode::Display,
            dto: StateDto {
                active,
                mode,
                status: status.clone(),
                remaining_ms: session.and_then(|s| s.remaining_ms(now)),
                spec: session.map(|s| s.spec),
                last_spec: self.memory.last_spec,
                durations: self
                    .settings
                    .durations
                    .iter()
                    .map(|&minutes| DurationDto {
                        minutes,
                        label: duration_label(lang, minutes),
                    })
                    .collect(),
                until_default: self.until_default(),
                lang,
                welcome: !self.memory.welcome_done,
                version: self.version.clone(),
            },
            status,
            active,
        }
    }

    /// L'orario proposto nel campo "fino alle": quello della sessione o
    /// dell'ultima volta, altrimenti fra un'ora arrotondato alla mezz'ora.
    fn until_default(&self) -> String {
        let chosen = self
            .session
            .map(|s| s.spec)
            .into_iter()
            .chain(std::iter::once(self.memory.last_spec))
            .find_map(|spec| match spec {
                Spec::Until { hour, minute } => Some((hour, minute)),
                _ => None,
            });
        let (h, m) = chosen.unwrap_or_else(|| {
            let t = Local::now() + chrono::Duration::minutes(60);
            let mut minutes = t.hour() * 60 + t.minute();
            minutes = minutes.div_ceil(30) * 30 % (24 * 60);
            (minutes / 60, minutes % 60)
        });
        format!("{h:02}:{m:02}")
    }
}

/// Ciò che serve per disegnare tray e pannello, calcolato sotto il lock e poi
/// usato fuori.
pub struct View {
    pub icon: IconState,
    pub tooltip: String,
    pub status: String,
    pub active: bool,
    pub display: bool,
    pub dto: StateDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DurationDto {
    pub minutes: u32,
    pub label: String,
}

/// Lo stato come lo vede il pannello. Tutti i testi arrivano già tradotti e
/// formattati: la pagina disegna, non calcola.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateDto {
    pub active: bool,
    pub mode: Mode,
    pub status: String,
    pub remaining_ms: Option<u64>,
    pub spec: Option<Spec>,
    pub last_spec: Spec,
    pub durations: Vec<DurationDto>,
    pub until_default: String,
    pub lang: Lang,
    pub welcome: bool,
    pub version: String,
}

/// Cache di ciò che è già stato mostrato nella tray, per non ridisegnarla
/// senza motivo. La tocca solo il thread principale.
#[derive(Default)]
pub struct UiCache {
    pub icon_key: Option<(IconState, bool, u32)>,
    pub tooltip: String,
    pub menu: Option<TrayMenu>,
    pub menu_key: Option<(Lang, Vec<u32>)>,
    pub menu_view: Option<(String, bool, bool)>,
}

pub struct AppState {
    pub core: Mutex<Core>,
    /// Sveglia il thread del timer quando la sessione cambia.
    pub wake: Condvar,
    pub ui: Mutex<UiCache>,
}
