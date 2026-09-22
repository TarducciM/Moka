//! La modifica dell'azione del coperchio, fatta in modo da non restare **mai**
//! cambiata (roadmap, "Come si cambia l'impostazione senza mai lasciarla
//! cambiata"). È l'unico punto in cui Moka tocca il piano energetico.
//!
//! Le regole, tutte qui:
//! - il registro delle modifiche (`lid-override.json`) si scrive e si forza su
//!   disco **prima** di toccare Windows, e si cancella solo a ripristino fatto;
//! - si ripristina nello schema **che era stato modificato**, non in quello
//!   attivo (l'utente può aver cambiato piano energetico nel frattempo);
//! - si ripristina **solo se il valore è ancora quello scritto da Moka**: se
//!   l'utente l'ha cambiato a mano, la sua scelta vince e il registro si scarta;
//! - finché c'è una modifica, `RunOnce` rimette tutto al prossimo accesso a
//!   Windows, anche se Moka non ripartisse.
//!
//! Le operazioni su Windows passano dal trait [`Backend`]: nei test c'è un
//! Windows finto, così ogni caso si prova senza toccare il PC.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use windows::core::GUID;

use crate::lid::{self, LidAction, DO_NOTHING};
use crate::lidplan::Force;
use crate::settings::write_json_atomic;

pub const FILE_NAME: &str = "lid-override.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub original: u32,
    pub written: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Lo schema modificato, come GUID in esadecimale.
    pub scheme: String,
    pub ac: Option<Change>,
    pub dc: Option<Change>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Index {
    Ac,
    Dc,
}

impl Entry {
    fn get(&self, i: Index) -> Option<Change> {
        match i {
            Index::Ac => self.ac,
            Index::Dc => self.dc,
        }
    }
    fn set(&mut self, i: Index, c: Option<Change>) {
        match i {
            Index::Ac => self.ac = c,
            Index::Dc => self.dc = c,
        }
    }
}

fn value_of(a: LidAction, i: Index) -> u32 {
    match i {
        Index::Ac => a.ac,
        Index::Dc => a.dc,
    }
}

fn wanted(f: Force, i: Index) -> bool {
    match i {
        Index::Ac => f.ac,
        Index::Dc => f.dc,
    }
}

pub fn scheme_key(g: &GUID) -> String {
    format!("{:032x}", g.to_u128())
}

fn scheme_from_key(k: &str) -> Option<GUID> {
    if k.len() != 32 {
        return None;
    }
    u128::from_str_radix(k, 16).ok().map(GUID::from_u128)
}

/// Ciò che serve di Windows. Vedi [`WindowsBackend`].
pub trait Backend {
    fn active_scheme(&self) -> Option<GUID>;
    fn read(&self, scheme: &GUID) -> Option<LidAction>;
    fn write(&self, scheme: &GUID, ac: Option<u32>, dc: Option<u32>) -> Result<(), String>;
    fn activate(&self, scheme: &GUID) -> Result<(), String>;
    fn set_restore_on_logon(&self, on: bool);
}

pub struct Overrides<B: Backend> {
    backend: B,
    path: PathBuf,
    entries: Vec<Entry>,
}

impl<B: Backend> Overrides<B> {
    /// Legge il registro lasciato da un'esecuzione precedente, come se fosse
    /// ostile: una modifica di cui non si sa il valore originale non si può
    /// rimettere, quindi si scarta invece di indovinare.
    pub fn load(backend: B, path: PathBuf) -> Self {
        let entries = fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.get("entries").cloned())
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|e| serde_json::from_value::<Entry>(e).ok())
            .filter_map(|mut e| {
                scheme_from_key(&e.scheme)?;
                let valid =
                    |c: Option<Change>| c.filter(|c| c.original <= 3 && c.written == DO_NOTHING);
                e.ac = valid(e.ac);
                e.dc = valid(e.dc);
                (e.ac.is_some() || e.dc.is_some()).then_some(e)
            })
            .collect();
        Overrides {
            backend,
            path,
            entries,
        }
    }

    pub fn is_active(&self) -> bool {
        !self.entries.is_empty()
    }

    /// L'impostazione di Windows com'era prima di Moka, per lo schema attivo:
    /// il valore originale dove Moka ha scritto, quello attuale altrove.
    pub fn original(&self) -> Option<LidAction> {
        let active = self.backend.active_scheme()?;
        let mut current = self.backend.read(&active)?;
        if let Some(e) = self
            .entries
            .iter()
            .find(|e| e.scheme == scheme_key(&active))
        {
            if let Some(c) = e.ac {
                current.ac = c.original;
            }
            if let Some(c) = e.dc {
                current.dc = c.original;
            }
        }
        Some(current)
    }

    /// Porta l'impostazione a `force`: forza "non fare nulla" dove serve,
    /// rimette com'era dove non serve più.
    pub fn apply(&mut self, force: Force) -> Result<(), String> {
        let active = self
            .backend
            .active_scheme()
            .ok_or("schema energetico non leggibile")?;
        let active_key = scheme_key(&active);
        let mut touched_active = false;

        // 1. Ciò che non serve più torna com'era, in tutti gli schemi toccati.
        for i in [Index::Ac, Index::Dc] {
            if wanted(force, i) {
                continue;
            }
            for e in &mut self.entries {
                if let Some(c) = e.get(i) {
                    if restore_one(&self.backend, &e.scheme, i, c) && e.scheme == active_key {
                        touched_active = true;
                    }
                    e.set(i, None);
                }
            }
        }
        self.entries.retain(|e| e.ac.is_some() || e.dc.is_some());
        self.persist()?;

        // 2. Ciò che serve si forza nello schema attivo, registro prima di tutto.
        if wanted(force, Index::Ac) || wanted(force, Index::Dc) {
            let current = self
                .backend
                .read(&active)
                .ok_or("azione del coperchio non leggibile")?;
            for i in [Index::Ac, Index::Dc] {
                if !wanted(force, i) {
                    continue;
                }
                let pos = match self.entries.iter().position(|e| e.scheme == active_key) {
                    Some(p) => p,
                    None => {
                        self.entries.push(Entry {
                            scheme: active_key.clone(),
                            ac: None,
                            dc: None,
                        });
                        self.entries.len() - 1
                    }
                };
                if self.entries[pos].get(i).is_some() {
                    continue;
                }
                let original = value_of(current, i);
                if original == DO_NOTHING {
                    // Windows fa già ciò che serve: niente da cambiare.
                    continue;
                }
                self.entries[pos].set(
                    i,
                    Some(Change {
                        original,
                        written: DO_NOTHING,
                    }),
                );
                self.persist()?;
                let (ac, dc) = match i {
                    Index::Ac => (Some(DO_NOTHING), None),
                    Index::Dc => (None, Some(DO_NOTHING)),
                };
                if let Err(err) = self.backend.write(&active, ac, dc) {
                    self.entries[pos].set(i, None);
                    self.entries.retain(|e| e.ac.is_some() || e.dc.is_some());
                    self.persist()?;
                    return Err(err);
                }
                touched_active = true;
            }
            self.entries.retain(|e| e.ac.is_some() || e.dc.is_some());
        }

        if touched_active {
            self.backend.activate(&active)?;
        }
        self.persist()
    }

    /// Rimette tutto com'era, in ogni schema toccato. Restituisce quante
    /// modifiche sono state davvero rimesse (quelle cambiate a mano
    /// dall'utente nel frattempo si lasciano come sono).
    pub fn restore_all(&mut self) -> usize {
        let active = self.backend.active_scheme().map(|g| scheme_key(&g));
        let mut restored = 0;
        let mut touched_active = false;
        for e in &self.entries {
            for i in [Index::Ac, Index::Dc] {
                if let Some(c) = e.get(i) {
                    if restore_one(&self.backend, &e.scheme, i, c) {
                        restored += 1;
                        if Some(&e.scheme) == active.as_ref() {
                            touched_active = true;
                        }
                    }
                }
            }
        }
        self.entries.clear();
        if touched_active {
            if let Some(g) = active.as_deref().and_then(scheme_from_key) {
                let _ = self.backend.activate(&g);
            }
        }
        let _ = self.persist();
        restored
    }

    /// Scrive il registro (o lo cancella se è vuoto) e tiene `RunOnce` allineato.
    fn persist(&self) -> Result<(), String> {
        if self.entries.is_empty() {
            let _ = fs::remove_file(&self.path);
            self.backend.set_restore_on_logon(false);
            return Ok(());
        }
        let value = serde_json::json!({ "version": 1, "entries": self.entries });
        write_json_atomic(&self.path, &value).map_err(|e| e.to_string())?;
        self.backend.set_restore_on_logon(true);
        Ok(())
    }
}

/// Rimette un valore solo se è ancora quello scritto da Moka. `true` se l'ha rimesso.
fn restore_one<B: Backend>(backend: &B, scheme: &str, i: Index, c: Change) -> bool {
    let Some(g) = scheme_from_key(scheme) else {
        return false;
    };
    let Some(current) = backend.read(&g) else {
        return false;
    };
    if value_of(current, i) != c.written {
        return false;
    }
    let (ac, dc) = match i {
        Index::Ac => (Some(c.original), None),
        Index::Dc => (None, Some(c.original)),
    };
    backend.write(&g, ac, dc).is_ok()
}

/// Il Windows vero.
pub struct WindowsBackend;

const RUN_ONCE: windows::core::PCWSTR =
    windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce");
const RUN_ONCE_VALUE: windows::core::PCWSTR = windows::core::w!("MokaRestoreLid");

impl Backend for WindowsBackend {
    fn active_scheme(&self) -> Option<GUID> {
        lid::active_scheme()
    }

    fn read(&self, scheme: &GUID) -> Option<LidAction> {
        lid::read(scheme)
    }

    fn write(&self, scheme: &GUID, ac: Option<u32>, dc: Option<u32>) -> Result<(), String> {
        lid::write_values(scheme, ac, dc).map_err(|e| format!("{e:?}"))
    }

    fn activate(&self, scheme: &GUID) -> Result<(), String> {
        lid::activate(scheme).map_err(|e| format!("{e:?}"))
    }

    /// `HKCU\…\RunOnce`: Windows esegue il comando una volta sola al prossimo
    /// accesso, anche se Moka non è impostata per avviarsi con Windows.
    fn set_restore_on_logon(&self, on: bool) {
        use windows::Win32::System::Registry::{
            RegCloseKey, RegCreateKeyExW, RegDeleteKeyValueW, RegSetValueExW, HKEY,
            HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
        };
        unsafe {
            if !on {
                let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_ONCE, RUN_ONCE_VALUE);
                return;
            }
            let Ok(exe) = std::env::current_exe() else {
                return;
            };
            let command = format!("\"{}\" --restore-lid", exe.display());
            let wide: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
            let mut key = HKEY::default();
            if RegCreateKeyExW(
                HKEY_CURRENT_USER,
                RUN_ONCE,
                None,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut key,
                None,
            )
            .is_ok()
            {
                let _ = RegSetValueExW(key, RUN_ONCE_VALUE, None, REG_SZ, Some(bytes));
                let _ = RegCloseKey(key);
            }
        }
    }
}

/// `moka --restore-lid`: rimette l'impostazione da un registro lasciato lì,
/// senza avviare l'app (lo usano `RunOnce` e il disinstallatore).
pub fn restore_from_disk(dir: PathBuf) -> usize {
    Overrides::load(WindowsBackend, dir.join(FILE_NAME)).restore_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;

    const A: u128 = 0x381b4222_f694_41f0_9685_ff5bb260df2e;
    const B: u128 = 0x8c5e7fda_e8bf_4a96_9a85_a6e23a8c635c;

    struct Fake {
        values: RefCell<HashMap<u128, LidAction>>,
        active: Cell<u128>,
        run_once: Cell<bool>,
        activations: Cell<u32>,
        fail_writes: Cell<bool>,
    }

    impl Fake {
        fn new() -> Fake {
            let mut values = HashMap::new();
            values.insert(A, LidAction { ac: 1, dc: 1 });
            values.insert(B, LidAction { ac: 2, dc: 1 });
            Fake {
                values: RefCell::new(values),
                active: Cell::new(A),
                run_once: Cell::new(false),
                activations: Cell::new(0),
                fail_writes: Cell::new(false),
            }
        }
        fn get(&self, s: u128) -> LidAction {
            self.values.borrow()[&s]
        }
        fn user_sets(&self, s: u128, a: LidAction) {
            self.values.borrow_mut().insert(s, a);
        }
    }

    impl Backend for &Fake {
        fn active_scheme(&self) -> Option<GUID> {
            Some(GUID::from_u128(self.active.get()))
        }
        fn read(&self, scheme: &GUID) -> Option<LidAction> {
            self.values.borrow().get(&scheme.to_u128()).copied()
        }
        fn write(&self, scheme: &GUID, ac: Option<u32>, dc: Option<u32>) -> Result<(), String> {
            if self.fail_writes.get() {
                return Err("accesso negato".into());
            }
            let mut v = self.values.borrow_mut();
            let e = v.get_mut(&scheme.to_u128()).ok_or("schema sconosciuto")?;
            if let Some(x) = ac {
                e.ac = x;
            }
            if let Some(x) = dc {
                e.dc = x;
            }
            Ok(())
        }
        fn activate(&self, _scheme: &GUID) -> Result<(), String> {
            self.activations.set(self.activations.get() + 1);
            Ok(())
        }
        fn set_restore_on_logon(&self, on: bool) {
            self.run_once.set(on);
        }
    }

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("moka-lid-{}-{name}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        dir.join(FILE_NAME)
    }

    const AC: Force = Force {
        ac: true,
        dc: false,
    };
    const BOTH: Force = Force { ac: true, dc: true };

    #[test]
    fn apply_and_release_leaves_windows_as_it_was() {
        let fake = Fake::new();
        let path = temp_path("roundtrip");
        let mut o = Overrides::load(&fake, path.clone());
        o.apply(AC).unwrap();
        assert_eq!(
            fake.get(A),
            LidAction { ac: 0, dc: 1 },
            "solo il valore in carica"
        );
        assert!(path.exists(), "registro scritto");
        assert!(
            fake.run_once.get(),
            "RunOnce attivo finché c'è una modifica"
        );
        assert_eq!(o.original(), Some(LidAction { ac: 1, dc: 1 }));

        o.apply(Force::NONE).unwrap();
        assert_eq!(fake.get(A), LidAction { ac: 1, dc: 1 });
        assert!(!path.exists(), "registro cancellato a ripristino fatto");
        assert!(!fake.run_once.get());
        assert!(!o.is_active());
    }

    #[test]
    fn a_value_changed_by_the_user_wins() {
        let fake = Fake::new();
        let mut o = Overrides::load(&fake, temp_path("user"));
        o.apply(BOTH).unwrap();
        // Durante la sessione l'utente mette "iberna" in carica.
        fake.user_sets(A, LidAction { ac: 2, dc: 0 });
        o.apply(Force::NONE).unwrap();
        assert_eq!(
            fake.get(A),
            LidAction { ac: 2, dc: 1 },
            "AC resta dell'utente, DC torna com'era"
        );
    }

    #[test]
    fn nothing_to_change_when_windows_already_does_nothing() {
        let fake = Fake::new();
        fake.user_sets(A, LidAction { ac: 0, dc: 0 });
        let path = temp_path("noop");
        let mut o = Overrides::load(&fake, path.clone());
        o.apply(BOTH).unwrap();
        assert!(!o.is_active());
        assert!(!path.exists());
        assert!(!fake.run_once.get());
        assert_eq!(fake.activations.get(), 0);
    }

    #[test]
    fn scheme_change_during_override() {
        let fake = Fake::new();
        let mut o = Overrides::load(&fake, temp_path("schemes"));
        o.apply(AC).unwrap();
        // L'utente passa a un altro piano energetico: si forza anche quello.
        fake.active.set(B);
        o.apply(AC).unwrap();
        assert_eq!(fake.get(A).ac, 0);
        assert_eq!(fake.get(B).ac, 0);
        o.apply(Force::NONE).unwrap();
        assert_eq!(
            fake.get(A),
            LidAction { ac: 1, dc: 1 },
            "lo schema modificato, non solo l'attivo"
        );
        assert_eq!(fake.get(B), LidAction { ac: 2, dc: 1 });
    }

    #[test]
    fn a_crash_is_repaired_from_the_log() {
        let fake = Fake::new();
        let path = temp_path("crash");
        {
            let mut o = Overrides::load(&fake, path.clone());
            o.apply(BOTH).unwrap();
            // Moka cade qui: niente ripristino.
        }
        assert_eq!(fake.get(A), LidAction { ac: 0, dc: 0 });
        let mut again = Overrides::load(&fake, path.clone());
        assert!(again.is_active());
        assert_eq!(again.restore_all(), 2);
        assert_eq!(fake.get(A), LidAction { ac: 1, dc: 1 });
        assert!(!path.exists());
        assert!(!fake.run_once.get());
    }

    #[test]
    fn partial_release() {
        let fake = Fake::new();
        let mut o = Overrides::load(&fake, temp_path("partial"));
        o.apply(BOTH).unwrap();
        o.apply(AC).unwrap();
        assert_eq!(fake.get(A), LidAction { ac: 0, dc: 1 });
        assert!(o.is_active());
    }

    #[test]
    fn failed_write_leaves_no_log_behind() {
        let fake = Fake::new();
        fake.fail_writes.set(true);
        let path = temp_path("fail");
        let mut o = Overrides::load(&fake, path.clone());
        assert!(o.apply(AC).is_err());
        assert!(!o.is_active());
        assert!(!path.exists());
        assert!(!fake.run_once.get());
        assert_eq!(fake.get(A), LidAction { ac: 1, dc: 1 });
    }

    #[test]
    fn hostile_log_is_trimmed_not_trusted() {
        let fake = Fake::new();
        let path = temp_path("hostile");
        fs::write(
            &path,
            r#"{"entries":[
                {"scheme":"not-a-guid","ac":{"original":1,"written":0}},
                {"scheme":"381b4222f69441f09685ff5bb260df2e","ac":{"original":9,"written":0},"dc":{"original":1,"written":0}},
                {"scheme":"381b4222f69441f09685ff5bb260df2e","ac":{"original":1,"written":3}},
                42
            ]}"#,
        )
        .unwrap();
        let o = Overrides::load(&fake, path.clone());
        // Resta solo la modifica di cui si conoscono origine e valore scritto.
        assert_eq!(o.entries.len(), 1);
        assert_eq!(o.entries[0].ac, None);
        assert_eq!(
            o.entries[0].dc,
            Some(Change {
                original: 1,
                written: 0
            })
        );
        fs::write(&path, "{ troncato").unwrap();
        assert!(!Overrides::load(&fake, path).is_active());
    }
}
