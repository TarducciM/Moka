//! Richieste di alimentazione: il cuore di Moka.
//!
//! Si usa `PowerCreateRequest` e non `SetThreadExecutionState` (trappola 11):
//! quest'ultima vale per il thread che la chiama, e sul pool di thread di Tauri
//! si perderebbe in silenzio. Una richiesta invece è un handle: se il processo
//! muore, anche in crash, il sistema la rilascia da sé e non resta niente di
//! appeso.
//!
//! Il motivo (`REASON_CONTEXT`) è una frase leggibile: chi apre
//! `powercfg /requests` vede "Moka: sessione fino alle 18:30", non un processo
//! anonimo. Il testo si fissa alla creazione, quindi cambiarlo vuol dire creare
//! una richiesta nuova; la nuova si attiva **prima** di spegnere la vecchia,
//! così non c'è nemmeno un istante scoperto.

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Power::{
    PowerClearRequest, PowerCreateRequest, PowerRequestDisplayRequired,
    PowerRequestExecutionRequired, PowerRequestSystemRequired, PowerSetRequest, POWER_REQUEST_TYPE,
};
use windows::Win32::System::Threading::{
    POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0,
};

/// `POWER_REQUEST_CONTEXT_VERSION` in `minwinbase.h`; il crate `windows` non lo esporta.
const POWER_REQUEST_CONTEXT_VERSION: u32 = 0;

/// Quali richieste tenere attive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Needs {
    /// Niente sospensione per inattività.
    pub system: bool,
    /// Niente spegnimento dello schermo per inattività.
    pub display: bool,
    /// Il processo non viene sospeso (standby moderno). Per ora serve solo allo
    /// spike: se lo spike dimostra che serve anche nell'app, si accende qui.
    pub execution: bool,
}

impl Needs {
    pub const NONE: Needs = Needs {
        system: false,
        display: false,
        execution: false,
    };

    pub fn any(self) -> bool {
        self.system || self.display || self.execution
    }

    fn kinds(self) -> impl Iterator<Item = (bool, POWER_REQUEST_TYPE)> {
        [
            (self.system, PowerRequestSystemRequired),
            (self.display, PowerRequestDisplayRequired),
            (self.execution, PowerRequestExecutionRequired),
        ]
        .into_iter()
    }
}

struct Active {
    handle: HANDLE,
    reason: String,
    needs: Needs,
}

/// Una richiesta di alimentazione con il suo motivo. Si porta sempre allo stato
/// voluto con [`PowerRequest::apply`]; `Drop` rilascia tutto.
#[derive(Default)]
pub struct PowerRequest {
    active: Option<Active>,
}

// L'handle è un riferimento del kernel, non memoria del processo: usarlo da un
// altro thread è lecito. L'accesso passa sempre da un Mutex dello stato dell'app.
unsafe impl Send for PowerRequest {}

impl PowerRequest {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ciò che è attivo adesso (utile a test e diagnostica).
    pub fn current(&self) -> (Needs, Option<&str>) {
        match &self.active {
            Some(a) => (a.needs, Some(a.reason.as_str())),
            None => (Needs::NONE, None),
        }
    }

    /// Porta le richieste a `needs`, con `reason` come motivo leggibile.
    pub fn apply(&mut self, needs: Needs, reason: &str) -> windows::core::Result<()> {
        if !needs.any() {
            self.release();
            return Ok(());
        }

        let same_reason = self.active.as_ref().is_some_and(|a| a.reason == reason);
        if same_reason {
            let active = self.active.as_mut().expect("controllato sopra");
            // Prima si accende ciò che serve, poi si spegne ciò che non serve più.
            for ((want, kind), (had, _)) in needs.kinds().zip(active.needs.kinds()) {
                if want && !had {
                    unsafe { PowerSetRequest(active.handle, kind)? };
                }
            }
            for ((want, kind), (had, _)) in needs.kinds().zip(active.needs.kinds()) {
                if had && !want {
                    unsafe { PowerClearRequest(active.handle, kind)? };
                }
            }
            active.needs = needs;
            return Ok(());
        }

        // Motivo diverso: richiesta nuova, attivata prima di spegnere la vecchia.
        let handle = create_request(reason)?;
        for (want, kind) in needs.kinds() {
            if want {
                if let Err(err) = unsafe { PowerSetRequest(handle, kind) } {
                    unsafe {
                        let _ = CloseHandle(handle);
                    }
                    return Err(err);
                }
            }
        }
        self.release();
        self.active = Some(Active {
            handle,
            reason: reason.to_owned(),
            needs,
        });
        Ok(())
    }

    /// Spegne tutto e chiude l'handle.
    pub fn release(&mut self) {
        if let Some(active) = self.active.take() {
            for (had, kind) in active.needs.kinds() {
                if had {
                    unsafe {
                        let _ = PowerClearRequest(active.handle, kind);
                    }
                }
            }
            unsafe {
                let _ = CloseHandle(active.handle);
            }
        }
    }
}

impl Drop for PowerRequest {
    fn drop(&mut self) {
        self.release();
    }
}

fn create_request(reason: &str) -> windows::core::Result<HANDLE> {
    // Il buffer deve restare vivo per tutta la chiamata: Windows copia la stringa
    // dentro la richiesta, quindi dopo si può liberare.
    let mut wide: Vec<u16> = reason.encode_utf16().chain(std::iter::once(0)).collect();
    let context = REASON_CONTEXT {
        Version: POWER_REQUEST_CONTEXT_VERSION,
        Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
        Reason: REASON_CONTEXT_0 {
            SimpleReasonString: PWSTR(wide.as_mut_ptr()),
        },
    };
    unsafe { PowerCreateRequest(&context) }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Questi test chiamano davvero Windows: una richiesta creata e rilasciata da
    // un test non lascia niente dietro di sé (è un handle del processo di test).

    #[test]
    fn apply_and_release_roundtrip() {
        let mut req = PowerRequest::new();
        let both = Needs {
            system: true,
            display: true,
            execution: false,
        };
        req.apply(both, "Moka: test").unwrap();
        assert_eq!(req.current(), (both, Some("Moka: test")));

        let only_system = Needs {
            system: true,
            ..Needs::NONE
        };
        req.apply(only_system, "Moka: test").unwrap();
        assert_eq!(req.current().0, only_system);

        req.apply(only_system, "Moka: altro motivo").unwrap();
        assert_eq!(req.current(), (only_system, Some("Moka: altro motivo")));

        req.apply(Needs::NONE, "ignorato").unwrap();
        assert_eq!(req.current(), (Needs::NONE, None));
    }
}
