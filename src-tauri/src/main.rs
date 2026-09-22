// Prevents an extra console window on Windows in release. DO NOT REMOVE.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `--restore-lid` si gestisce qui, prima di Tauri: niente finestre, niente
    // single-instance (che al prossimo accesso potrebbe litigare con l'avvio
    // automatico), solo il ripristino e l'uscita.
    if std::env::args().any(|a| a == "--restore-lid") {
        moka_lib::restore_lid_and_exit();
    }
    moka_lib::run()
}
