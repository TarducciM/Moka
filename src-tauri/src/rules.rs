//! Le regole automatiche ("tieni sveglio mentre…"): il modello e la loro
//! valutazione, logica pura. Le osservazioni sul sistema (processi aperti,
//! schermo intero, microfono, rete, CPU) le raccoglie `probes` e arrivano qui
//! come dati: così ogni caso si prova senza Windows.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::session::{Mode, ThenAct};

/// Quante regole al massimo: oltre, un elenco diventa illeggibile.
pub const MAX_RULES: usize = 20;
/// Soglie offerte per il download, in KB/s.
pub const DOWNLOAD_CHOICES: [u32; 4] = [100, 500, 1000, 5000];
/// Soglie offerte per la CPU, in percentuale.
pub const CPU_CHOICES: [u32; 3] = [25, 50, 75];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuleKind {
    /// Un programma è aperto (nome dell'eseguibile, minuscolo, con `.exe`).
    Process { exe: String },
    /// Un'app è a schermo intero: video, giochi, presentazioni.
    Fullscreen,
    /// Microfono o webcam in uso.
    Call,
    /// Il PC è in carica.
    Plugged,
    /// C'è un monitor esterno collegato.
    Monitor,
    /// Si scarica più di `kbps` KB/s.
    Download { kbps: u32 },
    /// La CPU lavora sopra `percent`.
    Cpu { percent: u32 },
    /// Nei giorni di `days` (bit 0 = lunedì … bit 6 = domenica), fra `from` e
    /// `to` (minuti dalla mezzanotte). Se `from` > `to` la fascia scavalca la
    /// mezzanotte e appartiene al giorno in cui comincia.
    Schedule { days: u8, from: u16, to: u16 },
    /// Solo da riga di comando (`--while-pid`): un processo preciso.
    Pid { pid: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: u32,
    pub enabled: bool,
    pub mode: Mode,
    /// "…e poi" quando la regola smette di valere (se nient'altro tiene
    /// sveglio il PC in quel momento).
    #[serde(default)]
    pub then: ThenAct,
    #[serde(flatten)]
    pub kind: RuleKind,
}

/// "OBS64" → "obs64.exe": minuscolo, sempre con l'estensione.
pub fn normalize_exe(name: &str) -> Option<String> {
    let name = name.trim().to_lowercase();
    let name = name.rsplit(['\\', '/']).next().unwrap_or(&name).to_owned();
    if name.is_empty() || name.len() > 64 || name.chars().any(|c| c.is_control()) {
        return None;
    }
    Some(if name.ends_with(".exe") {
        name
    } else {
        format!("{name}.exe")
    })
}

/// Una regola arrivata da fuori (file, pagina) è valida solo se il codice
/// avrebbe potuto scriverla.
pub fn validate(kind: RuleKind) -> Option<RuleKind> {
    match kind {
        RuleKind::Process { exe } => normalize_exe(&exe).map(|exe| RuleKind::Process { exe }),
        RuleKind::Download { kbps } => DOWNLOAD_CHOICES
            .contains(&kbps)
            .then_some(RuleKind::Download { kbps }),
        RuleKind::Cpu { percent } => CPU_CHOICES
            .contains(&percent)
            .then_some(RuleKind::Cpu { percent }),
        RuleKind::Schedule { days, from, to } => {
            (days & 0x7f != 0 && from < 1440 && to < 1440 && from != to).then_some(
                RuleKind::Schedule {
                    days: days & 0x7f,
                    from,
                    to,
                },
            )
        }
        // Un pid non ha senso salvato: vive solo finché vive il processo.
        RuleKind::Pid { .. } => None,
        other => Some(other),
    }
}

/// Legge le regole dalle impostazioni, come se fossero ostili: le voci non
/// valide si scartano, gli id doppi si rinumerano, al massimo 20.
pub fn from_value(v: Option<&Value>) -> Vec<Rule> {
    let mut out: Vec<Rule> = Vec::new();
    let mut seen = HashSet::new();
    for item in v.and_then(Value::as_array).into_iter().flatten() {
        let Ok(mut rule) = serde_json::from_value::<Rule>(item.clone()) else {
            continue;
        };
        let Some(kind) = validate(rule.kind.clone()) else {
            continue;
        };
        rule.kind = kind;
        if rule.id == 0 || !seen.insert(rule.id) {
            rule.id = next_id(&out);
            seen.insert(rule.id);
        }
        out.push(rule);
        if out.len() == MAX_RULES {
            break;
        }
    }
    out
}

pub fn next_id(rules: &[Rule]) -> u32 {
    rules.iter().map(|r| r.id).max().unwrap_or(0) + 1
}

/// Quello che le sonde hanno visto in un certo istante. `None` = non
/// osservato (nessuna regola ne aveva bisogno).
#[derive(Debug, Clone, Default)]
pub struct Observation {
    pub processes: Option<HashSet<String>>,
    pub pids: Option<HashSet<u32>>,
    pub fullscreen: bool,
    pub call: bool,
    pub on_ac: bool,
    pub external_monitors: u32,
    pub net_kbps: Option<u32>,
    pub cpu_percent: Option<u32>,
    /// 0 = lunedì … 6 = domenica.
    pub weekday: u8,
    /// Minuti dalla mezzanotte, ora locale.
    pub minute: u16,
}

/// Quali sonde servono per queste regole.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Needs {
    pub processes: bool,
    pub fullscreen: bool,
    pub call: bool,
    pub net: bool,
    pub cpu: bool,
}

pub fn needs<'a>(rules: impl IntoIterator<Item = &'a Rule>) -> Needs {
    let mut n = Needs::default();
    for r in rules.into_iter().filter(|r| r.enabled) {
        match r.kind {
            RuleKind::Process { .. } | RuleKind::Pid { .. } => n.processes = true,
            RuleKind::Fullscreen => n.fullscreen = true,
            RuleKind::Call => n.call = true,
            RuleKind::Download { .. } => n.net = true,
            RuleKind::Cpu { .. } => n.cpu = true,
            RuleKind::Plugged | RuleKind::Monitor | RuleKind::Schedule { .. } => {}
        }
    }
    n
}

/// Vale in questo istante, senza isteresi?
pub fn holds(kind: &RuleKind, o: &Observation) -> bool {
    match kind {
        RuleKind::Process { exe } => o.processes.as_ref().is_some_and(|p| p.contains(exe)),
        RuleKind::Pid { pid } => o.pids.as_ref().is_some_and(|p| p.contains(pid)),
        RuleKind::Fullscreen => o.fullscreen,
        RuleKind::Call => o.call,
        RuleKind::Plugged => o.on_ac,
        RuleKind::Monitor => o.external_monitors > 0,
        RuleKind::Download { kbps } => o.net_kbps.is_some_and(|k| k >= *kbps),
        RuleKind::Cpu { percent } => o.cpu_percent.is_some_and(|c| c >= *percent),
        RuleKind::Schedule { days, from, to } => {
            in_schedule(*days, *from, *to, o.weekday, o.minute)
        }
    }
}

fn in_schedule(days: u8, from: u16, to: u16, weekday: u8, minute: u16) -> bool {
    let day_on = |d: u8| days & (1 << (d % 7)) != 0;
    if from < to {
        day_on(weekday) && (from..to).contains(&minute)
    } else if minute >= from {
        // Sera della fascia che scavalca la mezzanotte: vale il giorno di oggi.
        day_on(weekday)
    } else {
        // Dopo mezzanotte: la fascia è cominciata ieri.
        minute < to && day_on((weekday + 6) % 7)
    }
}

/// Dopo quanto un "non vale più" diventa vero davvero. Download e CPU vanno
/// a ondate (una pausa fra due file, un attimo di calma in una
/// compilazione): senza un po' di pazienza la regola si accenderebbe e
/// spegnerebbe di continuo. Il microfono si spegne anche solo per un attimo
/// quando si cambia dispositivo in una chiamata.
pub fn linger_ms(kind: &RuleKind) -> u64 {
    match kind {
        RuleKind::Download { .. } | RuleKind::Cpu { .. } => 120_000,
        RuleKind::Call => 30_000,
        _ => 0,
    }
}

/// Ricorda da quando ogni regola vale, per l'isteresi.
#[derive(Debug, Default)]
pub struct Engine {
    last_true: HashMap<u32, u64>,
}

impl Engine {
    /// Gli id delle regole che valgono adesso.
    pub fn evaluate<'a>(
        &mut self,
        rules: impl IntoIterator<Item = &'a Rule>,
        o: &Observation,
        tick_ms: u64,
    ) -> Vec<u32> {
        let mut active = Vec::new();
        let mut alive = HashSet::new();
        for r in rules.into_iter().filter(|r| r.enabled) {
            alive.insert(r.id);
            if holds(&r.kind, o) {
                self.last_true.insert(r.id, tick_ms);
                active.push(r.id);
            } else if let Some(&t) = self.last_true.get(&r.id) {
                if tick_ms.saturating_sub(t) < linger_ms(&r.kind) {
                    active.push(r.id);
                } else {
                    self.last_true.remove(&r.id);
                }
            }
        }
        self.last_true.retain(|id, _| alive.contains(id));
        active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rule(id: u32, kind: RuleKind) -> Rule {
        Rule {
            id,
            enabled: true,
            mode: Mode::System,
            then: ThenAct::None,
            kind,
        }
    }

    fn obs() -> Observation {
        Observation {
            weekday: 0,
            minute: 600,
            ..Observation::default()
        }
    }

    #[test]
    fn exe_names() {
        assert_eq!(normalize_exe("OBS64").as_deref(), Some("obs64.exe"));
        assert_eq!(normalize_exe(" ffmpeg.EXE ").as_deref(), Some("ffmpeg.exe"));
        assert_eq!(
            normalize_exe(r"C:\Tools\HandBrake.exe").as_deref(),
            Some("handbrake.exe")
        );
        assert_eq!(normalize_exe(""), None);
        assert_eq!(normalize_exe(&"a".repeat(70)), None);
    }

    #[test]
    fn process_rule() {
        let r = rule(
            1,
            RuleKind::Process {
                exe: "obs64.exe".into(),
            },
        );
        let mut o = obs();
        assert!(!holds(&r.kind, &o), "processi non osservati: non vale");
        o.processes = Some(["explorer.exe".to_owned()].into());
        assert!(!holds(&r.kind, &o));
        o.processes.as_mut().unwrap().insert("obs64.exe".into());
        assert!(holds(&r.kind, &o));
    }

    #[test]
    fn schedule_same_day() {
        // lun-ven 9-18
        let k = RuleKind::Schedule {
            days: 0b0011111,
            from: 540,
            to: 1080,
        };
        let at = |weekday, minute| Observation {
            weekday,
            minute,
            ..Observation::default()
        };
        assert!(holds(&k, &at(0, 540)), "lunedì 9:00, compreso");
        assert!(holds(&k, &at(4, 1079)));
        assert!(!holds(&k, &at(4, 1080)), "le 18:00 sono escluse");
        assert!(!holds(&k, &at(5, 600)), "sabato no");
    }

    #[test]
    fn schedule_across_midnight() {
        // venerdì 22:00 - 06:00
        let k = RuleKind::Schedule {
            days: 1 << 4,
            from: 1320,
            to: 360,
        };
        let at = |weekday, minute| Observation {
            weekday,
            minute,
            ..Observation::default()
        };
        assert!(holds(&k, &at(4, 1330)), "venerdì 22:10");
        assert!(
            holds(&k, &at(5, 60)),
            "sabato 01:00 appartiene alla notte di venerdì"
        );
        assert!(!holds(&k, &at(5, 1330)), "sabato 22:10 no");
        assert!(
            !holds(&k, &at(4, 60)),
            "venerdì 01:00 è la notte di giovedì"
        );
        assert!(!holds(&k, &at(5, 360)), "06:00 esclusa");
    }

    #[test]
    fn download_lingers_then_stops() {
        let r = rule(1, RuleKind::Download { kbps: 500 });
        let mut e = Engine::default();
        let mut o = obs();
        o.net_kbps = Some(800);
        assert_eq!(e.evaluate([&r], &o, 0), vec![1]);
        o.net_kbps = Some(10);
        // Una pausa fra due file: resta attiva.
        assert_eq!(e.evaluate([&r], &o, 60_000), vec![1]);
        assert_eq!(e.evaluate([&r], &o, 119_999), vec![1]);
        // Due minuti sotto soglia: finita.
        assert!(e.evaluate([&r], &o, 120_000).is_empty());
    }

    #[test]
    fn disabled_rules_do_nothing() {
        let mut r = rule(1, RuleKind::Plugged);
        r.enabled = false;
        let mut o = obs();
        o.on_ac = true;
        assert!(Engine::default().evaluate([&r], &o, 0).is_empty());
        assert_eq!(needs([&r]), Needs::default());
    }

    #[test]
    fn needs_only_what_is_used() {
        let rules = [
            rule(
                1,
                RuleKind::Process {
                    exe: "a.exe".into(),
                },
            ),
            rule(2, RuleKind::Cpu { percent: 50 }),
        ];
        let n = needs(&rules);
        assert!(n.processes && n.cpu && !n.net && !n.call && !n.fullscreen);
    }

    #[test]
    fn hostile_rules_are_filtered() {
        let v = json!([
            { "id": 1, "enabled": true, "mode": "system", "kind": "process", "exe": "OBS64" },
            { "id": 1, "enabled": true, "mode": "display", "kind": "fullscreen" },
            { "id": 3, "enabled": true, "mode": "system", "kind": "download", "kbps": 7 },
            { "id": 4, "enabled": true, "mode": "system", "kind": "schedule", "days": 0, "from": 1, "to": 2 },
            { "id": 5, "enabled": true, "mode": "system", "kind": "pid", "pid": 1234 },
            { "id": 6, "enabled": "yes", "mode": "system", "kind": "call" },
            { "id": 7, "enabled": true, "mode": "system", "kind": "teleport" },
            42
        ]);
        let rules = from_value(Some(&v));
        assert_eq!(rules.len(), 2);
        assert_eq!(
            rules[0].kind,
            RuleKind::Process {
                exe: "obs64.exe".into()
            }
        );
        assert_eq!(rules[1].id, 2, "id doppio rinumerato");
        assert_eq!(rules[1].mode, Mode::Display);
        assert!(from_value(None).is_empty());
    }

    #[test]
    fn roundtrip_json_shape() {
        let r = Rule {
            id: 3,
            enabled: true,
            mode: Mode::Display,
            then: ThenAct::Sleep,
            kind: RuleKind::Process {
                exe: "ffmpeg.exe".into(),
            },
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["kind"], "process");
        assert_eq!(v["exe"], "ffmpeg.exe");
        assert_eq!(v["then"], "sleep");
        assert_eq!(from_value(Some(&json!([v]))), vec![r]);
    }
}
