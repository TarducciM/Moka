//! Moka: tieni sveglio il PC dalla tray.

pub mod actions;
pub mod capabilities;
pub mod cli;
pub mod diagnose;
pub mod i18n;
pub mod lid;
pub mod lidoverride;
pub mod lidplan;
pub mod power;
pub mod probes;
pub mod rules;
pub mod session;
pub mod settings;
pub mod sys;
pub mod sysevents;

mod commands;
mod control;
mod popover;
mod presence;
mod shortcuts;
mod state;
mod tray;
mod updates;

use std::sync::{Condvar, Mutex};
use std::time::Duration;

use tauri::menu::MenuEvent;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};

use crate::control::TRAY_ID;
use crate::session::{Mode, Spec};
use crate::settings::LeftClick;
use crate::state::{AppState, Core, Paths, UiCache};

/// Da quanto l'utente non tocca tastiera e mouse (per lo spike delle sonde).
pub fn presence_idle_ms() -> u64 {
    presence::idle_ms()
}

/// L'identifier dell'app: la cartella dei dati è `%APPDATA%\<identifier>`.
const IDENTIFIER: &str = "com.moka.app";

/// `moka --restore-lid`: rimette l'impostazione del coperchio da un registro
/// lasciato lì ed esce. Senza Tauri: lo lanciano `RunOnce` al prossimo
/// accesso e il disinstallatore, quando Moka non è in esecuzione.
pub fn restore_lid_and_exit() -> ! {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let dir = std::path::PathBuf::from(appdata).join(IDENTIFIER);
        lidoverride::restore_from_disk(dir);
    }
    std::process::exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let startup = cli::parse(std::env::args().skip(1));

    tauri::Builder::default()
        // Per primo: una seconda istanza deve passare gli argomenti a questa e
        // chiudersi prima di fare qualunque altra cosa.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let parsed = cli::parse(args.iter().skip(1));
            control::apply_cli(app, parsed.action, true);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(updates::Pending::default())
        .manage(Mutex::new(popover::PopoverState::default()))
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::start_session,
            commands::stop_session,
            commands::toggle_session,
            commands::set_mode,
            commands::screen_off,
            commands::dismiss_welcome,
            commands::hide_popover,
            commands::fit_popover,
            commands::open_settings,
            commands::settings_ready,
            commands::close_settings,
            commands::get_settings,
            commands::update_settings,
            commands::set_lid,
            commands::answer_lid,
            commands::restore_lid_now,
            commands::set_then,
            commands::extend_session,
            commands::cancel_countdown,
            commands::countdown_now,
            commands::dismiss_warning,
            commands::get_toast,
            commands::toast_ready,
            commands::fit_toast,
            commands::answer_star,
            commands::check_updates,
            commands::install_update,
            commands::list_processes,
            commands::list_networks,
            commands::diagnose,
            commands::diagnose_requests,
            commands::add_rule,
            commands::update_rule,
            commands::delete_rule,
            commands::pause_rules,
            commands::resume_rules,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // Chiamato dall'installer: applica la scelta ed esce, senza finestre.
            if let cli::Action::Autostart(enable) = startup.action {
                control::set_autostart(&handle, enable);
                handle.exit(0);
                return Ok(());
            }

            // Qui e non con gli altri plugin: il Builder dell'updater vuole un AppHandle.
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            let dir = app.path().app_data_dir()?;
            let paths = Paths {
                settings: dir.join("settings.json"),
                state: dir.join("state.json"),
                lid_log: dir.join(lidoverride::FILE_NAME),
            };
            let version = app.package_info().version.to_string();
            let core = Core::load(paths, version, sys::now());
            let show_welcome = !core.memory.welcome_done && !startup.from_autostart;
            app.manage(AppState {
                core: Mutex::new(core),
                wake: Condvar::new(),
                ui: Mutex::new(UiCache::default()),
            });

            // L'icona nasce vuota: la disegna `refresh`, come ogni volta dopo.
            TrayIconBuilder::with_id(TRAY_ID)
                .icon(tray::icon(
                    tray::IconState::Off,
                    sys::taskbar_is_light(),
                    16,
                ))
                .tooltip("Moka")
                .show_menu_on_left_click(false)
                .on_menu_event(on_menu_event)
                .on_tray_icon_event(|icon, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        let app = icon.app_handle();
                        let left = app
                            .state::<AppState>()
                            .core
                            .lock()
                            .unwrap()
                            .settings
                            .left_click;
                        match left {
                            LeftClick::Popover => {
                                popover::toggle(app, Some(popover::anchor_from_rect(&rect)))
                            }
                            LeftClick::Toggle => control::toggle(app),
                        }
                    }
                })
                .build(app)?;

            control::request_refresh(&handle, true);
            // Il pannello nasce nascosto: memoria bassa da subito.
            if let Some(win) = handle.get_webview_window(popover::LABEL) {
                popover::set_memory_low(&win, true);
            }
            spawn_ticker(handle.clone());
            {
                let h = handle.clone();
                sys::watch_taskbar_theme(move || control::request_refresh(&h, false));
            }
            {
                let h = handle.clone();
                sysevents::spawn(move |event| control::on_sys_event(&h, event));
            }

            control::apply_shortcuts(&handle);
            updates::spawn_checker(handle.clone());
            spawn_rules(handle.clone());
            presence::spawn(handle.clone());

            control::apply_cli(&handle, startup.action.clone(), false);

            if show_welcome {
                // L'icona appena creata ha bisogno di un attimo prima che
                // Windows ne conosca la posizione.
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(900));
                    let h2 = h.clone();
                    let _ = h.run_on_main_thread(move || control::show_popover_at_tray(&h2));
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            (popover::LABEL, WindowEvent::Focused(false)) => popover::on_blur(window.app_handle()),
            (popover::LABEL, WindowEvent::CloseRequested { api, .. }) => {
                // Il pannello non si chiude mai, si nasconde (trappola 2).
                api.prevent_close();
                popover::hide(window.app_handle());
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("avvio di Moka")
        .run(|app, event| match event {
            RunEvent::ExitRequested { api, code, .. } => {
                // Chiudere le Impostazioni non deve chiudere Moka: esce solo
                // chi lo chiede esplicitamente (app.exit, con un codice).
                if code.is_none() {
                    api.prevent_exit();
                }
            }
            RunEvent::Exit => {
                // Qualunque sia la strada per uscire, niente resta cambiato.
                if let Some(st) = app.try_state::<AppState>() {
                    st.core.lock().unwrap().shutdown();
                }
            }
            _ => {}
        });
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id.as_ref();
    match id {
        "toggle" => control::toggle(app),
        "for:never" => control::start(app, None, Some(Spec::Never), None, None),
        "until" => {
            control::show_popover_at_tray(app);
            let _ = app.emit_to(popover::LABEL, "moka://focus-until", ());
        }
        "screen" => {
            let st = app.state::<AppState>();
            let current = {
                let core = st.core.lock().unwrap();
                core.session
                    .map(|s| s.mode)
                    .unwrap_or(core.memory.last_mode)
            };
            let next = if current == Mode::Display {
                Mode::System
            } else {
                Mode::Display
            };
            control::set_mode(app, next);
        }
        "lid" => {
            let st = app.state::<AppState>();
            let current = {
                let core = st.core.lock().unwrap();
                core.session.map(|s| s.lid).unwrap_or(core.memory.last_lid)
            };
            control::set_lid(app, !current);
        }
        "screen_off" => control::screen_off(app),
        "rules" => {
            let paused = app
                .state::<AppState>()
                .core
                .lock()
                .unwrap()
                .rules
                .paused
                .is_some();
            control::with_core(app, |c| {
                if paused {
                    c.resume_rules(sys::now());
                } else {
                    c.pause_rules(Some(cli::PAUSE_DEFAULT_MINUTES), sys::now());
                }
            });
        }
        "open" => control::show_popover_at_tray(app),
        "settings" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = commands::show_settings(app) {
                    eprintln!("moka: impostazioni non aperte: {err}");
                }
            });
        }
        "diagnose" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = commands::show_settings_at(app, "diagnose") {
                    eprintln!("moka: impostazioni non aperte: {err}");
                }
            });
        }
        "quit" => control::quit(app),
        other => {
            if let Some(minutes) = other.strip_prefix("for:").and_then(|m| m.parse().ok()) {
                control::start(app, None, Some(Spec::Minutes { minutes }), None, None);
            }
        }
    }
}

/// Le regole automatiche: un controllo ogni 5 secondi (roadmap 0.4).
fn spawn_rules(app: AppHandle) {
    std::thread::Builder::new()
        .name("moka-rules".into())
        .spawn(move || {
            let mut probes = probes::Probes::default();
            loop {
                control::rules_tick(&app, &mut probes);
                std::thread::sleep(Duration::from_secs(5));
            }
        })
        .expect("thread delle regole");
}

/// Il timer: chiude le sessioni scadute e tiene aggiornato il tempo residuo
/// nel tooltip. Dorme fino al prossimo evento utile; chi cambia la sessione
/// lo sveglia (vedi `generation` in `state.rs`).
fn spawn_ticker(app: AppHandle) {
    std::thread::Builder::new()
        .name("moka-ticker".into())
        .spawn(move || {
            let st = app.state::<AppState>();
            let mut core = st.core.lock().unwrap();
            loop {
                let now = sys::now();
                let changed = core.on_tick(now);
                let wait = core.next_wait(now);
                let seen = core.generation;
                let effects = std::mem::take(&mut core.effects);
                let modern_standby = core.lid.caps.modern_standby;
                drop(core);
                control::request_refresh(&app, changed || !effects.is_empty());
                control::run_effects(effects, modern_standby);
                core = st.core.lock().unwrap();
                if core.generation == seen {
                    core = st.wake.wait_timeout(core, wait).unwrap().0;
                }
            }
        })
        .expect("thread del timer");
}
