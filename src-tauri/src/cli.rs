//! Riga di comando. Gli argomenti arrivano o all'avvio, o all'istanza già
//! aperta tramite il plugin single-instance: in entrambi i casi passano da qui.
//!
//! ```text
//! moka                      apre il pannello
//! moka --for 2h             sveglio per 2 ore (anche 90m, 1h30m)
//! moka --until 18:30        fino alle 18:30
//! moka --forever            finché non lo spengo
//! moka --screen             anche lo schermo (si combina con le altre;
//!                           senza, una durata vuol dire "solo il PC")
//! moka --on                 accende con l'ultima scelta fatta nel pannello
//! moka --toggle             accende o spegne, come il clic sull'icona
//! moka --off                spegne
//! moka --screen-off         spegne subito lo schermo, il PC resta sveglio
//! moka --quit               chiude Moka (la sessione finisce)
//! moka --then sleep         a fine sessione: display-off | lock | sleep | hibernate |
//!                           shutdown | none (da solo: vale per la sessione in corso)
//! moka --lid / --no-lid     questa sessione resta accesa (o no) a coperchio chiuso
//! moka --restore-lid        rimette l'impostazione del coperchio com'era, poi esce
//! moka --while ffmpeg.exe   sveglio finché gira ffmpeg (si combina con --screen
//!                           e --then; non si salva fra le regole)
//! moka --while-pid 1234     sveglio finché vive il processo 1234
//! moka --pause-rules        sospende le regole automatiche per un'ora
//!                           (--pause-rules=2h per un tempo diverso)
//! moka --resume-rules       le riattiva subito
//! ```
//!
//! L'eseguibile è un'app a finestre: niente output sul terminale (trappola 15).

use crate::rules::{normalize_exe, RuleKind};
use crate::session::{parse_clock, parse_duration, Mode, Spec, ThenAct};

/// `--pause-rules` senza durata: un'ora, come nel pannello.
pub const PAUSE_DEFAULT_MINUTES: u32 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Nessun comando: all'avvio non fa niente, da una seconda istanza apre il pannello.
    None,
    /// `mode: None` = l'ultima modalità usata; `spec: None` = l'ultima durata usata.
    Start {
        mode: Option<Mode>,
        spec: Option<Spec>,
        /// `None` = l'ultima scelta fatta nel pannello.
        lid: Option<bool>,
        /// "…e poi"; `None` = quello scelto nel pannello per la prossima sessione.
        then: Option<ThenAct>,
    },
    /// `--then` da solo: cambia il "…e poi" della sessione in corso (o della prossima).
    SetThen(ThenAct),
    Off,
    Toggle,
    ScreenOff,
    /// Chiude Moka: la sessione finisce e non viene ripresa.
    Quit,
    /// Rimette l'impostazione del coperchio da un registro lasciato lì ed
    /// esce, senza avviare l'app (lo usano `RunOnce` e il disinstallatore).
    RestoreLid,
    /// Usato dall'installer (0.3): applica la scelta sull'avvio automatico ed esce.
    Autostart(bool),
    /// `--while` / `--while-pid`: una regola che vive finché vive il processo.
    While {
        kind: RuleKind,
        mode: Mode,
        then: ThenAct,
    },
    /// Sospende le regole salvate per tanti minuti.
    PauseRules(u32),
    ResumeRules,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub action: Action,
    /// Avviato da Windows all'accesso (`--autostart`): nessun benvenuto.
    pub from_autostart: bool,
    /// Argomenti non capiti: si ignorano, ma i test li vedono.
    pub unknown: Vec<String>,
}

/// `args` senza il nome del programma.
pub fn parse<I, S>(args: I) -> Parsed
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Vec<String> = args.into_iter().map(|a| a.as_ref().to_owned()).collect();
    let mut spec: Option<Spec> = None;
    let mut screen = false;
    let mut on = false;
    let mut lid: Option<bool> = None;
    let mut then: Option<ThenAct> = None;
    let mut explicit: Option<Action> = None;
    let mut watch: Option<RuleKind> = None;
    let mut from_autostart = false;
    let mut unknown = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        // Accetta sia "--for 2h" sia "--for=2h".
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n, Some(v.to_owned())),
            _ => (arg, None),
        };
        let mut value = || {
            inline.clone().or_else(|| {
                i += 1;
                args.get(i).cloned()
            })
        };
        match name {
            "--for" => match value().as_deref().and_then(parse_duration) {
                Some(minutes) => spec = Some(Spec::Minutes { minutes }),
                None => unknown.push(arg.to_owned()),
            },
            "--until" => match value().as_deref().and_then(parse_clock) {
                Some((hour, minute)) => spec = Some(Spec::Until { hour, minute }),
                None => unknown.push(arg.to_owned()),
            },
            "--then" => match value().as_deref().and_then(ThenAct::parse) {
                Some(act) => then = Some(act),
                None => unknown.push(arg.to_owned()),
            },
            "--while" => match value().as_deref().and_then(normalize_exe) {
                Some(exe) => watch = Some(RuleKind::Process { exe }),
                None => unknown.push(arg.to_owned()),
            },
            "--while-pid" => match value().and_then(|v| v.trim().parse::<u32>().ok()) {
                Some(pid) if pid > 0 => watch = Some(RuleKind::Pid { pid }),
                _ => unknown.push(arg.to_owned()),
            },
            // La durata solo attaccata (`--pause-rules=2h`): staccata, il
            // valore si confonderebbe con l'argomento successivo.
            "--pause-rules" => match inline.as_deref().map(parse_duration) {
                None => explicit = Some(Action::PauseRules(PAUSE_DEFAULT_MINUTES)),
                Some(Some(minutes)) => explicit = Some(Action::PauseRules(minutes)),
                Some(None) => unknown.push(arg.to_owned()),
            },
            "--resume-rules" => explicit = Some(Action::ResumeRules),
            "--forever" => spec = Some(Spec::Never),
            "--screen" => screen = true,
            "--on" => on = true,
            "--off" => explicit = Some(Action::Off),
            "--toggle" => explicit = Some(Action::Toggle),
            "--screen-off" => explicit = Some(Action::ScreenOff),
            "--quit" => explicit = Some(Action::Quit),
            "--restore-lid" => explicit = Some(Action::RestoreLid),
            "--lid" => lid = Some(true),
            "--no-lid" => lid = Some(false),
            "--enable-autostart" => explicit = Some(Action::Autostart(true)),
            "--disable-autostart" => explicit = Some(Action::Autostart(false)),
            "--autostart" => from_autostart = true,
            _ => unknown.push(arg.to_owned()),
        }
        i += 1;
    }

    // Negli script il risultato non deve dipendere da cosa si è cliccato
    // l'ultima volta nel pannello: con una durata, senza `--screen` vuol dire
    // "solo il PC". Solo `--on` da solo riprende davvero l'ultima scelta.
    let mode = if screen {
        Some(Mode::Display)
    } else if spec.is_some() {
        Some(Mode::System)
    } else {
        None
    };
    let starts = spec.is_some() || screen || on || lid.is_some();
    let action = explicit.unwrap_or(if let Some(kind) = watch {
        // Come per una durata: senza `--screen`, solo il PC.
        Action::While {
            kind,
            mode: mode.unwrap_or(Mode::System),
            then: then.unwrap_or(ThenAct::None),
        }
    } else if starts {
        Action::Start {
            mode,
            spec,
            lid,
            then,
        }
    } else if let Some(act) = then {
        Action::SetThen(act)
    } else {
        Action::None
    });

    Parsed {
        action,
        from_autostart,
        unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(args: &[&str]) -> Action {
        parse(args).action
    }

    #[test]
    fn no_args() {
        assert_eq!(action(&[]), Action::None);
        let p = parse(["--autostart"]);
        assert_eq!(p.action, Action::None);
        assert!(p.from_autostart);
    }

    #[test]
    fn durations_and_times() {
        assert_eq!(
            action(&["--for", "2h"]),
            Action::Start {
                mode: Some(Mode::System),
                spec: Some(Spec::Minutes { minutes: 120 }),
                lid: None,
                then: None,
            }
        );
        assert_eq!(
            action(&["--for=1h30m", "--screen"]),
            Action::Start {
                mode: Some(Mode::Display),
                spec: Some(Spec::Minutes { minutes: 90 }),
                lid: None,
                then: None,
            }
        );
        assert_eq!(
            action(&["--until", "18:30"]),
            Action::Start {
                mode: Some(Mode::System),
                spec: Some(Spec::Until {
                    hour: 18,
                    minute: 30
                }),
                lid: None,
                then: None,
            }
        );
        assert_eq!(
            action(&["--screen"]),
            Action::Start {
                mode: Some(Mode::Display),
                spec: None,
                lid: None,
                then: None,
            }
        );
        assert_eq!(
            action(&["--on"]),
            Action::Start {
                mode: None,
                spec: None,
                lid: None,
                then: None,
            }
        );
        assert_eq!(
            action(&["--forever"]),
            Action::Start {
                mode: Some(Mode::System),
                spec: Some(Spec::Never),
                lid: None,
                then: None,
            }
        );
    }

    #[test]
    fn explicit_commands_win() {
        assert_eq!(action(&["--off"]), Action::Off);
        assert_eq!(action(&["--for", "2h", "--off"]), Action::Off);
        assert_eq!(action(&["--toggle"]), Action::Toggle);
        assert_eq!(action(&["--screen-off"]), Action::ScreenOff);
        assert_eq!(action(&["--quit"]), Action::Quit);
        assert_eq!(action(&["--restore-lid"]), Action::RestoreLid);
        assert_eq!(action(&["--enable-autostart"]), Action::Autostart(true));
        assert_eq!(action(&["--disable-autostart"]), Action::Autostart(false));
    }

    #[test]
    fn lid_flags() {
        assert_eq!(
            action(&["--for", "2h", "--lid"]),
            Action::Start {
                mode: Some(Mode::System),
                spec: Some(Spec::Minutes { minutes: 120 }),
                lid: Some(true),
                then: None,
            }
        );
        assert_eq!(
            action(&["--no-lid"]),
            Action::Start {
                mode: None,
                spec: None,
                lid: Some(false),
                then: None,
            }
        );
    }

    #[test]
    fn then_flag() {
        assert_eq!(
            action(&["--for", "2h", "--then", "sleep"]),
            Action::Start {
                mode: Some(Mode::System),
                spec: Some(Spec::Minutes { minutes: 120 }),
                lid: None,
                then: Some(ThenAct::Sleep),
            }
        );
        assert_eq!(
            action(&["--then=display-off"]),
            Action::SetThen(ThenAct::ScreenOff)
        );
        assert_eq!(parse(["--then", "boom"]).unknown, vec!["--then"]);
    }

    #[test]
    fn while_flags() {
        assert_eq!(
            action(&["--while", "C:\\Tools\\FFmpeg.exe", "--then", "sleep"]),
            Action::While {
                kind: RuleKind::Process {
                    exe: "ffmpeg.exe".into()
                },
                mode: Mode::System,
                then: ThenAct::Sleep,
            }
        );
        assert_eq!(
            action(&["--while-pid=1234", "--screen"]),
            Action::While {
                kind: RuleKind::Pid { pid: 1234 },
                mode: Mode::Display,
                then: ThenAct::None,
            }
        );
        assert_eq!(parse(["--while-pid", "0"]).unknown, vec!["--while-pid"]);
        assert_eq!(parse(["--while"]).unknown, vec!["--while"]);
    }

    #[test]
    fn pause_flags() {
        assert_eq!(action(&["--pause-rules"]), Action::PauseRules(60));
        assert_eq!(action(&["--pause-rules=2h"]), Action::PauseRules(120));
        // Staccato, "2h" non è la durata della pausa.
        assert_eq!(
            parse(["--pause-rules", "2h"]).unknown,
            vec!["2h".to_owned()]
        );
        assert_eq!(
            parse(["--pause-rules=boh"]).unknown,
            vec!["--pause-rules=boh"]
        );
        assert_eq!(action(&["--resume-rules"]), Action::ResumeRules);
    }

    #[test]
    fn bad_values_are_reported_not_guessed() {
        let p = parse(["--for", "boh", "--until", "25:00", "--nope"]);
        assert_eq!(p.action, Action::None);
        assert_eq!(p.unknown, vec!["--for", "--until", "--nope"]);
        // Valore mancante in fondo.
        assert_eq!(parse(["--for"]).unknown, vec!["--for"]);
    }
}
