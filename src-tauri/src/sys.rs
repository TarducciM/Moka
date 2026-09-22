//! Piccoli contatti con Windows che non meritano un modulo a sé: orologi,
//! sessione di accesso, DPI, tema della barra, spegnimento dello schermo.

use std::time::{SystemTime, UNIX_EPOCH};

use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::Security::{
    GetTokenInformation, TokenStatistics, TOKEN_QUERY, TOKEN_STATISTICS,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER,
    KEY_NOTIFY, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET, RRF_RT_REG_DWORD,
};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::WindowsAndMessaging::{
    PostMessageW, HWND_BROADCAST, SC_MONITORPOWER, WM_SYSCOMMAND,
};

use crate::session::Now;

pub fn now() -> Now {
    let wall_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    Now {
        tick_ms: unsafe { GetTickCount64() },
        wall_ms,
    }
}

/// L'identificativo (LUID) della sessione di accesso a Windows di questo
/// processo. Cambia a ogni accesso, anche quando con l'avvio rapido il kernel
/// non si riavvia. `0` se non si riesce a leggerlo: in quel caso nessuna
/// sessione salvata coincide mai, e la ripresa semplicemente non avviene.
pub fn logon_id() -> u64 {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return 0;
        }
        let mut stats = TOKEN_STATISTICS::default();
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenStatistics,
            Some(&mut stats as *mut _ as *mut core::ffi::c_void),
            std::mem::size_of::<TOKEN_STATISTICS>() as u32,
            &mut len,
        );
        let _ = CloseHandle(token);
        if ok.is_err() {
            return 0;
        }
        let luid = stats.AuthenticationId;
        (u64::from(luid.HighPart as u32) << 32) | u64::from(luid.LowPart)
    }
}

pub fn system_dpi() -> u32 {
    match unsafe { GetDpiForSystem() } {
        0 => 96,
        dpi => dpi,
    }
}

const PERSONALIZE: windows::core::PCWSTR =
    w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");

/// La barra delle applicazioni è chiara? (`SystemUsesLightTheme`, che è
/// diverso da `AppsUseLightTheme`: si possono avere barra scura e app chiare.)
/// Se il valore manca, la barra è scura: è il predefinito di Windows.
pub fn taskbar_is_light() -> bool {
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PERSONALIZE,
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut _ as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    status.is_ok() && data != 0
}

/// Chiama `on_change` ogni volta che cambia qualcosa nella chiave del tema.
/// Un thread fermo su `RegNotifyChangeKeyValue`: nessun polling, costo zero.
pub fn watch_taskbar_theme(on_change: impl Fn() + Send + 'static) {
    std::thread::Builder::new()
        .name("moka-theme-watch".into())
        .spawn(move || unsafe {
            let mut key = HKEY::default();
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PERSONALIZE,
                None,
                KEY_READ | KEY_NOTIFY,
                &mut key,
            )
            .is_err()
            {
                return;
            }
            loop {
                let status =
                    RegNotifyChangeKeyValue(key, false, REG_NOTIFY_CHANGE_LAST_SET, None, false);
                if status.is_err() {
                    break;
                }
                on_change();
            }
            let _ = RegCloseKey(key);
        })
        .ok();
}

/// Spegne subito il monitor. Con `PostMessage`, mai `SendMessage` a tutte le
/// finestre: una finestra che non risponde lo bloccherebbe (trappola 14).
/// Con la nostra finestra basta lei (la gestisce `DefWindowProc`); senza, si
/// manda a tutte, sempre in modo asincrono.
pub fn screen_off(hwnd: Option<HWND>) {
    const MONITOR_OFF: isize = 2;
    unsafe {
        let _ = PostMessageW(
            Some(hwnd.unwrap_or(HWND_BROADCAST)),
            WM_SYSCOMMAND,
            WPARAM(SC_MONITORPOWER as usize),
            LPARAM(MONITOR_OFF),
        );
    }
}
