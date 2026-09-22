//! I comandi che le pagine chiamano con `invoke`.
//!
//! I comandi sincroni girano sul thread principale; `open_settings` è
//! asincrono perché crea una finestra, e su Windows crearla dal thread
//! principale dentro un comando sincrono va in deadlock.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State, WebviewWindow, WebviewWindowBuilder};

use crate::control;
use crate::i18n::{duration_label, lid_action_label, t, tv, Lang};
use crate::popover;
use crate::session::{format_duration_compact, Mode, Spec};
use crate::settings::{
    parse_durations_text, spec_is_valid, LangSetting, LeftClick, LidMode, BACKPACK_CHOICES,
    BATTERY_CHOICES,
};
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
    control::start(&app, mode, Some(spec), None);
    Ok(())
}

/// "Anche a coperchio chiuso" nel pannello.
#[tauri::command]
pub fn set_lid(app: AppHandle, on: bool) {
    control::set_lid(&app, on);
}

/// La risposta alla domanda del primo avvio su un portatile.
#[tauri::command]
pub fn answer_lid(app: AppHandle, mode: LidMode, desk: bool) -> Result<(), String> {
    control::with_core(&app, |c| c.answer_lid(mode, desk)).map_err(|e| e.to_string())
}

/// "Ripristina ora" nelle Impostazioni.
#[tauri::command]
pub fn restore_lid_now(app: AppHandle) -> SettingsDto {
    control::with_core(&app, |c| c.restore_lid_now(sys::now()));
    settings_dto(&app)
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
pub struct ChoiceDto {
    pub value: u32,
    pub label: String,
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
    /// C'è un coperchio: la sezione Coperchio esiste solo qui.
    pub laptop: bool,
    pub has_battery: bool,
    pub lid_mode: LidMode,
    pub desk_mode: bool,
    pub lock_on_lid_open: bool,
    pub backpack_minutes: u32,
    pub backpack_choices: Vec<ChoiceDto>,
    pub battery_threshold: u32,
    pub battery_choices: Vec<ChoiceDto>,
    pub policy_managed: bool,
    /// "Impostazione di Windows: Sospendi in carica, Sospendi a batteria."
    pub windows_lid: String,
    /// Moka la sta tenendo su "non fare nulla" adesso.
    pub lid_held: bool,
    pub lid_error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub language: Option<LangSetting>,
    pub left_click: Option<LeftClick>,
    pub durations_text: Option<String>,
    pub autostart: Option<bool>,
    pub lid_mode: Option<LidMode>,
    pub desk_mode: Option<bool>,
    pub lock_on_lid_open: Option<bool>,
    pub backpack_minutes: Option<u32>,
    pub battery_threshold: Option<u32>,
}

fn settings_dto(app: &AppHandle) -> SettingsDto {
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    let core = app.state::<AppState>();
    let core = core.core.lock().unwrap();
    let lang = core.lang;
    let original = core.lid.original();
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
        lang,
        version: core.version.clone(),
        laptop: core.lid.caps.lid_present,
        has_battery: core.lid.caps.batteries,
        lid_mode: core.settings.lid_mode,
        desk_mode: core.settings.desk_mode,
        lock_on_lid_open: core.settings.lock_on_lid_open,
        backpack_minutes: core.settings.backpack_minutes,
        backpack_choices: BACKPACK_CHOICES
            .iter()
            .map(|&value| ChoiceDto {
                value,
                label: if value == 0 {
                    t(lang, "settings.backpack_never")
                } else {
                    duration_label(lang, value)
                },
            })
            .collect(),
        battery_threshold: core.settings.battery_threshold,
        battery_choices: BATTERY_CHOICES
            .iter()
            .map(|&value| ChoiceDto {
                value,
                label: if value == 0 {
                    t(lang, "settings.battery_off")
                } else {
                    format!("{value}%")
                },
            })
            .collect(),
        policy_managed: core.lid.policy_managed,
        windows_lid: original
            .map(|o| {
                tv(
                    lang,
                    "settings.windows_now",
                    &[
                        ("ac", &lid_action_label(lang, o.ac)),
                        ("dc", &lid_action_label(lang, o.dc)),
                    ],
                )
            })
            .unwrap_or_default(),
        lid_held: core.lid.held(),
        lid_error: core
            .lid
            .last_error
            .as_ref()
            .map(|e| tv(lang, "settings.lid_error", &[("error", e)])),
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
        if let Some(mode) = patch.lid_mode {
            // Sceglierlo nelle Impostazioni vale come risposta alla domanda.
            core.settings.lid_mode = mode;
            core.settings.lid_asked = true;
        }
        if let Some(desk) = patch.desk_mode {
            core.settings.desk_mode = desk;
            core.settings.lid_asked = true;
        }
        if let Some(lock) = patch.lock_on_lid_open {
            core.settings.lock_on_lid_open = lock;
        }
        if let Some(m) = patch
            .backpack_minutes
            .filter(|m| BACKPACK_CHOICES.contains(m))
        {
            core.settings.backpack_minutes = m;
        }
        if let Some(b) = patch
            .battery_threshold
            .filter(|b| BATTERY_CHOICES.contains(b))
        {
            core.settings.battery_threshold = b;
        }
        core.save_settings()
            .map_err(|e| tv(lang, "settings.save_error", &[("error", &e.to_string())]))
    });
    result.map(|()| settings_dto(&app))
}
