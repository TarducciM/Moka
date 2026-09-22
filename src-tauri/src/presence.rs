//! Presenza: niente salvaschermo, niente blocco per inattività, niente
//! "Assente" su Teams o Slack. Le richieste di alimentazione non bastano
//! (trappola 12): serve che Windows veda dell'attività.
//!
//! Ogni ~50 secondi, solo se Moka sta tenendo sveglio il PC e l'utente non
//! tocca niente da almeno 50 secondi, Moka preme F15: un tasto che non esiste
//! sulle tastiere comuni e che quasi nessuna app usa. Spenta di default:
//! il blocco per inattività esiste per sicurezza, e sui PC di lavoro questa
//! opzione può violare le regole aziendali (lo dice l'interfaccia).

use std::time::Duration;

use tauri::{AppHandle, Manager};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetLastInputInfo, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    LASTINPUTINFO, VK_F15,
};

use crate::state::AppState;

pub const IDLE_MS: u64 = 50_000;
const EVERY: Duration = Duration::from_secs(10);

/// Da quanto l'utente non tocca tastiera e mouse.
pub fn idle_ms() -> u64 {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return 0;
    }
    // Contatori a 32 bit: la sottrazione con giro è quella giusta.
    u64::from(unsafe { GetTickCount() }.wrapping_sub(info.dwTime))
}

/// Premi e rilascia F15. `true` se Windows ha accettato entrambi gli eventi.
pub fn press_f15() -> bool {
    let key = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_F15,
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    let inputs = [key(Default::default()), key(KEYEVENTF_KEYUP)];
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) == 2 }
}

pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("moka-presence".into())
        .spawn(move || loop {
            std::thread::sleep(EVERY);
            let wanted = app
                .state::<AppState>()
                .core
                .lock()
                .unwrap()
                .presence_wanted();
            if wanted && idle_ms() >= IDLE_MS {
                press_f15();
            }
        })
        .ok();
}
