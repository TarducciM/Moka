//! Le notifiche di sistema: coperchio, alimentazione, batteria, piano
//! energetico, monitor, sospensione e fine della sessione di Windows.
//!
//! Arrivano a una finestra top-level **nascosta** (`WS_EX_TOOLWINDOW`, mai
//! mostrata) su un thread tutto suo. Non una finestra solo-messaggi
//! (`HWND_MESSAGE`): quella non riceve i messaggi broadcast, cioè proprio
//! `WM_DISPLAYCHANGE` (trappola 23).

use std::sync::OnceLock;

use windows::core::{w, GUID};
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::{RegisterPowerSettingNotification, POWERBROADCAST_SETTING};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
    DEVICE_NOTIFY_WINDOW_HANDLE, MSG, PBT_APMRESUMEAUTOMATIC, PBT_APMSUSPEND,
    PBT_POWERSETTINGCHANGE, WINDOW_STYLE, WM_DISPLAYCHANGE, WM_ENDSESSION, WM_POWERBROADCAST,
    WM_QUERYENDSESSION, WNDCLASSW, WS_EX_TOOLWINDOW,
};

pub const GUID_LIDSWITCH_STATE_CHANGE: GUID =
    GUID::from_u128(0xba3e0f4d_b817_4094_a2d1_d56379e6a0f3);
pub const GUID_ACDC_POWER_SOURCE: GUID = GUID::from_u128(0x5d3e9a59_e9d5_4b00_a6bd_ff34ff516548);
pub const GUID_BATTERY_PERCENTAGE_REMAINING: GUID =
    GUID::from_u128(0xa7ad8041_b45a_4cae_87a3_eecbb468a9e1);
pub const GUID_ACTIVE_POWERSCHEME: GUID = GUID::from_u128(0x31f9f286_5084_42fe_b720_2b0264993763);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SysEvent {
    /// Arriva anche subito dopo la registrazione, con lo stato attuale. Su un
    /// fisso non arriva mai (trappola 24).
    Lid {
        closed: bool,
    },
    Power {
        on_ac: bool,
    },
    Battery {
        percent: u8,
    },
    /// Cambiato il piano energetico attivo.
    Scheme,
    /// Collegato o scollegato un monitor, cambiata una risoluzione.
    Displays,
    Suspending,
    Resumed,
    /// Windows si sta spegnendo o l'utente si disconnette: c'è poco tempo, e
    /// quello che va rimesso a posto va rimesso **adesso**, prima di tornare.
    EndSession,
}

type Handler = Box<dyn Fn(SysEvent) + Send + Sync>;
static HANDLER: OnceLock<Handler> = OnceLock::new();

/// Avvia il thread degli eventi. Da chiamare una volta sola.
pub fn spawn(on_event: impl Fn(SysEvent) + Send + Sync + 'static) {
    if HANDLER.set(Box::new(on_event)).is_err() {
        return;
    }
    std::thread::Builder::new()
        .name("moka-sysevents".into())
        .spawn(|| unsafe { run() })
        .ok();
}

unsafe fn run() {
    let Ok(module) = GetModuleHandleW(None) else {
        return;
    };
    let class = w!("MokaSysEvents");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: module.into(),
        lpszClassName: class,
        ..Default::default()
    };
    RegisterClassW(&wc);
    let Ok(hwnd) = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        class,
        w!("Moka"),
        WINDOW_STYLE(0),
        0,
        0,
        0,
        0,
        None,
        None,
        Some(module.into()),
        None,
    ) else {
        return;
    };
    for guid in [
        GUID_LIDSWITCH_STATE_CHANGE,
        GUID_ACDC_POWER_SOURCE,
        GUID_BATTERY_PERCENTAGE_REMAINING,
        GUID_ACTIVE_POWERSCHEME,
    ] {
        let _ =
            RegisterPowerSettingNotification(HANDLE(hwnd.0), &guid, DEVICE_NOTIFY_WINDOW_HANDLE);
    }
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        DispatchMessageW(&msg);
    }
}

fn emit(e: SysEvent) {
    if let Some(h) = HANDLER.get() {
        h(e);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_POWERBROADCAST => {
            match wparam.0 as u32 {
                PBT_POWERSETTINGCHANGE if lparam.0 != 0 => {
                    let setting = &*(lparam.0 as *const POWERBROADCAST_SETTING);
                    let data = if setting.DataLength >= 4 {
                        std::ptr::read_unaligned(setting.Data.as_ptr() as *const u32)
                    } else {
                        0
                    };
                    match setting.PowerSetting {
                        g if g == GUID_LIDSWITCH_STATE_CHANGE => {
                            emit(SysEvent::Lid { closed: data == 0 })
                        }
                        // 0 = in carica, 1 = batteria, 2 = UPS (a breve termine).
                        g if g == GUID_ACDC_POWER_SOURCE => {
                            emit(SysEvent::Power { on_ac: data == 0 })
                        }
                        g if g == GUID_BATTERY_PERCENTAGE_REMAINING => emit(SysEvent::Battery {
                            percent: data.min(100) as u8,
                        }),
                        g if g == GUID_ACTIVE_POWERSCHEME => emit(SysEvent::Scheme),
                        _ => {}
                    }
                }
                PBT_APMSUSPEND => emit(SysEvent::Suspending),
                PBT_APMRESUMEAUTOMATIC => emit(SysEvent::Resumed),
                _ => {}
            }
            LRESULT(1)
        }
        WM_DISPLAYCHANGE => {
            emit(SysEvent::Displays);
            LRESULT(0)
        }
        WM_QUERYENDSESSION => LRESULT(1),
        WM_ENDSESSION => {
            if wparam.0 != 0 {
                emit(SysEvent::EndSession);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
