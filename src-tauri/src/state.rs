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

use chrono::{Datelike, Local, TimeZone, Timelike};
use serde::Serialize;

use crate::capabilities::{self, Capabilities};
use crate::i18n::{self, duration_label, remaining_label, t, then_label, tv, Lang};
use crate::lid::{self, LidAction};
use crate::lidoverride::{Overrides, WindowsBackend};
use crate::lidplan::{self, Effect, Force, Inputs, LidAct, Tracker, World};
use crate::power::{Needs, PowerRequest};
use crate::probes::Seen;
use crate::rules::{self, Engine, Observation, Rule, RuleKind};
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

/// Le regole sospese dall'utente ("Sospendi per un'ora").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulePause {
    /// Fino a questo tick (e a quest'ora, per dirla).
    Until { tick_ms: u64, wall_ms: i64 },
    /// Fino al prossimo avvio di Moka: non si salva su disco apposta.
    UntilRestart,
}

/// Una regola nata dalla riga di comando (`--while`, `--while-pid`): vive
/// solo finché il suo processo è vivo, e non si salva.
#[derive(Debug, Clone)]
pub struct TempRule {
    pub rule: Rule,
    /// Il processo è stato visto almeno una volta.
    pub seen: bool,
    pub created_tick: u64,
}

/// Perché una regola non è stata aggiunta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {
    Invalid,
    TooMany,
    Duplicate,
    Save(String),
}

/// Una regola da riga di comando il cui processo non compare entro 10 s si
/// scarta: probabilmente era già finito, o il nome era sbagliato.
const TEMP_GRACE_MS: u64 = 10_000;
/// Gli id delle regole temporanee partono da qui, lontano da quelle salvate.
const TEMP_ID_BASE: u32 = 1_000_000;

#[derive(Default)]
pub struct RulesRuntime {
    engine: Engine,
    /// Le regole che tengono sveglio il PC adesso (id).
    pub active: Vec<u32>,
    pub temp: Vec<TempRule>,
    pub paused: Option<RulePause>,
    next_temp: u32,
    /// La soglia batteria ha fermato le regole (per dirlo una volta sola).
    battery_notified: bool,
}

/// Qualcosa da dire all'utente con una notifica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// La sessione (o le regole) si sono fermate per la soglia batteria.
    BatteryStopped(u8),
    /// `--while NOME`: il programma non è comparso entro 10 secondi.
    WhileNotFound(String),
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
    pub rules: RulesRuntime,
    power: PowerRequest,
    paths: Paths,
    logon_id: u64,
    /// Azioni sul sistema da eseguire fuori dal lock.
    pub effects: Vec<Effect>,
    pub notices: Vec<Notice>,
    pub countdown: Option<Countdown>,
    /// Il conto alla rovescia viene dalla fine di questa regola.
    countdown_rule: Option<String>,
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
            rules: RulesRuntime::default(),
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
            countdown_rule: None,
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
        let rule_display = self.active_rules().any(|r| r.mode == Mode::Display);
        let (needs, reason) = match &self.session {
            // Nessuna sessione, ma una regola: sveglio per lei.
            None if !self.rules.active.is_empty() => (
                Needs {
                    system: true,
                    display: rule_display,
                    execution: self.lid.caps.modern_standby && self.lid.held(),
                },
                tv(
                    self.lang,
                    "reason.rule",
                    &[("reason", &self.reasons().join(", "))],
                ),
            ),
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
                    // Vale la modalità più forte fra sessione e regole.
                    display: s.mode == Mode::Display || rule_display,
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
            session_lid: self.session.is_some_and(|s| s.lid)
                || (!self.rules.active.is_empty() && self.memory.last_lid),
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
            self.rules.battery_notified = true;
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

    /// Accende o spegne. Se a tenere sveglio il PC è solo una regola,
    /// "spegni" vuol dire sospendere le regole per un'ora: altrimenti la regola
    /// riaccenderebbe tutto al controllo successivo. (Il pannello chiede prima.)
    pub fn toggle(&mut self, now: Now) {
        if self.session.is_some() {
            self.stop(now);
        } else if !self.rules.active.is_empty() {
            self.pause_rules(Some(60), now);
        } else {
            self.start(None, None, None, None, now);
        }
    }

    /// Il PC è tenuto sveglio da Moka (sessione o regola).
    pub fn awake(&self) -> bool {
        self.session.is_some() || !self.rules.active.is_empty()
    }

    /// Le regole salvate e quelle da riga di comando, in un solo elenco.
    pub fn all_rules(&self) -> impl Iterator<Item = &Rule> {
        self.settings
            .rules
            .iter()
            .chain(self.rules.temp.iter().map(|t| &t.rule))
    }

    fn active_rules(&self) -> impl Iterator<Item = &Rule> {
        self.all_rules()
            .filter(|r| self.rules.active.contains(&r.id))
    }

    /// "obs64.exe è aperto", per ogni regola che vale adesso.
    pub fn reasons(&self) -> Vec<String> {
        self.active_rules()
            .map(|r| i18n::rule_reason(self.lang, &r.kind))
            .collect()
    }

    /// Quali sonde servono al thread delle regole.
    pub fn rules_needs(&self) -> rules::Needs {
        rules::needs(self.all_rules())
    }

    /// Batteria sotto soglia, a batteria: le regole non tengono sveglio il PC.
    fn battery_block(&self) -> bool {
        let threshold = self.settings.battery_threshold;
        threshold > 0
            && !self.lid.on_ac
            && self.lid.battery.is_some_and(|b| u32::from(b) <= threshold)
    }

    /// Fa partire il "…e poi": 60 s con la finestra, o 10 s in silenzio a
    /// coperchio chiuso (nessuno vedrebbe la finestra).
    /// `rule`: la regola finita ("Programma aperto · notepad.exe"), per
    /// dirlo nella finestra; `None` = una sessione.
    fn begin_countdown(&mut self, act: ThenAct, mode: Mode, rule: Option<String>, now: Now) {
        self.countdown_rule = rule;
        let silent = self.lid.tracker.is_closed();
        self.countdown = Some(Countdown {
            act,
            at_tick: now.tick_ms
                + if silent {
                    lidplan::GRACE_MS
                } else {
                    COUNTDOWN_MS
                },
            silent,
            mode,
        });
    }

    /// Il giro del thread delle regole (ogni 5 s). `true` se è cambiato qualcosa.
    pub fn update_rules(&mut self, seen: Seen, now: Now) -> bool {
        let local = Local
            .timestamp_millis_opt(now.wall_ms)
            .single()
            .unwrap_or_else(Local::now);
        let obs = Observation {
            processes: seen.processes,
            pids: seen.pids,
            fullscreen: seen.fullscreen,
            call: seen.call,
            on_ac: self.lid.on_ac,
            external_monitors: self.lid.externals,
            net_kbps: seen.net_kbps,
            cpu_percent: seen.cpu_percent,
            usb: seen.usb,
            networks: seen.networks,
            weekday: local.weekday().num_days_from_monday() as u8,
            minute: (local.hour() * 60 + local.minute()) as u16,
        };

        let mut changed = false;
        if let Some(RulePause::Until { tick_ms, .. }) = self.rules.paused {
            if now.tick_ms >= tick_ms {
                self.rules.paused = None;
                changed = true;
            }
        }

        let rules: Vec<Rule> = self.all_rules().cloned().collect();
        let holding = self.rules.engine.evaluate(&rules, &obs, now.tick_ms);

        // Regole da riga di comando: vivono finché vive il loro processo.
        let mut finished: Vec<Rule> = Vec::new();
        let mut not_found: Vec<Rule> = Vec::new();
        self.rules.temp.retain_mut(|t| {
            if holding.contains(&t.rule.id) {
                t.seen = true;
                true
            } else if t.seen {
                finished.push(t.rule.clone());
                false
            } else if now.tick_ms.saturating_sub(t.created_tick) < TEMP_GRACE_MS {
                true
            } else {
                not_found.push(t.rule.clone());
                false
            }
        });
        for r in not_found {
            changed = true;
            let name = match r.kind {
                RuleKind::Process { exe } => exe,
                RuleKind::Pid { pid } => pid.to_string(),
                _ => continue,
            };
            self.notices.push(Notice::WhileNotFound(name));
        }
        if !finished.is_empty() {
            changed = true;
        }

        // Sotto la soglia batteria le regole non tengono sveglio il PC: è
        // automatico, l'utente non l'ha chiesto adesso.
        let blocked = self.battery_block();
        if blocked && !self.rules.active.is_empty() && !self.rules.battery_notified {
            self.rules.battery_notified = true;
            if let Some(p) = self.lid.battery {
                self.notices.push(Notice::BatteryStopped(p));
            }
        }
        if !blocked {
            self.rules.battery_notified = false;
        }
        let temp_ids: Vec<u32> = self.rules.temp.iter().map(|t| t.rule.id).collect();
        let active: Vec<u32> = if blocked {
            Vec::new()
        } else {
            holding
                .into_iter()
                .filter(|id| temp_ids.contains(id) || self.rules.paused.is_none())
                .collect()
        };

        if active == self.rules.active && !changed {
            return false;
        }
        let ended: Vec<Rule> = rules
            .iter()
            .filter(|r| self.rules.active.contains(&r.id) && !active.contains(&r.id))
            .cloned()
            .chain(finished)
            .collect();
        let gained = active.iter().any(|id| !self.rules.active.contains(id));
        self.rules.active = active;
        if gained && self.session.is_none() {
            // Qualcosa ha di nuovo bisogno del PC (il download è ripartito):
            // il "…e poi" in attesa non ha più senso.
            self.countdown = None;
        }
        // "…e poi" di una regola: solo se è finita da sé (non per la
        // batteria) e adesso niente tiene più sveglio il PC.
        if !blocked
            && self.session.is_none()
            && self.rules.active.is_empty()
            && self.countdown.is_none()
        {
            if let Some(r) = ended.iter().find(|r| r.then != ThenAct::None) {
                let title = i18n::rule_title(self.lang, &r.kind);
                let detail = i18n::rule_detail(self.lang, &r.kind);
                let label = if detail.is_empty() {
                    title
                } else {
                    format!("{title} · {detail}")
                };
                self.begin_countdown(r.then, r.mode, Some(label), now);
            }
        }
        self.apply(now);
        if self.countdown.is_some() {
            self.lid.tracker.cancel_released();
        }
        true
    }

    /// "Sospendi le regole": per `minutes` minuti, o fino al prossimo avvio.
    pub fn pause_rules(&mut self, minutes: Option<u32>, now: Now) {
        self.rules.paused = Some(match minutes {
            Some(m) => {
                let ms = u64::from(m.clamp(1, 24 * 60)) * 60_000;
                RulePause::Until {
                    tick_ms: now.tick_ms + ms,
                    wall_ms: now.wall_ms + ms as i64,
                }
            }
            None => RulePause::UntilRestart,
        });
        // "Sospendi" ferma tutto ciò che è automatico, anche un `--while` in
        // corso: chi spegne vuole il PC libero di dormire. Un `--while` chiesto
        // dopo, durante la pausa, vale invece (è una richiesta esplicita).
        self.rules.temp.clear();
        self.rules.active.clear();
        self.apply(now);
    }

    pub fn resume_rules(&mut self, now: Now) {
        self.rules.paused = None;
        self.apply(now);
    }

    /// `--while ffmpeg.exe` / `--while-pid 1234`.
    pub fn add_temp_rule(&mut self, kind: RuleKind, mode: Mode, then: ThenAct, now: Now) {
        self.rules.next_temp += 1;
        self.rules.temp.push(TempRule {
            rule: Rule {
                id: TEMP_ID_BASE + self.rules.next_temp,
                enabled: true,
                mode,
                then,
                kind,
            },
            seen: false,
            created_tick: now.tick_ms,
        });
        self.generation += 1;
    }

    /// Nuova regola salvata.
    pub fn add_rule(&mut self, kind: RuleKind, mode: Mode, then: ThenAct) -> Result<(), RuleError> {
        let kind = rules::validate(kind).ok_or(RuleError::Invalid)?;
        if self.settings.rules.len() >= rules::MAX_RULES {
            return Err(RuleError::TooMany);
        }
        // Due regole uguali non servono: si cambia quella che c'è.
        if self.settings.rules.iter().any(|r| r.kind == kind) {
            return Err(RuleError::Duplicate);
        }
        let id = rules::next_id(&self.settings.rules);
        self.settings.rules.push(Rule {
            id,
            enabled: true,
            mode,
            then,
            kind,
        });
        self.save_settings()
            .map_err(|e| RuleError::Save(e.to_string()))
    }

    pub fn update_rule(
        &mut self,
        id: u32,
        enabled: Option<bool>,
        mode: Option<Mode>,
        then: Option<ThenAct>,
    ) -> std::io::Result<()> {
        if let Some(r) = self.settings.rules.iter_mut().find(|r| r.id == id) {
            if let Some(e) = enabled {
                r.enabled = e;
                if !e {
                    self.rules.active.retain(|a| *a != id);
                }
            }
            if let Some(m) = mode {
                r.mode = m;
            }
            if let Some(t) = then {
                r.then = t;
            }
        }
        self.save_settings()
    }

    pub fn delete_rule(&mut self, id: u32) -> std::io::Result<()> {
        self.settings.rules.retain(|r| r.id != id);
        self.rules.active.retain(|a| *a != id);
        self.save_settings()
    }

    /// Presenza: F15 solo se è attiva e Moka sta tenendo sveglio il PC.
    pub fn presence_wanted(&self) -> bool {
        self.settings.presence && self.awake()
    }

    /// "Regole sospese fino alle 15:30" / "… fino al riavvio di Moka".
    pub fn rules_paused_label(&self) -> Option<String> {
        match self.rules.paused? {
            RulePause::UntilRestart => Some(t(self.lang, "rules.paused_restart")),
            RulePause::Until { wall_ms, .. } => Some(tv(
                self.lang,
                "rules.paused_until",
                &[("time", &i18n::clock_label(wall_ms))],
            )),
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
                self.begin_countdown(s.then, s.mode, None, now);
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
                body: match &self.countdown_rule {
                    Some(rule) => tv(lang, "toast.countdown_rule", &[("rule", rule)]),
                    None => t(lang, "toast.countdown_body"),
                },
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
        let reasons = self.reasons();
        let active = session.is_some();
        let awake = self.awake();
        let status = if session.is_none() && !reasons.is_empty() {
            tv(
                lang,
                "state.on_rule",
                &[("reason", &short_reasons(lang, &reasons))],
            )
        } else {
            i18n::status_label(lang, session, now)
        };
        let mode = session.map(|s| s.mode).unwrap_or(self.memory.last_mode);
        let rule_display = self.active_rules().any(|r| r.mode == Mode::Display);
        let icon = if !awake {
            IconState::Off
        } else if session.is_some_and(|s| s.mode == Mode::Display) || rule_display {
            IconState::Display
        } else {
            IconState::System
        };
        let laptop = self.lid.caps.lid_present;
        let lid_mode = self.lid_mode();
        View {
            icon,
            tooltip: if session.is_none() && awake {
                format!("Moka · {status}")
            } else {
                i18n::tooltip(lang, session, now, self.lid.held() && active)
            },
            display: icon == IconState::Display,
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
                awake,
                reasons,
                rules_paused: self.rules_paused_label(),
                has_rules: !self.settings.rules.is_empty(),
                display_on: icon == IconState::Display,
            },
            status,
            active: awake,
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
    /// Il PC è tenuto sveglio (sessione o regola); `active` è solo la sessione.
    pub awake: bool,
    /// Perché: "obs64.exe è aperto", per ogni regola attiva.
    pub reasons: Vec<String>,
    pub rules_paused: Option<String>,
    pub has_rules: bool,
    /// Anche lo schermo resta acceso adesso (sessione o regola).
    pub display_on: bool,
}

/// "obs64.exe è aperto", oppure "obs64.exe è aperto +2".
fn short_reasons(lang: Lang, reasons: &[String]) -> String {
    match reasons {
        [] => String::new(),
        [one] => one.clone(),
        [first, rest @ ..] => format!(
            "{first} {}",
            tv(lang, "rules.more", &[("n", &rest.len().to_string())])
        ),
    }
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
    pub menu_key: Option<(Lang, Vec<u32>, bool, bool)>,
    pub menu_view: Option<(String, bool, bool, bool, bool)>,
}

pub struct AppState {
    pub core: Mutex<Core>,
    /// Sveglia il thread del timer quando la sessione cambia.
    pub wake: Condvar,
    pub ui: Mutex<UiCache>,
}
