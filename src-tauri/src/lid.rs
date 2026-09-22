//! L'impostazione di Windows "Quando chiudo il coperchio"
//! (`GUID_LIDCLOSE_ACTION`: 0 non fare nulla, 1 sospendi, 2 iberna, 3 arresta).
//!
//! Qui ci sono solo le primitive: leggere, scrivere, controllare i permessi.
//! Il registro delle modifiche e le sette strade di ripristino arrivano con la
//! 0.2 (vedi `docs/ROADMAP.md`). Per ora le usa solo lo spike, che rimette
//! sempre il valore originale.
//!
//! ⚠️ Su alcuni portatili l'impostazione è **nascosta** (`ATTRIB_HIDE`): è il
//! caso di LPT-MIKI. `powercfg /q` non la mostra affatto, serve `powercfg /qh`.
//! Le API invece la leggono e la scrivono normalmente.

use serde::Serialize;
use windows::core::GUID;
use windows::Win32::Foundation::{LocalFree, HLOCAL, WIN32_ERROR};
use windows::Win32::System::Power::{
    PowerGetActiveScheme, PowerReadACValueIndex, PowerReadDCValueIndex, PowerSetActiveScheme,
    PowerSettingAccessCheck, PowerWriteACValueIndex, PowerWriteDCValueIndex,
    ACCESS_AC_POWER_SETTING_INDEX, ACCESS_DC_POWER_SETTING_INDEX,
};

pub const GUID_SYSTEM_BUTTON_SUBGROUP: GUID =
    GUID::from_u128(0x4f971e89_eebd_4455_a8de_9e59040e7347);
pub const GUID_LIDCLOSE_ACTION: GUID = GUID::from_u128(0x5ca83367_6e45_459f_a27b_476b1d01c936);

pub const DO_NOTHING: u32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LidAction {
    pub ac: u32,
    pub dc: u32,
}

pub fn action_label(value: u32) -> &'static str {
    match value {
        0 => "non fare nulla",
        1 => "sospendi",
        2 => "iberna",
        3 => "arresta",
        _ => "sconosciuto",
    }
}

/// Lo schema energetico attivo.
pub fn active_scheme() -> Option<GUID> {
    unsafe {
        let mut ptr: *mut GUID = std::ptr::null_mut();
        if PowerGetActiveScheme(None, &mut ptr).is_err() || ptr.is_null() {
            return None;
        }
        let guid = *ptr;
        let _ = LocalFree(Some(HLOCAL(ptr as *mut core::ffi::c_void)));
        Some(guid)
    }
}

pub fn read(scheme: &GUID) -> Option<LidAction> {
    let mut ac = 0u32;
    let mut dc = 0u32;
    unsafe {
        PowerReadACValueIndex(
            None,
            Some(scheme),
            Some(&GUID_SYSTEM_BUTTON_SUBGROUP),
            Some(&GUID_LIDCLOSE_ACTION),
            &mut ac,
        )
        .ok()
        .ok()?;
        // Le versioni "DC" restituiscono un u32 grezzo invece di WIN32_ERROR.
        WIN32_ERROR(PowerReadDCValueIndex(
            None,
            Some(scheme),
            Some(&GUID_SYSTEM_BUTTON_SUBGROUP),
            Some(&GUID_LIDCLOSE_ACTION),
            &mut dc,
        ))
        .ok()
        .ok()?;
    }
    Some(LidAction { ac, dc })
}

/// Scrive i valori indicati (`None` lascia com'è) e riattiva lo schema:
/// senza `PowerSetActiveScheme` la modifica non ha effetto subito (trappola 25).
/// Da usare solo sullo schema **attivo**: riattivarne un altro lo renderebbe attivo.
pub fn write(scheme: &GUID, ac: Option<u32>, dc: Option<u32>) -> Result<(), WIN32_ERROR> {
    write_values(scheme, ac, dc)?;
    activate(scheme)
}

/// Riapplica lo schema, così le modifiche hanno effetto subito.
pub fn activate(scheme: &GUID) -> Result<(), WIN32_ERROR> {
    to_result(unsafe { PowerSetActiveScheme(None, Some(scheme)) })
}

/// Scrive i valori senza riattivare lo schema (vale per qualunque schema).
pub fn write_values(scheme: &GUID, ac: Option<u32>, dc: Option<u32>) -> Result<(), WIN32_ERROR> {
    unsafe {
        if let Some(v) = ac {
            to_result(PowerWriteACValueIndex(
                None,
                scheme,
                Some(&GUID_SYSTEM_BUTTON_SUBGROUP),
                Some(&GUID_LIDCLOSE_ACTION),
                v,
            ))?;
        }
        if let Some(v) = dc {
            to_result(WIN32_ERROR(PowerWriteDCValueIndex(
                None,
                scheme,
                Some(&GUID_SYSTEM_BUTTON_SUBGROUP),
                Some(&GUID_LIDCLOSE_ACTION),
                v,
            )))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Access {
    /// `true` se nessun criterio di gruppo impone il valore in carica.
    pub ac: bool,
    pub dc: bool,
}

/// C'è un criterio di gruppo che impone l'azione del coperchio? (trappola 28)
pub fn access_check() -> Access {
    unsafe {
        Access {
            ac: PowerSettingAccessCheck(ACCESS_AC_POWER_SETTING_INDEX, Some(&GUID_LIDCLOSE_ACTION))
                .is_ok(),
            dc: PowerSettingAccessCheck(ACCESS_DC_POWER_SETTING_INDEX, Some(&GUID_LIDCLOSE_ACTION))
                .is_ok(),
        }
    }
}

fn to_result(e: WIN32_ERROR) -> Result<(), WIN32_ERROR> {
    if e.is_ok() {
        Ok(())
    } else {
        Err(e)
    }
}
