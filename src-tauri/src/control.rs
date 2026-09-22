//! Le azioni, in un posto solo: le usano il pannello, il menu della tray, il
//! clic sull'icona, la riga di comando e gli eventi di sistema. Ognuna cambia
//! lo stato sotto lock e poi chiede di ridisegnare tray e pannello.
//!
//! Tray e menu si toccano **solo dal thread principale** ([`request_refresh`]
//! ci passa sempre): le API della tray, chiamate da un altro thread, aspettano
//! il principale, e un lock tenuto nel frattempo è un deadlock servito.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindowBuilder};
use tauri_plugin_notification::NotificationExt;

use crate::actions;
use crate::cli::Action;
use crate::i18n::{t, tv, Lang};
use crate::lidplan::Effect;
use crate::popover;
use crate::probes::Probes;
use crate::session::{Mode, Spec, ThenAct};
use crate::state::{AppState, Core, Notice};
use crate::sys;
use crate::sysevents::SysEvent;
use crate::tray::{self, MenuView, TrayMenu};

pub const TRAY_ID: &str = "moka";

/// Esegue `f` sullo stato e poi aggiorna tutto. Restituisce ciò che `f` restituisce.
pub fn with_core<T>(app: &AppHandle, f: impl FnOnce(&mut Core) -> T) -> T {
    let st = app.state::<AppState>();
    let (out, effects, notices, lang, modern_standby) = {
        let mut core = st.core.lock().unwrap();
        let out = f(&mut core);
        (
            out,
            std::mem::take(&mut core.effects),
            std::mem::take(&mut core.notices),
            core.lang,
            core.lid.caps.modern_standby,
        )
    };
    st.wake.notify_all();
    request_refresh(app, true);
    run_effects(effects, modern_standby);
    show_notices(app, lang, notices);
    out
}

/// Il giro delle regole (thread `moka-rules`, ogni 5 s). Le sonde girano
/// fuori dal lock: elenco dei processi e registro costano qualche
/// millisecondo, e il pannello non deve aspettarle.
pub fn rules_tick(app: &AppHandle, probes: &mut Probes) {
    let st = app.state::<AppState>();
    let needs = st.core.lock().unwrap().rules_needs();
    let seen = probes.observe(needs, sys::now().tick_ms);
    let changed = st.core.lock().unwrap().update_rules(seen, sys::now());
    if changed {
        // Solo per svuotare effetti e notifiche e ridisegnare.
        with_core(app, |_| ());
    }
}

/// Le azioni sul sistema, fuori dal lock e su un thread a parte: una
/// sospensione ritorna solo al risveglio.
pub fn run_effects(effects: Vec<Effect>, modern_standby: bool) {
    if effects.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        for effect in effects {
            match effect {
                Effect::Lock => actions::lock(),
                Effect::ScreenOff => sys::screen_off(None),
                Effect::Perform(act) => {
                    if let Err(err) = actions::perform(act, modern_standby) {
                        eprintln!("moka: {act:?} non eseguita: {err}");
                    }
                }
            }
        }
    });
}

fn show_notices(app: &AppHandle, lang: Lang, notices: Vec<Notice>) {
    for notice in notices {
        match notice {
            Notice::BatteryStopped(percent) => {
                let _ = app
                    .notification()
                    .builder()
                    .title(t(lang, "notify.battery_title"))
                    .body(tv(
                        lang,
                        "notify.battery_body",
                        &[("percent", &percent.to_string())],
                    ))
                    .show();
            }
            Notice::WhileNotFound(name) => {
                let _ = app
                    .notification()
                    .builder()
                    .title(t(lang, "notify.while_title"))
                    .body(tv(lang, "notify.while_body", &[("name", &name)]))
                    .show();
            }
        }
    }
}

pub fn start(
    app: &AppHandle,
    mode: Option<Mode>,
    spec: Option<Spec>,
    lid: Option<bool>,
    then: Option<ThenAct>,
) {
    with_core(app, |c| c.start(mode, spec, lid, then, sys::now()));
}

/// Registra i tasti rapidi delle impostazioni e ricorda se qualcuno non va.
pub fn apply_shortcuts(app: &AppHandle) {
    let (toggle, screen_off) = {
        let st = app.state::<AppState>();
        let core = st.core.lock().unwrap();
        (
            core.settings.shortcut_toggle.clone(),
            core.settings.shortcut_screen_off.clone(),
        )
    };
    let error = crate::shortcuts::register(app, &toggle, &screen_off);
    with_core(app, |c| c.shortcut_error = error);
}

pub fn stop(app: &AppHandle) {
    with_core(app, |c| c.stop(sys::now()));
}

pub fn toggle(app: &AppHandle) {
    with_core(app, |c| c.toggle(sys::now()));
}

pub fn set_mode(app: &AppHandle, mode: Mode) {
    with_core(app, |c| c.set_mode(mode, sys::now()));
}

pub fn set_lid(app: &AppHandle, on: bool) {
    with_core(app, |c| c.set_lid(on, sys::now()));
}

pub fn on_sys_event(app: &AppHandle, event: SysEvent) {
    with_core(app, |c| c.on_sys_event(event, sys::now()));
}

/// Spegne lo schermo lasciando il PC sveglio. L'attesa prima di spegnere non
/// è estetica: il clic che l'ha chiesto sta ancora finendo (rilascio del
/// tasto, piccolo movimento del mouse), e un input subito dopo lo
/// spegnimento lo riaccenderebbe.
pub fn screen_off(app: &AppHandle) {
    with_core(app, |c| c.prepare_screen_off(sys::now()));
    popover::hide(app);
    let hwnd = app
        .get_webview_window(popover::LABEL)
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(700));
        let hwnd = hwnd.map(|h| windows::Win32::Foundation::HWND(h as *mut core::ffi::c_void));
        sys::screen_off(hwnd);
    });
}

/// Esci: la sessione finisce davvero (non viene ripresa al prossimo avvio) e
/// l'impostazione del coperchio torna com'era.
pub fn quit(app: &AppHandle) {
    {
        let st = app.state::<AppState>();
        let mut core = st.core.lock().unwrap();
        core.stop(sys::now());
        core.shutdown();
    }
    app.exit(0);
}

/// Applica un comando arrivato dalla riga di comando. `from_second_instance`:
/// senza comandi, chi rilancia Moka vuole vederla, quindi si apre il pannello.
pub fn apply_cli(app: &AppHandle, action: Action, from_second_instance: bool) {
    match action {
        Action::None => {
            if from_second_instance {
                show_popover_at_tray(app);
            }
        }
        Action::Start {
            mode,
            spec,
            lid,
            then,
        } => start(app, mode, spec, lid, then),
        Action::Off => stop(app),
        Action::Toggle => toggle(app),
        Action::ScreenOff => screen_off(app),
        Action::Quit => quit(app),
        Action::Autostart(enable) => set_autostart(app, enable),
        Action::SetThen(act) => {
            with_core(app, |c| c.set_then(act, sys::now()));
        }
        Action::While { kind, mode, then } => {
            with_core(app, |c| c.add_temp_rule(kind, mode, then, sys::now()));
        }
        Action::PauseRules(minutes) => {
            with_core(app, |c| c.pause_rules(Some(minutes), sys::now()));
        }
        Action::ResumeRules => {
            with_core(app, |c| c.resume_rules(sys::now()));
        }
        // Gestita in main.rs, prima di avviare Tauri: qui non arriva mai.
        Action::RestoreLid => {}
    }
}

pub fn set_autostart(app: &AppHandle, enable: bool) {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();
    // Solo se cambia davvero: disattivare ciò che non è attivo dà "os error 2"
    // (trappola 7).
    if launcher.is_enabled().unwrap_or(!enable) == enable {
        return;
    }
    let result = if enable {
        launcher.enable()
    } else {
        launcher.disable()
    };
    if let Err(err) = result {
        eprintln!("moka: avvio automatico non modificato: {err}");
    }
}

pub fn tray_anchor(app: &AppHandle) -> Option<popover::Anchor> {
    app.tray_by_id(TRAY_ID)
        .and_then(|t| t.rect().ok().flatten())
        .map(|r| popover::anchor_from_rect(&r))
}

pub fn show_popover_at_tray(app: &AppHandle) {
    let anchor = tray_anchor(app);
    popover::show(app, anchor);
}

/// Chiede di ridisegnare tray e pannello, sul thread principale.
pub fn request_refresh(app: &AppHandle, emit: bool) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || refresh(&handle, emit));
}

/// Aggiorna icona, tooltip e menu solo dove qualcosa è cambiato, e manda lo
/// stato alle pagine.
fn refresh(app: &AppHandle, emit: bool) {
    let st = app.state::<AppState>();
    let now = sys::now();
    let (view, lang, durations, toast) = {
        let core = st.core.lock().unwrap();
        (
            core.view(now),
            core.lang,
            core.settings.durations.clone(),
            core.toast(now),
        )
    };
    sync_toast(app, toast.is_some());
    let Some(tray_icon) = app.tray_by_id(TRAY_ID) else {
        return;
    };

    let mut ui = st.ui.lock().unwrap();
    let key = (
        view.icon,
        sys::taskbar_is_light(),
        tray::pick_size(sys::system_dpi()),
    );
    if ui.icon_key != Some(key) {
        let _ = tray_icon.set_icon(Some(tray::icon(key.0, key.1, key.2)));
        ui.icon_key = Some(key);
    }
    if ui.tooltip != view.tooltip {
        let _ = tray_icon.set_tooltip(Some(&view.tooltip));
        ui.tooltip = view.tooltip.clone();
    }

    let menu_key = (lang, durations, view.dto.lid_row, view.dto.has_rules);
    if ui.menu_key.as_ref() != Some(&menu_key) {
        match TrayMenu::build(app, lang, &menu_key.1, menu_key.2, menu_key.3) {
            Ok(menu) => {
                let _ = tray_icon.set_menu(Some(menu.menu.clone()));
                ui.menu = Some(menu);
                ui.menu_key = Some(menu_key);
                ui.menu_view = None;
            }
            Err(err) => eprintln!("moka: menu non costruito: {err}"),
        }
    }
    let paused = view.dto.rules_paused.is_some();
    let menu_view = (
        view.status.clone(),
        view.active,
        view.display,
        view.dto.lid,
        paused,
    );
    if ui.menu_view.as_ref() != Some(&menu_view) {
        if let Some(menu) = &ui.menu {
            menu.update(
                lang,
                &MenuView {
                    status: &view.status,
                    active: view.active,
                    display: view.display,
                    lid: view.dto.lid,
                    rules_paused: paused,
                },
            );
        }
        ui.menu_view = Some(menu_view);
    }
    drop(ui);

    if let Some(settings) = app.get_webview_window("settings") {
        let _ = settings.set_title(&t(lang, "settings.window_title"));
    }
    if emit {
        let _ = app.emit("moka://state", &view.dto);
    }
}

/// La finestrella degli avvisi ("si spegne tra 5 minuti", il conto alla
/// rovescia di "…e poi"): si crea quando serve e si chiude quando non serve
/// più. La crea un thread a parte: dal thread principale, dentro un evento,
/// la creazione di una finestra va in deadlock su Windows.
pub const TOAST_LABEL: &str = "toast";
static TOAST_CREATING: AtomicBool = AtomicBool::new(false);

fn sync_toast(app: &AppHandle, want: bool) {
    match (want, app.get_webview_window(TOAST_LABEL)) {
        (true, Some(_)) => {
            let _ = app.emit_to(TOAST_LABEL, "moka://toast", ());
        }
        (true, None) => {
            if !TOAST_CREATING.swap(true, Ordering::SeqCst) {
                let app = app.clone();
                std::thread::spawn(move || {
                    if let Err(err) = create_toast(&app) {
                        eprintln!("moka: avviso non mostrato: {err}");
                    }
                    TOAST_CREATING.store(false, Ordering::SeqCst);
                });
            }
        }
        (false, Some(win)) => {
            let _ = win.close();
        }
        (false, None) => {}
    }
}

fn create_toast(app: &AppHandle) -> tauri::Result<()> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == TOAST_LABEL)
        .cloned()
        .expect("finestra toast dichiarata in tauri.conf.json");
    WebviewWindowBuilder::from_config(app, &config)?.build()?;
    Ok(())
}

/// In basso a destra del monitor principale, sopra la barra delle
/// applicazioni, con l'altezza che serve al contenuto.
pub fn place_toast(app: &AppHandle, content_height: f64) {
    let Some(win) = app.get_webview_window(TOAST_LABEL) else {
        return;
    };
    let _ = win.set_size(LogicalSize::new(
        360.0,
        content_height.clamp(80.0, 400.0).ceil(),
    ));
    let Some(monitor) = app.primary_monitor().ok().flatten() else {
        return;
    };
    let Ok(size) = win.outer_size() else { return };
    let area = monitor.work_area();
    let margin = (16.0 * monitor.scale_factor()).round() as i32;
    let x = area.position.x + area.size.width as i32 - size.width as i32 - margin;
    let y = area.position.y + area.size.height as i32 - size.height as i32 - margin;
    let _ = win.set_position(PhysicalPosition::new(x, y));
}
