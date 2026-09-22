//! Le azioni, in un posto solo: le usano il pannello, il menu della tray, il
//! clic sull'icona e la riga di comando. Ognuna cambia lo stato sotto lock e
//! poi chiede di ridisegnare tray e pannello.
//!
//! Tray e menu si toccano **solo dal thread principale** ([`request_refresh`]
//! ci passa sempre): le API della tray, chiamate da un altro thread, aspettano
//! il principale, e un lock tenuto nel frattempo è un deadlock servito.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::cli::Action;
use crate::i18n::t;
use crate::popover;
use crate::session::{Mode, Spec};
use crate::state::{AppState, Core};
use crate::sys;
use crate::tray::{self, MenuView, TrayMenu};

pub const TRAY_ID: &str = "moka";

/// Esegue `f` sullo stato e poi aggiorna tutto. Restituisce ciò che `f` restituisce.
pub fn with_core<T>(app: &AppHandle, f: impl FnOnce(&mut Core) -> T) -> T {
    let st = app.state::<AppState>();
    let out = {
        let mut core = st.core.lock().unwrap();
        f(&mut core)
    };
    st.wake.notify_all();
    request_refresh(app, true);
    out
}

pub fn start(app: &AppHandle, mode: Option<Mode>, spec: Option<Spec>) {
    with_core(app, |c| c.start(mode, spec, sys::now()));
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

/// Esci: la sessione finisce davvero (non viene ripresa al prossimo avvio).
pub fn quit(app: &AppHandle) {
    {
        let st = app.state::<AppState>();
        let mut core = st.core.lock().unwrap();
        core.stop(sys::now());
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
        Action::Start { mode, spec } => start(app, mode, spec),
        Action::Off => stop(app),
        Action::Toggle => toggle(app),
        Action::ScreenOff => screen_off(app),
        Action::Quit => quit(app),
        Action::Autostart(enable) => set_autostart(app, enable),
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
    let (view, lang, durations) = {
        let core = st.core.lock().unwrap();
        (core.view(now), core.lang, core.settings.durations.clone())
    };
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

    let menu_key = (lang, durations);
    if ui.menu_key.as_ref() != Some(&menu_key) {
        match TrayMenu::build(app, lang, &menu_key.1) {
            Ok(menu) => {
                let _ = tray_icon.set_menu(Some(menu.menu.clone()));
                ui.menu = Some(menu);
                ui.menu_key = Some(menu_key);
                ui.menu_view = None;
            }
            Err(err) => eprintln!("moka: menu non costruito: {err}"),
        }
    }
    let menu_view = (view.status.clone(), view.active, view.display);
    if ui.menu_view.as_ref() != Some(&menu_view) {
        if let Some(menu) = &ui.menu {
            menu.update(
                lang,
                &MenuView {
                    status: &view.status,
                    active: view.active,
                    display: view.display,
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
