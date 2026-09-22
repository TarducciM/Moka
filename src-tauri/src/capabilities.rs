//! Che PC è questo? Portatile, standby moderno, ibernazione, batteria.
//! Serve oggi allo spike (`examples/spike.rs`) e dalla 0.2 all'app, che mostra
//! la sezione del coperchio solo dove il coperchio esiste.

use serde::Serialize;
use windows::Win32::System::Power::{
    CallNtPowerInformation, GetPwrCapabilities, GetSystemPowerStatus, SystemExecutionState,
    SYSTEM_POWER_CAPABILITIES, SYSTEM_POWER_STATUS,
};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// C'è un coperchio: è un portatile (o un convertibile).
    pub lid_present: bool,
    /// Standby moderno (S0 Low Power Idle, "AoAc"): il caso difficile.
    pub modern_standby: bool,
    /// Sospensione classica S3.
    pub s3: bool,
    /// Ibernazione disponibile (S4 supportato e file di ibernazione presente).
    pub hibernate: bool,
    pub batteries: bool,
}

pub fn read() -> Capabilities {
    let mut c = SYSTEM_POWER_CAPABILITIES::default();
    let ok = unsafe { GetPwrCapabilities(&mut c) };
    if !ok {
        return Capabilities {
            lid_present: false,
            modern_standby: false,
            s3: false,
            hibernate: false,
            batteries: false,
        };
    }
    Capabilities {
        lid_present: c.LidPresent,
        modern_standby: c.AoAc,
        s3: c.SystemS3,
        hibernate: c.SystemS4 && c.HiberFilePresent,
        batteries: c.SystemBatteriesPresent,
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerStatus {
    /// `None` se Windows non lo sa.
    pub on_ac: Option<bool>,
    pub battery_percent: Option<u8>,
}

pub fn power_status() -> PowerStatus {
    let mut s = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut s) }.is_err() {
        return PowerStatus {
            on_ac: None,
            battery_percent: None,
        };
    }
    PowerStatus {
        on_ac: match s.ACLineStatus {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        battery_percent: (s.BatteryLifePercent <= 100).then_some(s.BatteryLifePercent),
    }
}

/// Lo stato di esecuzione complessivo del sistema (`ES_SYSTEM_REQUIRED` = 1,
/// `ES_DISPLAY_REQUIRED` = 2, …). Non richiede l'amministratore, a differenza
/// di `powercfg /requests`: lo spike lo usa per vedere se una richiesta si
/// riflette qui.
pub fn execution_state() -> Option<u32> {
    let mut state = 0u32;
    let status = unsafe {
        CallNtPowerInformation(
            SystemExecutionState,
            None,
            0,
            Some(&mut state as *mut _ as *mut core::ffi::c_void),
            std::mem::size_of::<u32>() as u32,
        )
    };
    status.is_ok().then_some(state)
}
