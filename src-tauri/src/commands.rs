//! I comandi che le pagine chiamano con `invoke`.
//!
//! I comandi sincroni girano sul thread principale; `open_settings` è
//! asincrono perché crea una finestra, e su Windows crearla dal thread
//! principale dentro un comando sincrono va in deadlock.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State, WebviewWindow, WebviewWindowBuilder};

use crate::control;
use crate::i18n::{t, tv, Lang};
use crate::popover;
use crate::session::{format_duration_compact, Mode, Spec};
use crate::settings::{parse_durations_text, spec_is_valid, LangSetting, LeftClick};
use crate::state::{AppState, StateDto};
use crate::sys;

pub const SETTINGS_LABEL: &str = "settings";

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> StateDto {
    state.core.lock().unwrap().view(sys::now()).dto
}

#[tauri::command]
pub fn start_session(app: AppHandle, spec: Spec, mode: Option<Mode>) -> Result<(), String> {
    if !spec_is_valid(spec) {
        return Err(format!("durata non valida: {spec:?}"));
    }
    control::start(&app, mode, Some(spec));
    Ok(())
}

#[tauri::command]
pub fn stop_session(app: AppHandle) {
    control::stop(&app);
}

#[tauri::command]
pub fn toggle_session(app: AppHandle) {
    control::toggle(&app);
}

#[tauri::command]
pub fn set_mode(app: AppHandle, mode: Mode) {
    control::set_mode(&app, mode);
}

#[tauri::command]
pub fn screen_off(app: AppHandle) {
    control::screen_off(&app);
}

#[tauri::command]
pub fn dismiss_welcome(app: AppHandle) {
    control::with_core(&app, |c| c.dismiss_welcome());
}

#[tauri::command]
pub fn hide_popover(app: AppHandle) {
    popover::hide(&app);
}

#[tauri::command]
pub fn fit_popover(app: AppHandle, height: f64) {
    popover::fit(&app, height);
}

#[tauri::command]
pub async fn open_settings(app: AppHandle) -> Result<(), String> {
    show_settings(app).map_err(|e| e.to_string())
}

/// Apre le Impostazioni. La finestra si crea solo quando serve e si distrugge
/// alla chiusura: una WebView2 costa decine di MB, e Moka passa quasi tutto il
/// tempo con le Impostazioni chiuse. Va chiamata da un thread che non sia il
/// principale.
pub fn show_settings(app: AppHandle) -> tauri::Result<()> {
    popover::hide(&app);
    if let Some(win) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = win.unminimize();
        win.show()?;
        win.set_focus()?;
        return Ok(());
    }
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == SETTINGS_LABEL)
        .cloned()
        .expect("finestra settings dichiarata in tauri.conf.json");
    let lang = app.state::<AppState>().core.lock().unwrap().lang;
    let win = WebviewWindowBuilder::from_config(&app, &config)?
        .title(t(lang, "settings.window_title"))
        .build()?;
    // Si mostra quando la pagina è pronta (`settings_ready`), per non far
    // vedere un rettangolo bianco; se per qualche motivo non lo dice, dopo 2 s
    // si mostra comunque.
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        if !win.is_visible().unwrap_or(true) {
            let _ = win.show();
            let _ = win.set_focus();
        }
    });
    Ok(())
}

/// Esc nelle Impostazioni: la finestra si chiude davvero (e libera memoria).
#[tauri::command]
pub fn close_settings(window: WebviewWindow) {
    let _ = window.close();
}

#[tauri::command]
pub fn settings_ready(window: WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub language: LangSetting,
    pub left_click: LeftClick,
    pub durations_text: String,
    pub autostart: bool,
    pub lang: Lang,
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub language: Option<LangSetting>,
    pub left_click: Option<LeftClick>,
    pub durations_text: Option<String>,
    pub autostart: Option<bool>,
}

fn settings_dto(app: &AppHandle) -> SettingsDto {
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    let core = app.state::<AppState>();
    let core = core.core.lock().unwrap();
    SettingsDto {
        language: core.settings.language,
        left_click: core.settings.left_click,
        durations_text: core
            .settings
            .durations
            .iter()
            .map(|m| format_duration_compact(*m))
            .collect::<Vec<_>>()
            .join(", "),
        autostart,
        lang: core.lang,
        version: core.version.clone(),
    }
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> SettingsDto {
    settings_dto(&app)
}

/// Salva un pezzo delle impostazioni. Un errore torna già tradotto, pronto da
/// mostrare accanto al campo.
#[tauri::command]
pub fn update_settings(app: AppHandle, patch: SettingsPatch) -> Result<SettingsDto, String> {
    if let Some(enable) = patch.autostart {
        control::set_autostart(&app, enable);
    }
    let result = control::with_core(&app, |core| {
        let lang = core.lang;
        if let Some(text) = &patch.durations_text {
            core.settings.durations = parse_durations_text(text)
                .map_err(|bad| tv(lang, "settings.durations_invalid", &[("value", &bad)]))?;
        }
        if let Some(language) = patch.language {
            core.settings.language = language;
        }
        if let Some(left) = patch.left_click {
            core.settings.left_click = left;
        }
        core.save_settings()
            .map_err(|e| tv(lang, "settings.save_error", &[("error", &e.to_string())]))
    });
    result.map(|()| settings_dto(&app))
}
