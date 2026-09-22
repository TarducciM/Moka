//! Le azioni sul sistema: bloccare, sospendere, ibernare, arrestare.
//!
//! ⚠️ Nessuna di queste si può provare in un test automatico (spegnerebbero la
//! macchina che lo esegue): sono verificate a mano, vedi `test.md`. E la
//! sospensione **da codice** su un PC con standby moderno è l'ipotesi 4 dello
//! spike: `SetSuspendState` potrebbe non bastare, per questo c'è un ripiego.

use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
    SE_SHUTDOWN_NAME, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Power::SetSuspendState;
use windows::Win32::System::Shutdown::{
    ExitWindowsEx, LockWorkStation, EWX_POWEROFF, EWX_SHUTDOWN, SHTDN_REASON_FLAG_PLANNED,
    SHTDN_REASON_MAJOR_OTHER,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use crate::lidplan::LidAct;

pub fn lock() {
    unsafe {
        let _ = LockWorkStation();
    }
}

/// Fa ciò che Windows avrebbe fatto chiudendo il coperchio.
pub fn perform(act: LidAct, modern_standby: bool) -> Result<(), String> {
    enable_shutdown_privilege();
    match act {
        LidAct::Sleep => {
            // Sui PC con standby moderno `SetSuspendState(FALSE)` potrebbe non
            // funzionare (da verificare nello spike). Il ripiego: schermo spento
            // senza più richieste attive, che su quei PC è l'ingresso naturale
            // nello standby.
            let ok = unsafe { SetSuspendState(false, false, false) };
            if !ok && modern_standby {
                crate::sys::screen_off(None);
                return Ok(());
            }
            ok.then_some(())
                .ok_or_else(|| "SetSuspendState(sospendi) rifiutata".into())
        }
        LidAct::Hibernate => unsafe { SetSuspendState(true, false, false) }
            .then_some(())
            .ok_or_else(|| "SetSuspendState(iberna) rifiutata".into()),
        LidAct::Shutdown => unsafe {
            ExitWindowsEx(
                EWX_SHUTDOWN | EWX_POWEROFF,
                SHTDN_REASON_MAJOR_OTHER | SHTDN_REASON_FLAG_PLANNED,
            )
        }
        .map_err(|e| e.to_string()),
    }
}

/// Sospendere e spegnere richiedono il privilegio `SeShutdownPrivilege`, che
/// ogni utente ha ma che va attivato sul token del processo.
fn enable_shutdown_privilege() {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
        .is_err()
        {
            return;
        }
        let mut luid = LUID::default();
        if LookupPrivilegeValueW(None, SE_SHUTDOWN_NAME, &mut luid).is_ok() {
            let tp = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES {
                    Luid: luid,
                    Attributes: SE_PRIVILEGE_ENABLED,
                }],
            };
            let _ = AdjustTokenPrivileges(token, false, Some(&tp), 0, None, None);
        }
        let _ = CloseHandle(token);
    }
}
