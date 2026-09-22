//! I comandi che le pagine chiamano con `invoke`.
//!
//! I comandi sincroni girano sul thread principale; `open_settings` è
//! asincrono perché crea una finestra, e su Windows crearla dal thread
//! principale dentro un comando sincrono va in deadlock.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, WebviewWindowBuilder};

use crate::capabilities;
use crate::control;
use crate::diagnose::{self, note, ElevatedError, NoteDto, RequestDto, RestDto};
use crate::i18n;
use crate::i18n::{duration_label, lid_action_label, t, tv, Lang};
use crate::popover;
use crate::probes;
use crate::rules::{RuleKind, CPU_CHOICES, DOWNLOAD_CHOICES};
use crate::session::{format_duration_compact, Mode, Spec, ThenAct};
use crate::settings::{
    normalize_shortcut, parse_durations_text, spec_is_valid, LangSetting, LeftClick, LidMode,
    BACKPACK_CHOICES, BATTERY_CHOICES, SHORTCUT_CHOICES,
};
use crate::state::{AppState, RuleError, StateDto, ThenChoice, ToastDto};
use crate::sys;
use crate::updates;

pub const SETTINGS_LABEL: &str = "settings";

/// La sezione da mostrare all'apertura delle Impostazioni ("Perché non
/// dorme?…" dal menu apre la diagnostica). La prende `settings_ready`.
static SETTINGS_SECTION: Mutex<Option<&'static str>> = Mutex::new(None);

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> StateDto {
    state.core.lock().unwrap().view(sys::now()).dto
}

#[tauri::command]
pub fn start_session(app: AppHandle, spec: Spec, mode: Option<Mode>) -> Result<(), String> {
    if !spec_is_valid(spec) {
        return Err(format!("durata non valida: {spec:?}"));
    }
    control::start(&app, mode, Some(spec), None, None);
    Ok(())
}

/// "Poi:" nel pannello.
#[tauri::command]
pub fn set_then(app: AppHandle, then: ThenAct) {
    control::with_core(&app, |c| c.set_then(then, sys::now()));
}

/// "+30 min", dall'avviso o dal conto alla rovescia.
#[tauri::command]
pub fn extend_session(app: AppHandle, minutes: u32) {
    let minutes = minutes.clamp(1, 24 * 60);
    control::with_core(&app, |c| c.extend(minutes, sys::now()));
}

#[tauri::command]
pub fn cancel_countdown(app: AppHandle) {
    control::with_core(&app, |c| c.cancel_countdown(sys::now()));
}

#[tauri::command]
pub fn countdown_now(app: AppHandle) {
    control::with_core(&app, |c| c.countdown_now(sys::now()));
}

#[tauri::command]
pub fn dismiss_warning(app: AppHandle) {
    control::with_core(&app, |c| c.dismiss_warning());
}

/// Cosa mostra la finestrella degli avvisi.
#[tauri::command]
pub fn get_toast(state: State<'_, AppState>) -> Option<ToastDto> {
    state.core.lock().unwrap().toast(sys::now())
}

/// La finestrella è pronta: si mostra senza rubare il focus a chi lavora.
#[tauri::command]
pub fn toast_ready(app: AppHandle, height: f64) {
    control::place_toast(&app, height);
    if let Some(win) = app.get_webview_window(control::TOAST_LABEL) {
        let _ = win.show();
    }
}

#[tauri::command]
pub fn fit_toast(app: AppHandle, height: f64) {
    control::place_toast(&app, height);
}

/// Promemoria stella: "più tardi" (`never: false`) o "non mostrare più".
#[tauri::command]
pub fn answer_star(app: AppHandle, never: bool) {
    control::with_core(&app, |c| c.answer_star(never, sys::now()));
}

#[tauri::command]
pub async fn check_updates(app: AppHandle) -> Result<Option<String>, String> {
    updates::check(&app).await
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    updates::install(&app).await
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

/// Apre le Impostazioni su una sezione. Se la finestra è già aperta glielo
/// dice con un evento; altrimenti la sezione aspetta `settings_ready`.
pub fn show_settings_at(app: AppHandle, section: &'static str) -> tauri::Result<()> {
    if app.get_webview_window(SETTINGS_LABEL).is_some() {
        let _ = app.emit_to(SETTINGS_LABEL, "moka://settings-section", section);
    } else {
        *SETTINGS_SECTION.lock().unwrap() = Some(section);
    }
    show_settings(app)
}

/// La pagina è pronta: si mostra, e prende la sezione chiesta (se c'è).
#[tauri::command]
pub fn settings_ready(window: WebviewWindow) -> Option<String> {
    let _ = window.show();
    let _ = window.set_focus();
    SETTINGS_SECTION.lock().unwrap().take().map(str::to_owned)
}

/// La diagnostica, già a parole.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisDto {
    pub now: Vec<NoteDto>,
    pub rests: Vec<RestDto>,
    pub devices: Vec<String>,
    pub timers: Option<String>,
}

/// "Perché non dorme? Perché si è svegliato?", senza amministratore.
/// Asincrono: legge il registro eventi fuori dal thread principale.
#[tauri::command]
pub async fn diagnose(app: AppHandle) -> DiagnosisDto {
    let (lang, session_on, reasons, presence, has_battery, modern) = {
        let st = app.state::<AppState>();
        let c = st.core.lock().unwrap();
        (
            c.lang,
            c.session.is_some(),
            c.reasons(),
            c.settings.presence,
            c.lid.caps.batteries,
            c.lid.caps.modern_standby,
        )
    };
    let moka_awake = session_on || !reasons.is_empty();
    let mut now = Vec::new();
    if session_on {
        now.push(note(t(lang, "diag.now_moka_session"), false));
    }
    if !reasons.is_empty() {
        now.push(note(
            tv(
                lang,
                "diag.now_moka_rules",
                &[("reasons", &reasons.join(", "))],
            ),
            false,
        ));
    }
    if moka_awake {
        if presence {
            now.push(note(t(lang, "diag.now_presence"), true));
        }
    } else {
        now.push(note(t(lang, "diag.now_moka_off"), false));
        // Senza Moka, i bit dello stato di esecuzione sono di qualcun altro.
        let es = capabilities::execution_state().unwrap_or(0);
        now.push(if es & 1 != 0 {
            note(t(lang, "diag.now_others"), true)
        } else if es & 2 != 0 {
            note(t(lang, "diag.now_others_display"), true)
        } else {
            note(t(lang, "diag.now_nobody"), false)
        });
    }
    let sleep = diagnose::sleep_settings();
    now.extend(diagnose::sleep_notes(lang, &sleep, has_battery));
    let rests = diagnose::rests(&diagnose::recent_power_events(200));
    now.extend(diagnose::audio_note(lang, &rests));
    if modern {
        now.push(note(t(lang, "diag.now_modern"), false));
    }
    DiagnosisDto {
        now,
        rests: rests
            .iter()
            .take(8)
            .map(|r| diagnose::describe_rest(lang, r))
            .collect(),
        devices: diagnose::wake_devices(),
        timers: diagnose::timers_note(lang, &sleep),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestsDto {
    pub items: Vec<RequestDto>,
    /// Le righe di `powercfg /waketimers`, come le scrive Windows.
    pub timers: Vec<String>,
}

/// "Mostra chi lo tiene sveglio": `powercfg /requests` con il prompt
/// dell'amministratore. Aspetta la risposta dell'utente su un thread a parte.
#[tauri::command]
pub async fn diagnose_requests(app: AppHandle) -> Result<RequestsDto, String> {
    let lang = app.state::<AppState>().core.lock().unwrap().lang;
    let result = tauri::async_runtime::spawn_blocking(diagnose::elevated_powercfg)
        .await
        .map_err(|e| tv(lang, "diag.admin_failed", &[("error", &e.to_string())]))?;
    match result {
        // Senza la categoria SYSTEM l'uscita non è un elenco ma un errore di
        // Windows: dirlo, invece di rispondere "nessuno".
        Ok((requests, _)) if !requests.contains("SYSTEM:") => {
            let first = requests.lines().map(str::trim).find(|l| !l.is_empty());
            Err(tv(
                lang,
                "diag.admin_failed",
                &[("error", first.unwrap_or("?"))],
            ))
        }
        Ok((requests, timers)) => Ok(RequestsDto {
            items: diagnose::parse_requests(&requests)
                .iter()
                .filter_map(|r| diagnose::describe_request(lang, r))
                .collect(),
            timers: timers
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .take(20)
                .map(str::to_owned)
                .collect(),
        }),
        Err(ElevatedError::Cancelled) => Err(t(lang, "diag.admin_cancelled")),
        Err(ElevatedError::Failed(e)) => Err(tv(lang, "diag.admin_failed", &[("error", &e)])),
    }
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
    pub warn_before_end: bool,
    pub shortcut_toggle: String,
    pub shortcut_screen_off: String,
    pub shortcut_choices: Vec<ShortcutChoice>,
    pub shortcut_error: Option<String>,
    pub update_version: Option<String>,
    pub rules: Vec<RuleDto>,
    /// "Regole sospese fino alle 15:30".
    pub rules_paused: Option<String>,
    pub rule_kinds: Vec<ShortcutChoice>,
    pub download_choices: Vec<ChoiceDto>,
    pub cpu_choices: Vec<ChoiceDto>,
    /// "lun", "mar"… da lunedì.
    pub day_labels: Vec<String>,
    pub then_choices: Vec<ThenChoice>,
    pub presence: bool,
}

/// Una regola come la vede la pagina: testi già tradotti.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleDto {
    pub id: u32,
    pub enabled: bool,
    pub mode: Mode,
    pub then: ThenAct,
    /// "Programma aperto".
    pub title: String,
    /// "obs64.exe", "lun–ven, 09:00–18:00".
    pub detail: String,
    /// La regola sta tenendo sveglio il PC adesso.
    pub active: bool,
}

/// Una regola nuova, come la manda la pagina.
#[derive(Debug, Deserialize)]
pub struct NewRule {
    #[serde(flatten)]
    pub kind: RuleKind,
    pub mode: Mode,
    pub then: ThenAct,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutChoice {
    pub value: String,
    pub label: String,
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
    pub warn_before_end: Option<bool>,
    pub shortcut_toggle: Option<String>,
    pub shortcut_screen_off: Option<String>,
    pub presence: Option<bool>,
}

/// I tipi di regola che hanno senso su questo PC: "in carica" solo con una
/// batteria, "monitor esterno" solo su un portatile.
fn rule_kinds(lang: Lang, has_battery: bool, laptop: bool) -> Vec<ShortcutChoice> {
    [
        ("process", "settings.rule_kind_process", true),
        ("fullscreen", "settings.rule_kind_fullscreen", true),
        ("call", "settings.rule_kind_call", true),
        ("download", "settings.rule_kind_download", true),
        ("cpu", "settings.rule_kind_cpu", true),
        ("schedule", "settings.rule_kind_schedule", true),
        ("usb", "settings.rule_kind_usb", true),
        ("network", "settings.rule_kind_network", true),
        ("plugged", "settings.rule_kind_plugged", has_battery),
        ("monitor", "settings.rule_kind_monitor", laptop),
    ]
    .into_iter()
    .filter(|(_, _, ok)| *ok)
    .map(|(value, key, _)| ShortcutChoice {
        value: value.to_owned(),
        label: t(lang, key),
    })
    .collect()
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
        warn_before_end: core.settings.warn_before_end,
        shortcut_toggle: core.settings.shortcut_toggle.clone(),
        shortcut_screen_off: core.settings.shortcut_screen_off.clone(),
        shortcut_choices: SHORTCUT_CHOICES
            .iter()
            .map(|&value| ShortcutChoice {
                value: value.to_owned(),
                label: if value.is_empty() {
                    t(lang, "settings.shortcut_none")
                } else {
                    value.replace("Shift", &t(lang, "keys.shift"))
                },
            })
            .collect(),
        shortcut_error: core
            .shortcut_error
            .as_ref()
            .map(|e| tv(lang, "settings.shortcut_error", &[("keys", e)])),
        update_version: core.update_version.clone(),
        rules: core
            .settings
            .rules
            .iter()
            .map(|r| RuleDto {
                id: r.id,
                enabled: r.enabled,
                mode: r.mode,
                then: r.then,
                title: i18n::rule_title(lang, &r.kind),
                detail: i18n::rule_detail(lang, &r.kind),
                active: core.rules.active.contains(&r.id),
            })
            .collect(),
        rules_paused: core.rules_paused_label(),
        rule_kinds: rule_kinds(lang, core.lid.caps.batteries, core.lid.caps.lid_present),
        download_choices: DOWNLOAD_CHOICES
            .iter()
            .map(|&value| ChoiceDto {
                value,
                label: tv(
                    lang,
                    "rules.detail_kbps",
                    &[("rate", &i18n::rate_label(value))],
                ),
            })
            .collect(),
        cpu_choices: CPU_CHOICES
            .iter()
            .map(|&value| ChoiceDto {
                value,
                label: tv(lang, "rules.detail_cpu", &[("percent", &value.to_string())]),
            })
            .collect(),
        day_labels: (0..7).map(|d| i18n::day_short(lang, d)).collect(),
        then_choices: ThenAct::ALL
            .iter()
            .map(|&act| ThenChoice {
                value: act,
                label: i18n::then_label(lang, act),
            })
            .collect(),
        presence: core.settings.presence,
    }
}

/// I programmi con una finestra aperta, per scegliere quello della regola.
/// Asincrono: gira fuori dal thread principale.
#[tauri::command]
pub async fn list_processes() -> Vec<String> {
    probes::windowed_processes()
}

/// Le reti connesse adesso, per scegliere quella della regola.
#[tauri::command]
pub async fn list_networks() -> Vec<String> {
    probes::connected_networks()
}

#[tauri::command]
pub fn add_rule(app: AppHandle, rule: NewRule) -> Result<SettingsDto, String> {
    let result = control::with_core(&app, |c| {
        let lang = c.lang;
        c.add_rule(rule.kind, rule.mode, rule.then)
            .map_err(|e| match e {
                RuleError::Invalid => t(lang, "settings.rule_invalid"),
                RuleError::TooMany => t(lang, "settings.rule_too_many"),
                RuleError::Duplicate => t(lang, "settings.rule_duplicate"),
                RuleError::Save(err) => tv(lang, "settings.save_error", &[("error", &err)]),
            })
    });
    // Senza aspettare 5 secondi: la regola nuova vale subito.
    wake_rules(&app);
    result.map(|()| settings_dto(&app))
}

#[tauri::command]
pub fn update_rule(
    app: AppHandle,
    id: u32,
    enabled: Option<bool>,
    mode: Option<Mode>,
    then: Option<ThenAct>,
) -> Result<SettingsDto, String> {
    let result = control::with_core(&app, |c| {
        let lang = c.lang;
        c.update_rule(id, enabled, mode, then)
            .map_err(|e| tv(lang, "settings.save_error", &[("error", &e.to_string())]))
    });
    wake_rules(&app);
    result.map(|()| settings_dto(&app))
}

#[tauri::command]
pub fn delete_rule(app: AppHandle, id: u32) -> Result<SettingsDto, String> {
    let result = control::with_core(&app, |c| {
        let lang = c.lang;
        c.delete_rule(id)
            .map_err(|e| tv(lang, "settings.save_error", &[("error", &e.to_string())]))
    });
    result.map(|()| settings_dto(&app))
}

/// `minutes: None` = fino al prossimo avvio di Moka.
#[tauri::command]
pub fn pause_rules(app: AppHandle, minutes: Option<u32>) -> SettingsDto {
    control::with_core(&app, |c| c.pause_rules(minutes, sys::now()));
    settings_dto(&app)
}

#[tauri::command]
pub fn resume_rules(app: AppHandle) -> SettingsDto {
    control::with_core(&app, |c| c.resume_rules(sys::now()));
    wake_rules(&app);
    settings_dto(&app)
}

/// Un giro delle regole adesso, su un thread a parte (le sonde non devono
/// bloccare il thread principale).
fn wake_rules(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || control::rules_tick(&app, &mut probes::Probes::default()));
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
        // Si cambia una copia e la si mette solo se il salvataggio riesce.
        let mut next = core.settings.clone();
        if let Some(text) = &patch.durations_text {
            next.durations = parse_durations_text(text)
                .map_err(|bad| tv(lang, "settings.durations_invalid", &[("value", &bad)]))?;
        }
        if let Some(language) = patch.language {
            next.language = language;
        }
        if let Some(left) = patch.left_click {
            next.left_click = left;
        }
        if let Some(mode) = patch.lid_mode {
            // Sceglierlo nelle Impostazioni vale come risposta alla domanda.
            next.lid_mode = mode;
            next.lid_asked = true;
        }
        if let Some(desk) = patch.desk_mode {
            next.desk_mode = desk;
            next.lid_asked = true;
        }
        if let Some(lock) = patch.lock_on_lid_open {
            next.lock_on_lid_open = lock;
        }
        if let Some(m) = patch
            .backpack_minutes
            .filter(|m| BACKPACK_CHOICES.contains(m))
        {
            next.backpack_minutes = m;
        }
        if let Some(b) = patch
            .battery_threshold
            .filter(|b| BATTERY_CHOICES.contains(b))
        {
            next.battery_threshold = b;
        }
        if let Some(w) = patch.warn_before_end {
            next.warn_before_end = w;
        }
        if let Some(s) = &patch.shortcut_toggle {
            next.shortcut_toggle = normalize_shortcut(s);
        }
        if let Some(s) = &patch.shortcut_screen_off {
            next.shortcut_screen_off = normalize_shortcut(s);
        }
        if let Some(p) = patch.presence {
            next.presence = p;
        }
        core.set_settings(next.dedup_shortcuts())
            .map_err(|e| tv(lang, "settings.save_error", &[("error", &e.to_string())]))
    });
    if result.is_ok() && (patch.shortcut_toggle.is_some() || patch.shortcut_screen_off.is_some()) {
        control::apply_shortcuts(&app);
    }
    result.map(|()| settings_dto(&app))
}
