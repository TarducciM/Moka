//! Aggiornamenti automatici firmati (tauri-plugin-updater).
//!
//! Controllo all'avvio (dopo un minuto, per non pesare sull'accesso a
//! Windows) e ogni 24 ore. Se c'è una versione nuova, il pannello e le
//! Impostazioni lo dicono; si installa solo quando l'utente lo chiede.
//!
//! Regola non negoziabile: **mai un riavvio per aggiornare durante una
//! sessione attiva**. La sessione cadrebbe e il PC andrebbe in sospensione a
//! metà di un download. E prima di installare, l'impostazione del coperchio
//! torna com'era: l'installer chiude l'app senza passare dalla sua uscita.

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::control;
use crate::i18n::t;
use crate::state::AppState;

/// L'aggiornamento trovato, pronto da installare.
#[derive(Default)]
pub struct Pending(pub Mutex<Option<Update>>);

const FIRST_CHECK: Duration = Duration::from_secs(60);
const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

pub fn spawn_checker(app: AppHandle) {
    std::thread::Builder::new()
        .name("moka-updates".into())
        .spawn(move || {
            std::thread::sleep(FIRST_CHECK);
            loop {
                if let Err(err) = tauri::async_runtime::block_on(check(&app)) {
                    // Nessun rumore per l'utente: il controllo riprova domani.
                    eprintln!("moka: controllo aggiornamenti non riuscito: {err}");
                }
                std::thread::sleep(EVERY);
            }
        })
        .ok();
}

/// Cerca una versione nuova. `Some(versione)` se c'è.
pub async fn check(app: &AppHandle) -> Result<Option<String>, String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let version = update.as_ref().map(|u| u.version.clone());
    *app.state::<Pending>().0.lock().unwrap() = update;
    let v = version.clone();
    control::with_core(app, move |c| c.update_version = v);
    Ok(version)
}

/// Scarica e installa. L'installer chiude Moka e la riapre aggiornata.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let lang = {
        let st = app.state::<AppState>();
        let core = st.core.lock().unwrap();
        if core.session.is_some() || core.countdown.is_some() {
            return Err(t(core.lang, "update.session_active"));
        }
        core.lang
    };
    let update = app
        .state::<Pending>()
        .0
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| t(lang, "update.none"))?;
    // Niente resta cambiato: l'installer chiuderà l'app senza passare da `quit`.
    app.state::<AppState>().core.lock().unwrap().shutdown();
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}
