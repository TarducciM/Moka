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
use crate::i18n::{self, duration_label, remaining_label, t, then_label, tv, Lang};
use crate::lid::{self, LidAction};
use crate::lidoverride::{Overrides, WindowsBackend};
use crate::lidplan::{self, Effect, Force, Inputs, LidAct, Tracker, World};
use crate::power::{Needs, PowerRequest};
use crate::session::{Mode, Now, Session, Spec, ThenAct};
use crate::settings::{LidMode, Memory, SavedSession, Settings};
use crate::sys;
use crate::sysevents::SysEvent;
use crate::tray::{IconState, TrayMenu};

/// Il conto alla rovescia di "…e poi".
pub const COUNTDOWN_MS: u64 = 60_000;
/// Una sessione scaduta da più di così ha finito mentre il PC dormiva: niente
/// azione, per non spegnere il portatile appena lo si riapre.
const LATE_MS: u64 = 30_000;
/// L'avviso compare quando mancano 5 minuti, e solo per sessioni più lunghe
/// di 10 (su una sessione da 15 minuti è utile, su una da 5 sarebbe rumore).
const WARN_BEFORE_MS: u64 = 5 * 60_000;
const WARN_MIN_TOTAL_MS: u64 = 10 * 60_000;
/// Quanto resta visibile l'avviso se nessuno lo tocca.
const WARN_SHOWN_MS: u64 = 60_000;
const DAY_MS: i64 = 86_400_000;

/// "…e poi" in attesa di partire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Countdown {
    pub act: ThenAct,
    pub at_tick: u64,
    /// A coperchio chiuso nessuno vede il conto alla rovescia: si aspettano
    /// solo i 10 s di garanzia, senza finestra.
    pub silent: bool,
    /// La modalità della sessione finita, per "+30 min".
    pub mode: Mode,
}

/// L'azione di sistema per un "…e poi".
pub fn then_effect(act: ThenAct) -> Option<Effect> {
    match act {
        ThenAct::None => None,
        ThenAct::ScreenOff => Some(Effect::ScreenOff),
        ThenAct::Lock => Some(Effect::Lock),
        ThenAct::Sleep => Some(Effect::Perform(LidAct::Sleep)),
        ThenAct::Hibernate => Some(Effect::Perform(LidAct::Hibernate)),
        ThenAct::Shutdown => Some(Effect::Perform(LidAct::Shutdown)),
    }
}

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
    pub countdown: Option<Countdown>,
    /// L'avviso "si spegne tra 5 minuti" resta visibile fino a questo tick.
    pub warning_until: Option<u64>,
    warned: bool,
    /// Una versione nuova pronta da installare (la trova `updates`).
    pub update_version: Option<String>,
    /// Un tasto rapido che non si è potuto registrare (preso da un'altra app).
    pub shortcut_error: Option<String>,
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
            countdown: None,
            warning_until: None,
            warned: false,
            update_version: None,
            shortcut_error: None,
            generation: 0,
        };
        core.memory.launches = core.memory.launches.saturating_add(1);
        if core.memory.first_seen_wall_ms <= 0 {
            core.memory.first_seen_wall_ms = now.wall_ms;
        }
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
            // Durante il conto alla rovescia il PC resta sveglio: "sveglio
            // finché finisce, poi spegni" non deve addormentarsi un minuto prima.
            None => match self.countdown {
                Some(c) => (
                    Needs {
                        system: true,
                        display: false,
                        execution: false,
                    },
                    tv(
                        self.lang,
                        "reason.then",
                        &[("action", &then_label(self.lang, c.act))],
                    ),
                ),
                None => (Needs::NONE, String::new()),
            },
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
            // Un "…e poi" in corso vince su ciò che Windows avrebbe fatto.
            let then_wins = self.countdown.is_some() && matches!(effect, Effect::Perform(_));
            if !then_wins {
                self.effects.push(effect);
            }
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
    pub fn start(
        &mut self,
        mode: Option<Mode>,
        spec: Option<Spec>,
        lid: Option<bool>,
        then: Option<ThenAct>,
        now: Now,
    ) {
        let mode = mode.unwrap_or(self.memory.last_mode);
        let spec = spec.unwrap_or(self.memory.last_spec);
        let lid = lid.unwrap_or(self.memory.last_lid);
        let then = then.unwrap_or(self.memory.next_then);
        self.memory.last_mode = mode;
        self.memory.last_spec = spec;
        self.memory.last_lid = lid;
        self.memory.next_then = then;
        let mut session = Session::start(mode, spec, now, &Local);
        session.lid = lid;
        session.then = then;
        self.session = Some(session);
        // Riaccendere annulla un "…e poi" in attesa e l'avviso.
        self.countdown = None;
        self.warning_until = None;
        self.warned = false;
        self.lid.paused = false;
        self.arm_battery();
        self.apply(now);
    }

    /// Spegne a mano: nessun "…e poi" (l'ha spenta l'utente, sa cosa vuole).
    pub fn stop(&mut self, now: Now) {
        self.session = None;
        self.countdown = None;
        self.warning_until = None;
        self.memory.next_then = ThenAct::None;
        self.apply(now);
    }

    /// "…e poi" per la sessione in corso, o per la prossima.
    pub fn set_then(&mut self, act: ThenAct, now: Now) {
        self.memory.next_then = act;
        if let Some(s) = &mut self.session {
            s.then = act;
        }
        self.apply(now);
    }

    /// "+30 min", dall'avviso o dal conto alla rovescia (che riaccende).
    pub fn extend(&mut self, minutes: u32, now: Now) {
        if let Some(s) = &mut self.session {
            s.extend(minutes, now);
            self.warned = false;
            self.warning_until = None;
            self.apply(now);
        } else if let Some(c) = self.countdown.take() {
            self.start(
                Some(c.mode),
                Some(Spec::Minutes { minutes }),
                None,
                Some(c.act),
                now,
            );
        }
    }

    pub fn cancel_countdown(&mut self, now: Now) {
        if self.countdown.take().is_some() {
            self.apply(now);
        }
    }

    /// "Sospendi ora": salta l'attesa.
    pub fn countdown_now(&mut self, now: Now) {
        if let Some(c) = self.countdown.take() {
            self.effects.extend(then_effect(c.act));
            self.apply(now);
        }
    }

    pub fn dismiss_warning(&mut self) {
        self.warning_until = None;
        self.generation += 1;
    }

    /// Il promemoria "metti una stella su GitHub": dopo 5 avvii e 3 giorni,
    /// mai se l'utente ha già risposto, mai insieme al benvenuto.
    pub fn star_due(&self, now: Now) -> bool {
        let m = &self.memory;
        !m.star_done
            && m.welcome_done
            && m.launches >= 5
            && m.first_seen_wall_ms > 0
            && now.wall_ms - m.first_seen_wall_ms >= 3 * DAY_MS
            && now.wall_ms >= m.star_snooze_until_ms
    }

    /// "Più tardi" (fra 14 giorni) o "non mostrare più".
    pub fn answer_star(&mut self, never: bool, now: Now) {
        if never {
            self.memory.star_done = true;
        } else {
            self.memory.star_snooze_until_ms = now.wall_ms + 14 * DAY_MS;
        }
        self.generation += 1;
        self.save_memory();
    }

    pub fn toggle(&mut self, now: Now) {
        if self.session.is_some() {
            self.stop(now);
        } else {
            self.start(None, None, None, None, now);
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
                self.countdown = None;
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

    /// Il giro del timer: sessione scaduta ("…e poi"), avviso dei 5 minuti,
    /// conto alla rovescia, scadenze del coperchio (i 10 s di attesa, la
    /// protezione zaino). `true` se qualcosa di visibile è cambiato.
    pub fn on_tick(&mut self, now: Now) -> bool {
        if let Some(s) = self.session.filter(|s| s.is_expired(now)) {
            self.session = None;
            self.warning_until = None;
            self.memory.next_then = ThenAct::None;
            // Se il PC ha dormito oltre la scadenza, la sessione finisce e
            // basta: niente spegnimento a sorpresa appena lo si riapre.
            if s.then != ThenAct::None && s.overdue_ms(now) <= LATE_MS {
                let silent = self.lid.tracker.is_closed();
                self.countdown = Some(Countdown {
                    act: s.then,
                    at_tick: now.tick_ms
                        + if silent {
                            lidplan::GRACE_MS
                        } else {
                            COUNTDOWN_MS
                        },
                    silent,
                    mode: s.mode,
                });
            }
            self.apply(now);
            if self.countdown.is_some() {
                self.lid.tracker.cancel_released();
            }
            return true;
        }
        let mut changed = false;
        if let Some(s) = self.session {
            if self.settings.warn_before_end && !self.warned {
                if let (Some(r), Some(total)) = (s.remaining_ms(now), s.total_ms()) {
                    if total > WARN_MIN_TOTAL_MS && r <= WARN_BEFORE_MS {
                        self.warned = true;
                        self.warning_until = Some(now.tick_ms + WARN_SHOWN_MS);
                        self.generation += 1;
                        changed = true;
                    }
                }
            }
        }
        if self.warning_until.is_some_and(|w| now.tick_ms >= w) {
            self.warning_until = None;
            self.generation += 1;
            changed = true;
        }
        if let Some(c) = self.countdown.filter(|c| now.tick_ms >= c.at_tick) {
            self.countdown = None;
            self.effects.extend(then_effect(c.act));
            self.apply(now);
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
        changed
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
        // Conto alla rovescia e avviso vanno seguiti al secondo.
        if self.countdown.is_some() || self.warning_until.is_some() {
            ms = ms.min(1_000);
        }
        // L'avviso dei 5 minuti deve comparire in tempo.
        if let (Some(s), false) = (self.session, self.warned) {
            if let Some(r) = s.remaining_ms(now) {
                if r > WARN_BEFORE_MS {
                    ms = ms.min(r - WARN_BEFORE_MS + 20);
                }
            }
        }
        Duration::from_millis(ms.max(20))
    }

    /// Cosa deve mostrare la finestrella degli avvisi, se qualcosa.
    pub fn toast(&self, now: Now) -> Option<ToastDto> {
        let lang = self.lang;
        if let Some(c) = self.countdown.filter(|c| !c.silent) {
            let seconds = c.at_tick.saturating_sub(now.tick_ms).div_ceil(1000);
            let label = then_label(lang, c.act);
            return Some(ToastDto {
                kind: "countdown",
                title: tv(lang, countdown_key(c.act), &[("s", &seconds.to_string())]),
                body: t(lang, "toast.countdown_body"),
                act: Some(tv(lang, "toast.now", &[("action", &label)])),
                seconds: Some(seconds),
                total_seconds: COUNTDOWN_MS / 1000,
                lang,
            });
        }
        if self.warning_until.is_some() {
            let s = self.session?;
            let remaining = s.remaining_ms(now)?;
            let body = if s.then == ThenAct::None {
                t(lang, "toast.warning_body")
            } else {
                tv(
                    lang,
                    "toast.warning_then",
                    &[("action", &then_label(lang, s.then))],
                )
            };
            return Some(ToastDto {
                kind: "warning",
                title: tv(
                    lang,
                    "toast.warning_title",
                    &[("time", &remaining_label(lang, remaining))],
                ),
                body,
                act: None,
                seconds: None,
                total_seconds: 0,
                lang,
            });
        }
        None
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
                then: session.map(|s| s.then).unwrap_or(self.memory.next_then),
                then_choices: ThenAct::ALL
                    .iter()
                    .map(|&act| ThenChoice {
                        value: act,
                        label: then_label(lang, act),
                    })
                    .collect(),
                // Con "finché non lo spegni" non c'è una fine: niente "…e poi".
                then_row: !session.is_some_and(|s| s.spec == Spec::Never),
                update: self.update_version.clone(),
                star: self.star_due(now),
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
    /// "…e poi" della sessione in corso, o della prossima.
    pub then: ThenAct,
    pub then_choices: Vec<ThenChoice>,
    pub then_row: bool,
    /// Versione nuova disponibile.
    pub update: Option<String>,
    /// Mostrare il promemoria stella.
    pub star: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThenChoice {
    pub value: ThenAct,
    pub label: String,
}

/// La finestrella degli avvisi: "si spegne tra 5 minuti" oppure il conto
/// alla rovescia di "…e poi".
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToastDto {
    pub kind: &'static str,
    pub title: String,
    pub body: String,
    /// Etichetta del pulsante "Sospendi ora".
    pub act: Option<String>,
    pub seconds: Option<u64>,
    pub total_seconds: u64,
    pub lang: Lang,
}

/// Scritte per esteso, non composte: così il test delle traduzioni le vede.
fn countdown_key(act: ThenAct) -> &'static str {
    match act {
        ThenAct::None | ThenAct::Sleep => "then.countdown_sleep",
        ThenAct::ScreenOff => "then.countdown_screen_off",
        ThenAct::Lock => "then.countdown_lock",
        ThenAct::Hibernate => "then.countdown_hibernate",
        ThenAct::Shutdown => "then.countdown_shutdown",
    }
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
