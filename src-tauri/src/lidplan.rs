//! La logica del coperchio, pura: niente Windows, tick dall'esterno. È la
//! tabella "Cosa fa Moka da sola" della roadmap trasformata in codice, e ogni
//! sua riga ha un test qui sotto.
//!
//! Due domande separate:
//! 1. [`desired`]: quali valori dell'azione del coperchio forzare su "non fare
//!    nulla" in questo momento (in carica, a batteria, nessuno);
//! 2. [`Tracker::update`]: con il coperchio **già chiuso**, quando smette di
//!    valere il motivo per cui Moka lo teneva acceso, cosa avrebbe fatto
//!    Windows. Windows applica l'azione del coperchio solo nel momento della
//!    chiusura (trappola 22): rimettere l'impostazione dopo non sospende
//!    niente, e il portatile resterebbe acceso nello zaino. Quindi lo fa Moka.

use crate::lid::LidAction;
use crate::settings::LidMode;

/// Attesa prima di agire a coperchio chiuso: nessuno vede un conto alla
/// rovescia, ma il coperchio potrebbe riaprirsi proprio in quel momento.
pub const GRACE_MS: u64 = 10_000;

/// Quali valori tenere su "non fare nulla".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Force {
    pub ac: bool,
    pub dc: bool,
}

impl Force {
    pub const NONE: Force = Force {
        ac: false,
        dc: false,
    };

    pub fn any(self) -> bool {
        self.ac || self.dc
    }
}

pub struct Inputs {
    /// Già "Windows" se la domanda non ha avuto risposta o se l'impostazione è
    /// imposta da un criterio aziendale.
    pub mode: LidMode,
    /// C'è una sessione attiva con "anche a coperchio chiuso".
    pub session_lid: bool,
    pub desk_mode: bool,
    pub external_monitors: u32,
}

/// Più motivi possono valere insieme (una sessione e la modalità scrivania):
/// la modifica resta finché ne vale almeno uno.
pub fn desired(i: &Inputs) -> Force {
    let mut f = Force::NONE;
    if i.session_lid {
        match i.mode {
            LidMode::Windows => {}
            // Solo il valore in carica: quello a batteria resta di Windows,
            // che quindi gestisce da sé la batteria anche se Moka cadesse.
            LidMode::Ac => f.ac = true,
            LidMode::Always => {
                f.ac = true;
                f.dc = true;
            }
        }
    }
    if i.desk_mode && i.external_monitors > 0 {
        f.ac = true;
        f.dc = true;
    }
    f
}

/// Ciò che Windows fa chiudendo il coperchio, adesso, dati i valori originali
/// e quelli forzati.
pub fn effective(original: LidAction, force: Force, on_ac: bool) -> u32 {
    if on_ac {
        if force.ac {
            0
        } else {
            original.ac
        }
    } else if force.dc {
        0
    } else {
        original.dc
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LidAct {
    Sleep,
    Hibernate,
    Shutdown,
}

impl LidAct {
    pub fn from_value(v: u32) -> Option<LidAct> {
        match v {
            1 => Some(LidAct::Sleep),
            2 => Some(LidAct::Hibernate),
            3 => Some(LidAct::Shutdown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Il coperchio si è riaperto dopo che Moka l'aveva tenuto acceso; oppure
    /// "…e poi: blocca".
    Lock,
    /// Fare ciò che Windows avrebbe fatto (o "…e poi": sospendi, iberna, arresta).
    Perform(LidAct),
    /// "…e poi: spegni lo schermo".
    ScreenOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Why {
    /// Il motivo per restare acceso è finito a coperchio chiuso.
    Released,
    /// Protezione zaino.
    Backpack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pending {
    at_tick: u64,
    why: Why,
}

/// Il mondo visto dal coperchio in un certo istante.
#[derive(Debug, Clone, Copy)]
pub struct World {
    pub closed: bool,
    pub on_ac: bool,
    pub external_monitors: u32,
    /// L'impostazione di Windows com'era prima di Moka.
    pub original: LidAction,
    pub force: Force,
    pub backpack_minutes: u32,
    pub lock_on_open: bool,
}

#[derive(Debug, Default)]
pub struct Tracker {
    closed: bool,
    prev_effective: Option<u32>,
    /// Il coperchio è chiuso e il PC è acceso **grazie a Moka** (Windows
    /// l'avrebbe sospeso): alla riapertura va bloccato.
    kept_awake: bool,
    backpack_since: Option<u64>,
    pending: Option<Pending>,
}

impl Tracker {
    /// Da chiamare a ogni cambiamento e alla scadenza di [`Tracker::deadline`].
    pub fn update(&mut self, w: &World, tick: u64) -> Option<Effect> {
        let eff = effective(w.original, w.force, w.on_ac);
        let original_now = if w.on_ac {
            w.original.ac
        } else {
            w.original.dc
        };
        let mut out = None;

        if !w.closed {
            if self.closed && self.kept_awake && w.lock_on_open {
                out = Some(Effect::Lock);
            }
            self.kept_awake = false;
            self.backpack_since = None;
            self.pending = None;
        } else {
            if eff == 0 && original_now != 0 {
                self.kept_awake = true;
            }

            // Il motivo per restare acceso è finito mentre era già chiuso.
            let was_closed = self.closed;
            if was_closed && self.prev_effective == Some(0) && eff != 0 && self.pending.is_none() {
                self.pending = Some(Pending {
                    at_tick: tick + GRACE_MS,
                    why: Why::Released,
                });
            }
            if eff == 0 && matches!(self.pending, Some(p) if p.why == Why::Released) {
                self.pending = None;
            }

            // Protezione zaino: a batteria, chiuso, niente monitor, tenuto acceso.
            let backpack = !w.on_ac
                && w.external_monitors == 0
                && w.force.dc
                && w.original.dc != 0
                && w.backpack_minutes > 0;
            if backpack {
                let since = *self.backpack_since.get_or_insert(tick);
                if self.pending.is_none() {
                    self.pending = Some(Pending {
                        at_tick: since + u64::from(w.backpack_minutes) * 60_000,
                        why: Why::Backpack,
                    });
                }
            } else {
                self.backpack_since = None;
                if matches!(self.pending, Some(p) if p.why == Why::Backpack) {
                    self.pending = None;
                }
            }

            if let Some(p) = self.pending {
                if tick >= p.at_tick {
                    self.pending = None;
                    let value = match p.why {
                        Why::Released => eff,
                        Why::Backpack => w.original.dc,
                    };
                    if let Some(act) = LidAct::from_value(value) {
                        out = Some(Effect::Perform(act));
                    }
                    // Dopo la sospensione il conto dello zaino riparte da capo.
                    self.backpack_since = None;
                }
            }
        }

        self.closed = w.closed;
        self.prev_effective = Some(eff);
        out
    }

    /// Un "…e poi" vince su ciò che Windows avrebbe fatto: la sua azione
    /// arriva al posto di questa, non in aggiunta.
    pub fn cancel_released(&mut self) {
        if matches!(self.pending, Some(p) if p.why == Why::Released) {
            self.pending = None;
        }
    }

    /// Quando richiamare [`Tracker::update`] anche se non succede niente.
    pub fn deadline(&self) -> Option<u64> {
        self.pending.map(|p| p.at_tick)
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLEEP: LidAction = LidAction { ac: 1, dc: 1 };

    fn inputs(mode: LidMode, session_lid: bool, desk: bool, monitors: u32) -> Inputs {
        Inputs {
            mode,
            session_lid,
            desk_mode: desk,
            external_monitors: monitors,
        }
    }

    fn world(closed: bool, on_ac: bool, force: Force) -> World {
        World {
            closed,
            on_ac,
            external_monitors: 0,
            original: SLEEP,
            force,
            backpack_minutes: 30,
            lock_on_open: true,
        }
    }

    const AC: Force = Force {
        ac: true,
        dc: false,
    };
    const BOTH: Force = Force { ac: true, dc: true };

    #[test]
    fn desired_follows_the_three_choices() {
        assert_eq!(
            desired(&inputs(LidMode::Windows, true, false, 0)),
            Force::NONE
        );
        assert_eq!(desired(&inputs(LidMode::Ac, true, false, 0)), AC);
        assert_eq!(desired(&inputs(LidMode::Always, true, false, 0)), BOTH);
        // Nessuna sessione con il coperchio: niente da forzare.
        assert_eq!(
            desired(&inputs(LidMode::Always, false, false, 0)),
            Force::NONE
        );
    }

    #[test]
    fn desk_mode_needs_a_monitor_and_not_a_session() {
        assert_eq!(desired(&inputs(LidMode::Windows, false, true, 1)), BOTH);
        assert_eq!(
            desired(&inputs(LidMode::Windows, false, true, 0)),
            Force::NONE
        );
    }

    #[test]
    fn session_ends_with_lid_closed_then_moka_sleeps() {
        let mut t = Tracker::default();
        // Sessione in carica, coperchio aperto, poi chiuso: resta acceso.
        assert_eq!(t.update(&world(false, true, AC), 0), None);
        assert_eq!(t.update(&world(true, true, AC), 1_000), None);
        // La sessione finisce a coperchio chiuso: non subito, dopo 10 s.
        assert_eq!(t.update(&world(true, true, Force::NONE), 60_000), None);
        assert_eq!(t.deadline(), Some(60_000 + GRACE_MS));
        assert_eq!(
            t.update(&world(true, true, Force::NONE), 60_000 + GRACE_MS),
            Some(Effect::Perform(LidAct::Sleep))
        );
        assert_eq!(t.deadline(), None);
    }

    #[test]
    fn lid_reopened_during_grace_cancels_and_locks() {
        let mut t = Tracker::default();
        t.update(&world(true, true, AC), 0);
        t.update(&world(true, true, Force::NONE), 5_000);
        assert!(t.deadline().is_some());
        assert_eq!(
            t.update(&world(false, true, Force::NONE), 8_000),
            Some(Effect::Lock)
        );
        assert_eq!(t.deadline(), None);
    }

    #[test]
    fn unplugging_with_ac_only_behaves_like_windows() {
        let mut t = Tracker::default();
        t.update(&world(true, true, AC), 0);
        // A batteria il valore DC è di Windows (sospendi), ma Windows non
        // rivaluta il coperchio al cambio di alimentazione: lo fa Moka.
        t.update(&world(true, false, AC), 30_000);
        assert_eq!(
            t.update(&world(true, false, AC), 30_000 + GRACE_MS),
            Some(Effect::Perform(LidAct::Sleep))
        );
    }

    #[test]
    fn plugging_back_in_during_grace_cancels() {
        let mut t = Tracker::default();
        t.update(&world(true, true, AC), 0);
        t.update(&world(true, false, AC), 1_000);
        assert!(t.deadline().is_some());
        assert_eq!(t.update(&world(true, true, AC), 2_000), None);
        assert_eq!(t.deadline(), None);
    }

    #[test]
    fn closing_without_moka_leaves_it_to_windows() {
        let mut t = Tracker::default();
        // Niente forzato: Windows sospende da sé alla chiusura, Moka non fa niente.
        assert_eq!(t.update(&world(true, true, Force::NONE), 0), None);
        assert_eq!(t.deadline(), None);
        // E alla riapertura non blocca: non l'ha tenuto acceso lei.
        assert_eq!(t.update(&world(false, true, Force::NONE), 1_000), None);
    }

    #[test]
    fn windows_already_does_nothing() {
        let mut w = world(true, true, AC);
        w.original = LidAction { ac: 0, dc: 0 };
        let mut t = Tracker::default();
        t.update(&w, 0);
        w.force = Force::NONE;
        // Windows non avrebbe fatto niente: Moka nemmeno.
        assert_eq!(t.update(&w, 1_000), None);
        assert_eq!(t.update(&w, 1_000 + GRACE_MS), None);
        w.closed = false;
        assert_eq!(
            t.update(&w, 2_000 + GRACE_MS),
            None,
            "nessun blocco: non era merito di Moka"
        );
    }

    #[test]
    fn backpack_protection() {
        let mut t = Tracker::default();
        let mut w = world(true, false, BOTH);
        assert_eq!(t.update(&w, 0), None);
        assert_eq!(t.deadline(), Some(30 * 60_000));
        assert_eq!(t.update(&w, 29 * 60_000), None);
        assert_eq!(
            t.update(&w, 30 * 60_000),
            Some(Effect::Perform(LidAct::Sleep))
        );
        // Con un monitor esterno non è uno zaino.
        let mut t = Tracker::default();
        w.external_monitors = 1;
        t.update(&w, 0);
        assert_eq!(t.deadline(), None);
        // Tempo 0 = protezione spenta.
        let mut t = Tracker::default();
        w.external_monitors = 0;
        w.backpack_minutes = 0;
        t.update(&w, 0);
        assert_eq!(t.deadline(), None);
    }

    #[test]
    fn backpack_uses_windows_battery_action() {
        let mut t = Tracker::default();
        let mut w = world(true, false, BOTH);
        w.original = LidAction { ac: 1, dc: 2 };
        t.update(&w, 0);
        assert_eq!(
            t.update(&w, 30 * 60_000),
            Some(Effect::Perform(LidAct::Hibernate))
        );
    }

    #[test]
    fn backpack_restarts_when_plugged_in() {
        let mut t = Tracker::default();
        t.update(&world(true, false, BOTH), 0);
        // In carica per un po': il conto si azzera.
        t.update(&world(true, true, BOTH), 10 * 60_000);
        assert_eq!(t.deadline(), None);
        t.update(&world(true, false, BOTH), 20 * 60_000);
        assert_eq!(t.deadline(), Some(50 * 60_000));
    }

    #[test]
    fn desk_mode_monitor_unplugged_with_lid_closed() {
        let mut t = Tracker::default();
        let mut w = world(true, true, BOTH);
        w.external_monitors = 1;
        t.update(&w, 0);
        // Si scollega l'ultimo monitor: la modalità scrivania non vale più.
        w.external_monitors = 0;
        w.force = Force::NONE;
        t.update(&w, 1_000);
        assert_eq!(
            t.update(&w, 1_000 + GRACE_MS),
            Some(Effect::Perform(LidAct::Sleep))
        );
    }

    #[test]
    fn no_lock_when_disabled() {
        let mut t = Tracker::default();
        let mut w = world(true, true, AC);
        w.lock_on_open = false;
        t.update(&w, 0);
        w.closed = false;
        assert_eq!(t.update(&w, 1_000), None);
    }

    #[test]
    fn shutdown_and_unknown_values() {
        assert_eq!(LidAct::from_value(3), Some(LidAct::Shutdown));
        assert_eq!(LidAct::from_value(0), None);
        assert_eq!(LidAct::from_value(99), None);
    }
}
