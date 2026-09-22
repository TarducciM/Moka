//! Spike sullo standby moderno: a schermo spento (o a coperchio chiuso), quali
//! richieste tengono davvero sveglio il PC? Vedi `docs/ROADMAP.md`, sezione
//! "Standby moderno", e `docs/SPIKE.md` per la procedura.
//!
//! ```text
//! cargo run --release --example spike -- info
//! cargo run --release --example spike -- run --minutes 10 --screen-off
//! cargo run --release --example spike -- run --minutes 30 --lid --display
//! cargo run --release --example spike -- report spike-AAAAMMGG-HHMMSS.csv
//! ```
//!
//! `run` tiene le richieste scelte, scrive un battito ogni 10 s in un CSV e
//! alla fine cerca i **buchi**: un intervallo molto più lungo di 10 s vuol dire
//! che il processo non ha girato, cioè che il PC è andato in standby. Poi
//! incrocia il risultato con gli eventi Kernel-Power del registro di sistema.
//! Non si fida dell'impressione: si fida dei numeri.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, TimeZone, Utc};
use moka_lib::capabilities;
use moka_lib::lid::{self, LidAction};
use moka_lib::power::{Needs, PowerRequest};
use moka_lib::sys;
use windows::core::{BOOL, GUID};
use windows::Win32::System::Console::SetConsoleCtrlHandler;

const BEAT: Duration = Duration::from_secs(10);
/// Un intervallo oltre questa soglia è un buco (il processo non ha girato).
const GAP_MS: i64 = 25_000;

/// Il valore originale del coperchio, da rimettere comunque vada.
static LID_BACKUP: Mutex<Option<(GUID, LidAction)>> = Mutex::new(None);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("info") => info(),
        Some("lid-write-check") => lid_write_check(),
        Some("run") => run(&args[1..]),
        Some("report") => match args.get(1) {
            Some(path) => report(path),
            None => usage(),
        },
        _ => usage(),
    }
}

fn usage() {
    eprintln!(
        "uso:\n  spike info\n  spike run [--minutes N] [--display] [--execution] [--no-system]\n            [--screen-off] [--lid] [--lid-battery] [--log FILE]\n  spike report FILE"
    );
    std::process::exit(2);
}

fn info() {
    let host = std::env::var("COMPUTERNAME").unwrap_or_default();
    let caps = capabilities::read();
    let power = capabilities::power_status();
    println!("macchina          {host}");
    println!("portatile         {}", yes(caps.lid_present));
    println!("standby moderno   {}", yes(caps.modern_standby));
    println!("sospensione S3    {}", yes(caps.s3));
    println!("ibernazione       {}", yes(caps.hibernate));
    println!(
        "alimentazione     {}",
        match power.on_ac {
            Some(true) => "in carica",
            Some(false) => "a batteria",
            None => "sconosciuta",
        }
    );
    if let Some(p) = power.battery_percent {
        println!("batteria          {p}%");
    }

    match lid::active_scheme().and_then(|s| lid::read(&s).map(|a| (s, a))) {
        Some((scheme, action)) => {
            println!("schema attivo     {scheme:?}");
            println!(
                "coperchio         in carica: {} · a batteria: {}",
                lid::action_label(action.ac),
                lid::action_label(action.dc)
            );
        }
        None => println!("coperchio         non leggibile"),
    }
    let access = lid::access_check();
    println!(
        "criteri aziendali coperchio {}",
        if access.ac && access.dc {
            "nessuno (Moka può cambiarlo)"
        } else {
            "PRESENTI: il valore è imposto"
        }
    );

    // La richiesta si vede senza amministratore? (powercfg /requests lo richiede)
    let before = capabilities::execution_state();
    let mut req = PowerRequest::new();
    let held = req.apply(
        Needs {
            system: true,
            display: true,
            execution: false,
        },
        "Moka spike: prova",
    );
    std::thread::sleep(Duration::from_millis(200));
    let during = capabilities::execution_state();
    req.release();
    std::thread::sleep(Duration::from_millis(200));
    let after = capabilities::execution_state();
    println!(
        "stato di esecuzione prima/durante/dopo una richiesta: {} / {} / {}{}",
        fmt_state(before),
        fmt_state(during),
        fmt_state(after),
        if held.is_err() {
            "  (richiesta NON creata)"
        } else {
            ""
        }
    );
}

/// Riscrive l'azione del coperchio con il valore che ha già: non cambia niente,
/// ma dice se questo account può scriverla senza essere amministratore.
fn lid_write_check() {
    let Some(scheme) = lid::active_scheme() else {
        println!("Schema energetico non leggibile.");
        return;
    };
    let Some(current) = lid::read(&scheme) else {
        println!("Azione del coperchio non leggibile.");
        return;
    };
    match lid::write(&scheme, Some(current.ac), Some(current.dc)) {
        Ok(()) => println!(
            "Scrittura riuscita senza amministratore (valori invariati: in carica {}, a batteria {}).",
            lid::action_label(current.ac),
            lid::action_label(current.dc)
        ),
        Err(err) => println!("Scrittura NON riuscita: {err:?}"),
    }
    let after = lid::read(&scheme);
    println!("Rilettura: {after:?} (deve coincidere con {current:?})");
}

fn fmt_state(s: Option<u32>) -> String {
    match s {
        None => "?".into(),
        Some(v) => {
            let mut parts = Vec::new();
            if v & 1 != 0 {
                parts.push("SYSTEM");
            }
            if v & 2 != 0 {
                parts.push("DISPLAY");
            }
            if v & 0x40 != 0 {
                parts.push("AWAYMODE");
            }
            if parts.is_empty() {
                format!("0x{v:x}")
            } else {
                format!("0x{v:x} ({})", parts.join("+"))
            }
        }
    }
}

fn yes(b: bool) -> &'static str {
    if b {
        "sì"
    } else {
        "no"
    }
}

fn run(args: &[String]) {
    let mut minutes: u64 = 10;
    let mut needs = Needs {
        system: true,
        display: false,
        execution: false,
    };
    let mut screen_off = false;
    let mut lid_ac = false;
    let mut lid_dc = false;
    let mut log_path = format!("spike-{}.csv", Local::now().format("%Y%m%d-%H%M%S"));

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--minutes" => minutes = it.next().and_then(|v| v.parse().ok()).unwrap_or(minutes),
            "--display" => needs.display = true,
            "--execution" => needs.execution = true,
            "--no-system" => needs.system = false,
            "--screen-off" => screen_off = true,
            "--lid" => lid_ac = true,
            "--lid-battery" => {
                lid_ac = true;
                lid_dc = true;
            }
            "--log" => log_path = it.next().cloned().unwrap_or(log_path),
            _ => usage(),
        }
    }

    let caps = capabilities::read();
    println!(
        "Spike: {} min · richieste: {} · standby moderno: {} · log: {log_path}",
        minutes,
        describe(needs),
        yes(caps.modern_standby)
    );

    let mut req = PowerRequest::new();
    if needs.any() {
        if let Err(err) = req.apply(needs, "Moka spike: prova standby moderno") {
            eprintln!("Richiesta di alimentazione non creata: {err}");
            std::process::exit(1);
        }
    }

    if lid_ac {
        override_lid(lid_dc);
    }

    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("file di log");
    writeln!(
        log,
        "# needs={} screen_off={} lid_ac={} lid_dc={} modern_standby={}",
        describe(needs),
        screen_off,
        lid_ac,
        lid_dc,
        caps.modern_standby
    )
    .ok();
    writeln!(log, "local_time,tick_ms,wall_ms,on_ac,battery,exec_state").ok();

    if screen_off {
        println!("Lo schermo si spegne fra 5 secondi: non toccare mouse e tastiera.");
        std::thread::sleep(Duration::from_secs(5));
        sys::screen_off(None);
    } else if lid_ac {
        println!("Ora chiudi il coperchio e lascialo chiuso fino alla fine.");
    }

    let start = Instant::now();
    let total = Duration::from_secs(minutes * 60);
    let mut beats = 0u64;
    while start.elapsed() < total {
        beat(&mut log);
        beats += 1;
        if beats.is_multiple_of(6) {
            println!(
                "  {} min trascorsi (orologio del processo)",
                start.elapsed().as_secs() / 60
            );
        }
        std::thread::sleep(BEAT);
    }
    beat(&mut log);
    drop(log);

    restore_lid();
    req.release();
    println!("\nFine. Risultato:\n");
    report(&log_path);
}

fn describe(n: Needs) -> String {
    let mut parts = Vec::new();
    if n.system {
        parts.push("SystemRequired");
    }
    if n.display {
        parts.push("DisplayRequired");
    }
    if n.execution {
        parts.push("ExecutionRequired");
    }
    if parts.is_empty() {
        "nessuna".into()
    } else {
        parts.join(" + ")
    }
}

fn beat(log: &mut File) {
    let now = sys::now();
    let power = capabilities::power_status();
    let line = format!(
        "{},{},{},{},{},{}",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        now.tick_ms,
        now.wall_ms,
        power
            .on_ac
            .map(|b| if b { "1" } else { "0" })
            .unwrap_or("?"),
        power
            .battery_percent
            .map(|p| p.to_string())
            .unwrap_or_default(),
        capabilities::execution_state()
            .map(|s| format!("0x{s:x}"))
            .unwrap_or_default(),
    );
    writeln!(log, "{line}").ok();
    log.flush().ok();
}

fn override_lid(battery_too: bool) {
    let Some(scheme) = lid::active_scheme() else {
        eprintln!("Schema energetico non leggibile: niente modifica al coperchio.");
        return;
    };
    let Some(original) = lid::read(&scheme) else {
        eprintln!("Azione del coperchio non leggibile: niente modifica.");
        return;
    };
    *LID_BACKUP.lock().unwrap() = Some((scheme, original));
    unsafe {
        let _ = SetConsoleCtrlHandler(Some(on_console_close), true);
    }
    let dc = battery_too.then_some(lid::DO_NOTHING);
    match lid::write(&scheme, Some(lid::DO_NOTHING), dc) {
        Ok(()) => println!(
            "Coperchio temporaneamente su \"non fare nulla\" (era: in carica {}, a batteria {}).\n\
             Se lo spike si interrompe male, per rimetterlo:\n  \
             powercfg /setacvalueindex SCHEME_CURRENT SUB_BUTTONS LIDACTION {}\n  \
             powercfg /setdcvalueindex SCHEME_CURRENT SUB_BUTTONS LIDACTION {}\n  \
             powercfg /setactive SCHEME_CURRENT",
            lid::action_label(original.ac),
            lid::action_label(original.dc),
            original.ac,
            original.dc
        ),
        Err(err) => {
            eprintln!("Modifica del coperchio non riuscita: {err:?}");
            *LID_BACKUP.lock().unwrap() = None;
        }
    }
}

fn restore_lid() {
    if let Some((scheme, original)) = LID_BACKUP.lock().unwrap().take() {
        match lid::write(&scheme, Some(original.ac), Some(original.dc)) {
            Ok(()) => println!(
                "Coperchio rimesso com'era (in carica {}, a batteria {}).",
                lid::action_label(original.ac),
                lid::action_label(original.dc)
            ),
            Err(err) => {
                eprintln!("ATTENZIONE: coperchio NON rimesso ({err:?}). Usa i comandi sopra.")
            }
        }
    }
}

/// Ctrl+C o chiusura della console: si rimette il coperchio, poi si lascia
/// terminare il processo come al solito.
unsafe extern "system" fn on_console_close(_ctrl: u32) -> BOOL {
    restore_lid();
    BOOL(0)
}

fn report(path: &str) {
    let Ok(file) = File::open(path) else {
        eprintln!("Log non trovato: {path}");
        return;
    };
    let mut rows: Vec<(String, i64)> = Vec::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line.starts_with('#') || line.starts_with("local_time") {
            continue;
        }
        let cols: Vec<&str> = line.split(',').collect();
        if let (Some(time), Some(wall)) = (cols.first(), cols.get(2).and_then(|w| w.parse().ok())) {
            rows.push(((*time).to_owned(), wall));
        }
    }
    if rows.len() < 2 {
        println!("Troppo pochi battiti per dire qualcosa ({}).", rows.len());
        return;
    }

    let first = rows.first().unwrap().1;
    let last = rows.last().unwrap().1;
    let mut gaps = Vec::new();
    for pair in rows.windows(2) {
        let dt = pair[1].1 - pair[0].1;
        if dt > GAP_MS {
            gaps.push((pair[0].0.clone(), pair[1].0.clone(), dt));
        }
    }
    println!(
        "Battiti: {} in {} min.",
        rows.len(),
        (last - first) / 60_000
    );
    if gaps.is_empty() {
        println!("Nessun buco: il PC è rimasto sveglio per tutto il tempo.");
    } else {
        let total: i64 = gaps.iter().map(|g| g.2).sum();
        println!(
            "{} buchi, {} s in tutto: il PC è andato in standby.",
            gaps.len(),
            total / 1000
        );
        for (a, b, dt) in &gaps {
            println!("  da {a} a {b}  ({} s)", dt / 1000);
        }
    }
    kernel_power_events(first, last);
}

/// Gli eventi di sospensione e standby nell'intervallo del log: 42 = sospensione,
/// 107 = ripresa, 506/507 = ingresso/uscita dallo standby moderno.
fn kernel_power_events(from_ms: i64, to_ms: i64) {
    let iso = |ms: i64| -> String {
        let t: DateTime<Utc> = Utc.timestamp_millis_opt(ms).single().unwrap_or_default();
        t.format("%Y-%m-%dT%H:%M:%S.000Z").to_string()
    };
    let query = format!(
        "*[System[Provider[@Name='Microsoft-Windows-Kernel-Power'] and \
         (EventID=42 or EventID=107 or EventID=506 or EventID=507) and \
         TimeCreated[@SystemTime>='{}' and @SystemTime<='{}']]]",
        iso(from_ms - 60_000),
        iso(to_ms + 60_000)
    );
    let out = Command::new("wevtutil")
        .args(["qe", "System", &format!("/q:{query}"), "/f:text", "/c:50"])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            let events: Vec<&str> = text
                .lines()
                .filter(|l| {
                    let l = l.trim_start();
                    l.starts_with("Date:")
                        || l.starts_with("Data:")
                        || l.starts_with("Event ID:")
                        || l.starts_with("ID evento:")
                })
                .collect();
            if events.is_empty() {
                println!(
                    "Registro di sistema: nessun evento di sospensione o standby nell'intervallo."
                );
            } else {
                println!("Registro di sistema (Kernel-Power):");
                for pair in events.chunks(2) {
                    println!(
                        "  {}",
                        pair.join("  ")
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                }
            }
        }
        Ok(o) => println!(
            "Registro di sistema non leggibile: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ),
        Err(err) => println!("wevtutil non eseguito: {err}"),
    }
}
