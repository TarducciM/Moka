//! Tasti rapidi globali: accendi/spegni e "spegni lo schermo ora".
//!
//! Si scelgono da una lista di combinazioni sicure (`SHORTCUT_CHOICES`), mai
//! Ctrl+Alt (sulle tastiere italiane è AltGr, trappola 19). Se un'altra app ha
//! già preso la combinazione, la registrazione fallisce e le Impostazioni lo
//! dicono invece di far finta di niente.

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::control;

/// Registra i tasti scelti al posto dei precedenti. Restituisce l'errore da
/// mostrare, se una combinazione non è disponibile.
pub fn register(app: &AppHandle, toggle: &str, screen_off: &str) -> Option<String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut failed = Vec::new();
    if !toggle.is_empty() {
        let ok = gs.on_shortcut(toggle, |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                control::toggle(app);
            }
        });
        if ok.is_err() {
            failed.push(toggle.to_owned());
        }
    }
    if !screen_off.is_empty() {
        let ok = gs.on_shortcut(screen_off, |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                control::screen_off(app);
            }
        });
        if ok.is_err() {
            failed.push(screen_off.to_owned());
        }
    }
    (!failed.is_empty()).then(|| failed.join(", "))
}
