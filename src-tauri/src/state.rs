//! Lo stato dell'app: impostazioni, memoria, sessione attiva, richiesta di
//! alimentazione e coperchio, tutti dietro un solo Mutex. Ogni modifica passa
//! da [`Core::apply`], che porta allo stato nuovo, insieme, l'impostazione del
//! coperchio, la richiesta di alimentazione e il file su disco: non esiste un
//! modo di cambiare la sessione dimenticandosi di uno dei tre.
//!
//! Le azioni sul sistema (sospendere, bloccare) non si eseguono qui dentro: si
//! accumulano in `effects` e le esegue `control`, **fuori** dal lock, perché
//! una sospensione ritorna solo al risveglio.

use std::path::PathBuf;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use chrono::{Local, Timelike};
use serde::Serialize;

use crate::capabilities::{self, Capabilities};
use crate::i18n::{self, duration_label, Lang};
use crate::lid::{self, LidAction};
use crate::lidoverride::{Overrides, WindowsBackend};
use crate::lidplan::{self, Effect, Force, Inputs, Tracker, World};
use crate::power::{Needs, PowerRequest};
use crate::session::{Mode, Now, Session, Spec};
use crate::settings::{LidMode, Memory, SavedSession, Settings};
use crate::sys;
use crate::sysevents::SysEvent;
use crate::tray::{IconState, TrayMenu};

pub struct Paths {
    pub settings: PathBuf,
    pub state: PathBuf,
    pub lid_log: PathBuf,
}

/// Qualcosa da dire all'utente con una notifica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// La sessione è finita per la soglia batteria.
    BatteryStopped(u8),
}

/// Tutto ciò che riguarda il portatile: coperchio, alimentazione, monitor.
pub struct LidRuntime {
    pub caps: Capabilities,
    overrides: Overrides<WindowsBackend>,
    tracker: Tracker,
    pub closed: bool,
    pub on_ac: bool,
    pub battery: Option<u8>,
    pub externals: u32,
    /// Un criterio aziendale impone l'azione del coperchio: la sezione si
    /// disattiva e lo dice, invece di fingere di funzionare (trappola 28).
    pub policy_managed: bool,
    /// "Ripristina ora": nessuna modifica fino alla prossima sessione.
    paused: bool,
    /// La soglia batteria scatta scendendo sotto il valore, non se la
    /// sessione è partita quando la batteria era già sotto (lo ha chiesto
    /// l'utente, sapendolo).
    battery_armed: bool,
    pub last_error: Option<String>,
}

impl LidRuntime {
    fn new(lid_log: PathBuf) -> LidRuntime {
        let caps = capabilities::read();
        let power = capabilities::power_status();
        let mut overrides = Overrides::load(WindowsBackend, lid_log);
        // Un registro lasciato lì vuol dire che l'esecuzione precedente è
        // caduta con la modifica attiva: si rimette tutto com'era, subito.
        if overrides.is_active() {
            overrides.restore_all();
        }
        let access = lid::access_check();
        LidRuntime {
            caps,
            overrides,
            tracker: Tracker::default(),
            closed: false,
            on_ac: power.on_ac.unwrap_or(true),
            battery: power.battery_percent,
            externals: capabilities::external_monitors(),
            policy_managed: caps.lid_present && !(access.ac && access.dc),
            paused: false,
            battery_armed: true,
            last_error: None,
        }
    }

    pub fn held(&self) -> bool {
        self.overrides.is_active()
    }

    /// L'impostazione di Windows com'era prima di Moka.
    pub fn original(&self) -> Option<LidAction> {
        self.overrides.original()
    }
}

pub struct Core {
    pub settings: Settings,
    pub memory: Memory,
    pub session: Option<Session>,
    pub lang: Lang,
    pub version: String,
    pub lid: LidRuntime,
    power: PowerRequest,
    paths: Paths,
    logon_id: u64,
    /// Azioni sul sistema da eseguire fuori dal lock.
    pub effects: Vec<Effect>,
    pub notices: Vec<Notice>,
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
            lid: LidRuntime::new(paths.lid_log.clone()),
            settings,
            memory,
            session,
            version,
            power: PowerRequest::new(),
            paths,
            logon_id,
            effects: Vec::new(),
            notices: Vec::new(),
            generation: 0,
        };
        // Riattiva la sessione ripresa, oppure toglie dal file quella scartata.
        core.apply(now);
        core
    }

    /// Porta coperchio, richiesta di alimentazione e `state.json` allo stato
    /// attuale.
    fn apply(&mut self, now: Now) {
        self.generation += 1;
        self.reconcile_lid(now);
        let (needs, reason) = match &self.session {
            None => (Needs::NONE, String::new()),
            Some(s) => (
                Needs {
                    system: true,
                    display: s.mode == Mode::Display,
                    // Ipotesi 1 dello spike (docs/SPIKE.md): a coperchio chiuso
                    // su standby moderno potrebbe servire anche questa. Non
                    // costa niente; lo spike dirà se basta, o se è inutile.
                    execution: self.lid.caps.modern_standby && self.lid.held(),
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

    /// Il modo del coperchio che vale adesso.
    pub fn lid_mode(&self) -> LidMode {
        if !self.lid.caps.lid_present || self.lid.policy_managed {
            LidMode::Windows
        } else {
            self.settings.effective_lid_mode()
        }
    }

    /// Porta l'impostazione del coperchio allo stato voluto e chiede alla
    /// logica del coperchio se c'è da fare qualcosa (vedi `lidplan`).
    fn reconcile_lid(&mut self, now: Now) {
        if !self.lid.caps.lid_present {
            return;
        }
        let mode = self.lid_mode();
        let inputs = Inputs {
            mode,
            session_lid: self.session.is_some_and(|s| s.lid),
            desk_mode: self.settings.lid_asked
                && self.settings.desk_mode
                && !self.lid.policy_managed,
            external_monitors: self.lid.externals,
        };
        let force = if self.lid.paused {
            Force::NONE
        } else {
            lidplan::desired(&inputs)
        };
        let applied = match self.lid.overrides.apply(force) {
            Ok(()) => {
                self.lid.last_error = None;
                force
            }
            Err(err) => {
                eprintln!("moka: coperchio non modificato: {err}");
                self.lid.last_error = Some(err);
                Force::NONE
            }
        };
        let original = self.lid.original().unwrap_or(LidAction { ac: 1, dc: 1 });
        let world = World {
            closed: self.lid.closed,
            on_ac: self.lid.on_ac,
            external_monitors: self.lid.externals,
            original,
            force: applied,
            backpack_minutes: self.settings.backpack_minutes,
            lock_on_open: self.settings.lock_on_lid_open,
        };
        if let Some(effect) = self.lid.tracker.update(&world, now.tick_ms) {
            self.effects.push(effect);
        }
    }

    /// Un evento di sistema (coperchio, alimentazione, batteria, monitor…).
    pub fn on_sys_event(&mut self, event: SysEvent, now: Now) {
        match event {
            SysEvent::Lid { closed } => self.lid.closed = closed,
            SysEvent::Power { on_ac } => self.lid.on_ac = on_ac,
            SysEvent::Battery { percent } => self.lid.battery = Some(percent),
            SysEvent::Displays => self.lid.externals = capabilities::external_monitors(),
            SysEvent::Resumed => {
                let power = capabilities::power_status();
                self.lid.on_ac = power.on_ac.unwrap_or(self.lid.on_ac);
                self.lid.battery = power.battery_percent.or(self.lid.battery);
                self.lid.externals = capabilities::external_monitors();
            }
            SysEvent::Scheme | SysEvent::Suspending => {}
            SysEvent::EndSession => {
                // Windows si spegne o l'utente esce: si rimette tutto adesso,
                // e non si riapplica più niente (il processo sta per finire).
                self.lid.overrides.restore_all();
                self.lid.paused = true;
                return;
            }
        }
        self.check_battery(now);
        self.apply(now);
    }

    /// Soglia batteria: sotto, la sessione finisce da sola e lo dice.
    fn check_battery(&mut self, now: Now) {
        let threshold = self.settings.battery_threshold;
        let Some(percent) = self.lid.battery else {
            return;
        };
        if threshold == 0 || self.lid.on_ac || u32::from(percent) > threshold {
            self.lid.battery_armed = true;
            return;
        }
        if self.session.is_some() && self.lid.battery_armed {
            self.lid.battery_armed = false;
            self.session = None;
            self.notices.push(Notice::BatteryStopped(percent));
            self.apply(now);
        }
    }

    fn arm_battery(&mut self) {
        let threshold = self.settings.battery_threshold;
        self.lid.battery_armed = threshold == 0
            || self.lid.on_ac
            || self.lid.battery.is_none_or(|b| u32::from(b) > threshold);
    }

    fn save_memory(&self) {
        if let Err(err) = self.memory.save(&self.paths.state) {
            eprintln!("moka: state.json non salvato: {err}");
        }
    }

    pub fn save_settings(&mut self) -> std::io::Result<()> {
        self.settings.save(&self.paths.settings)?;
        self.lang = Lang::resolve(self.settings.language);
        // Il motivo della richiesta è tradotto, e il coperchio dipende dalle
        // impostazioni: si rifà tutto.
        self.apply(sys::now());
        Ok(())
    }

    /// Accende. `None` = l'ultima scelta fatta.
    pub fn start(&mut self, mode: Option<Mode>, spec: Option<Spec>, lid: Option<bool>, now: Now) {
        let mode = mode.unwrap_or(self.memory.last_mode);
        let spec = spec.unwrap_or(self.memory.last_spec);
        let lid = lid.unwrap_or(self.memory.last_lid);
        self.memory.last_mode = mode;
        self.memory.last_spec = spec;
        self.memory.last_lid = lid;
        let mut session = Session::start(mode, spec, now, &Local);
        session.lid = lid;
        self.session = Some(session);
        self.lid.paused = false;
        self.arm_battery();
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
            self.start(None, None, None, now);
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

    /// "Anche a coperchio chiuso", per la sessione in corso e per le prossime.
    pub fn set_lid(&mut self, on: bool, now: Now) {
        self.memory.last_lid = on;
        if let Some(s) = &mut self.session {
            s.lid = on;
        }
        if on {
            self.lid.paused = false;
        }
        self.apply(now);
    }

    /// Risposta alla domanda del primo avvio su un portatile.
    pub fn answer_lid(&mut self, mode: LidMode, desk: bool) -> std::io::Result<()> {
        self.settings.lid_asked = true;
        self.settings.lid_mode = mode;
        self.settings.desk_mode = desk;
        self.save_settings()
    }

    /// "Ripristina ora": l'impostazione di Windows torna com'era adesso, e
    /// resta così fino alla prossima sessione.
    pub fn restore_lid_now(&mut self, now: Now) {
        self.lid.paused = true;
        if let Some(s) = &mut self.session {
            s.lid = false;
        }
        self.lid.overrides.restore_all();
        self.apply(now);
    }

    /// Chiusura dell'app: niente resta cambiato.
    pub fn shutdown(&mut self) {
        self.lid.overrides.restore_all();
        self.lid.paused = true;
        self.power.release();
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
                let mut s = Session::start(Mode::System, Spec::Never, now, &Local);
                s.lid = self.memory.last_lid;
                self.session = Some(s);
                self.arm_battery();
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

    /// Il giro del timer: sessione scaduta, e scadenze del coperchio (i 10 s
    /// di attesa, la protezione zaino). `true` se la sessione è finita.
    pub fn on_tick(&mut self, now: Now) -> bool {
        if self.session.is_some_and(|s| s.is_expired(now)) {
            self.stop(now);
            return true;
        }
        if self
            .lid
            .tracker
            .deadline()
            .is_some_and(|d| now.tick_ms >= d)
        {
            self.apply(now);
        }
        false
    }

    /// Quanto può dormire il timer: fino alla scadenza, al prossimo cambio del
    /// minuto mostrato o alla prossima scadenza del coperchio, e comunque non
    /// più di 15 s (dopo una sospensione l'attesa del sistema può allungarsi:
    /// così ci si riallinea presto).
    pub fn next_wait(&self, now: Now) -> Duration {
        let cap = 15_000;
        let mut ms = match self.session.and_then(|s| s.remaining_ms(now)) {
            None => cap,
            Some(r) => {
                let to_next_minute = match r % 60_000 {
                    0 => 60_000,
                    x => x,
                };
                r.min(to_next_minute + 20).min(cap)
            }
        };
        if let Some(d) = self.lid.tracker.deadline() {
            ms = ms.min(d.saturating_sub(now.tick_ms) + 20);
        }
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
        let laptop = self.lid.caps.lid_present;
        let lid_mode = self.lid_mode();
        View {
            icon,
            tooltip: i18n::tooltip(lang, session, now, self.lid.held() && active),
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
                laptop,
                lid_question: laptop && !self.settings.lid_asked && !self.lid.policy_managed,
                lid_row: laptop && lid_mode != LidMode::Windows,
                lid_mode,
                lid: session.map(|s| s.lid).unwrap_or(self.memory.last_lid),
                lid_held: self.lid.held(),
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
    /// C'è un coperchio.
    pub laptop: bool,
    /// Va ancora chiesto se tenere acceso a coperchio chiuso.
    pub lid_question: bool,
    /// Mostrare la riga "Anche a coperchio chiuso".
    pub lid_row: bool,
    /// Il modo che vale adesso (per dire "solo in carica" o "anche a batteria").
    pub lid_mode: LidMode,
    pub lid: bool,
    /// Moka sta tenendo l'impostazione del coperchio su "non fare nulla".
    pub lid_held: bool,
}

/// Cache di ciò che è già stato mostrato nella tray, per non ridisegnarla
/// senza motivo. La tocca solo il thread principale.
#[derive(Default)]
pub struct UiCache {
    pub icon_key: Option<(IconState, bool, u32)>,
    pub tooltip: String,
    pub menu: Option<TrayMenu>,
    pub menu_key: Option<(Lang, Vec<u32>, bool)>,
    pub menu_view: Option<(String, bool, bool, bool)>,
}

pub struct AppState {
    pub core: Mutex<Core>,
    /// Sveglia il thread del timer quando la sessione cambia.
    pub wake: Condvar,
    pub ui: Mutex<UiCache>,
}
