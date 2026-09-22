//! Le sonde delle regole: cosa succede sul PC adesso. Solo lettura, e solo
//! ciò che serve alle regole attive (vedi `rules::needs`): nessuna regola sul
//! download, nessuna lettura dei contatori di rete.

use std::collections::{HashMap, HashSet};

use windows::core::{w, BOOL, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, FILETIME, HWND, LPARAM};
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2, IF_TYPE_SOFTWARE_LOOPBACK, MIB_IF_TABLE2,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    RRF_RT_REG_QWORD,
};
use windows::Win32::System::Threading::GetSystemTimes;
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindow, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible,
    GW_OWNER,
};

use crate::rules::Needs;

/// Ciò che le sonde hanno visto (la parte che non arriva dagli eventi di
/// sistema: alimentazione e monitor li conosce già `sysevents`).
#[derive(Debug, Default)]
pub struct Seen {
    pub processes: Option<HashSet<String>>,
    pub pids: Option<HashSet<u32>>,
    pub fullscreen: bool,
    pub call: bool,
    pub net_kbps: Option<u32>,
    pub cpu_percent: Option<u32>,
}

/// Le sonde con memoria: velocità e carico sono differenze fra due letture.
#[derive(Default)]
pub struct Probes {
    net_prev: HashMap<u32, u64>,
    net_prev_tick: u64,
    cpu_prev: Option<(u64, u64)>,
}

impl Probes {
    pub fn observe(&mut self, needs: Needs, tick_ms: u64) -> Seen {
        let mut seen = Seen::default();
        if needs.processes {
            let (names, pids) = processes();
            seen.processes = Some(names);
            seen.pids = Some(pids);
        }
        if needs.fullscreen {
            seen.fullscreen = fullscreen();
        }
        if needs.call {
            seen.call = in_call();
        }
        if needs.net {
            seen.net_kbps = self.net_kbps(tick_ms);
        } else {
            self.net_prev.clear();
        }
        if needs.cpu {
            seen.cpu_percent = self.cpu_percent();
        } else {
            self.cpu_prev = None;
        }
        seen
    }

    /// Velocità di download in KB/s. Windows elenca la stessa scheda di rete
    /// più volte (interfacce filtro): sommarle conterebbe lo stesso download
    /// due volte, quindi vale la più veloce.
    fn net_kbps(&mut self, tick_ms: u64) -> Option<u32> {
        let now = net_bytes();
        let dt = tick_ms.saturating_sub(self.net_prev_tick);
        let rate = if self.net_prev.is_empty() || dt == 0 {
            None
        } else {
            now.iter()
                .filter_map(|(i, b)| self.net_prev.get(i).map(|p| b.saturating_sub(*p)))
                .max()
                .map(|bytes| (bytes * 1000 / dt / 1024) as u32)
        };
        self.net_prev = now;
        self.net_prev_tick = tick_ms;
        rate
    }

    fn cpu_percent(&mut self) -> Option<u32> {
        let (idle, total) = cpu_times()?;
        let out = self.cpu_prev.and_then(|(pi, pt)| {
            let dt = total.saturating_sub(pt);
            (dt > 0).then(|| (100 - (idle.saturating_sub(pi) * 100 / dt).min(100)) as u32)
        });
        self.cpu_prev = Some((idle, total));
        out
    }
}

/// Nomi (minuscoli) e pid dei processi in esecuzione.
pub fn processes() -> (HashSet<String>, HashSet<u32>) {
    let mut names = HashSet::new();
    let mut pids = HashSet::new();
    for (pid, name) in process_list() {
        names.insert(name);
        pids.insert(pid);
    }
    (names, pids)
}

fn process_list() -> Vec<(u32, String)> {
    let mut out = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return out;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
                out.push((entry.th32ProcessID, name));
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    out
}

/// Processi di Windows che hanno una finestra ma non sono "programmi" che
/// qualcuno sceglierebbe per una regola (ospitano altre app o la shell).
const NOT_PROGRAMS: [&str; 7] = [
    "moka.exe",
    "applicationframehost.exe",
    "shellexperiencehost.exe",
    "startmenuexperiencehost.exe",
    "searchhost.exe",
    "textinputhost.exe",
    "lockapp.exe",
];

/// I programmi che hanno una finestra visibile: sono quelli fra cui ha senso
/// scegliere per una regola (i servizi di sistema no).
pub fn windowed_processes() -> Vec<String> {
    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let pids = &mut *(lparam.0 as *mut HashSet<u32>);
        let visible = IsWindowVisible(hwnd).as_bool();
        let owned = GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.is_invalid());
        if visible && !owned && GetWindowTextLengthW(hwnd) > 0 {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            pids.insert(pid);
        }
        BOOL(1)
    }
    let mut pids: HashSet<u32> = HashSet::new();
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut pids as *mut _ as isize));
    }
    let mut names: Vec<String> = process_list()
        .into_iter()
        .filter(|(pid, name)| pids.contains(pid) && !NOT_PROGRAMS.contains(&name.as_str()))
        .map(|(_, name)| name)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Un'app a schermo intero (video, gioco, presentazione) è in primo piano.
pub fn fullscreen() -> bool {
    matches!(
        unsafe { SHQueryUserNotificationState() },
        Ok(s) if s == QUNS_BUSY || s == QUNS_RUNNING_D3D_FULL_SCREEN || s == QUNS_PRESENTATION_MODE
    )
}

/// Microfono o webcam in uso adesso. Windows annota nel registro, per ogni
/// app, quando ha cominciato e smesso di usarli: `LastUsedTimeStop = 0` con
/// un inizio registrato vuol dire "in uso ora". Da verificare con Teams, Zoom
/// e Meet nel browser (roadmap).
pub fn in_call() -> bool {
    [
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone"),
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\webcam"),
    ]
    .into_iter()
    .any(|path| unsafe { in_use_under(path) })
}

unsafe fn in_use_under(path: PCWSTR) -> bool {
    let mut key = HKEY::default();
    if RegOpenKeyExW(HKEY_CURRENT_USER, path, None, KEY_READ, &mut key).is_err() {
        return false;
    }
    let found = scan(key, 0);
    let _ = RegCloseKey(key);
    found
}

/// App impacchettate direttamente sotto la chiave; le altre sotto
/// `NonPackaged`, un livello più giù.
unsafe fn scan(key: HKEY, depth: u32) -> bool {
    let mut index = 0;
    loop {
        let mut name = [0u16; 512];
        let mut len = name.len() as u32;
        let status = RegEnumKeyExW(
            key,
            index,
            Some(PWSTR(name.as_mut_ptr())),
            &mut len,
            None,
            None,
            None,
            None,
        );
        if status.is_err() {
            return false;
        }
        index += 1;
        let sub = PCWSTR(name.as_ptr());
        if in_use(key, sub) {
            return true;
        }
        if depth == 0 && String::from_utf16_lossy(&name[..len as usize]) == "NonPackaged" {
            let mut child = HKEY::default();
            if RegOpenKeyExW(key, sub, None, KEY_READ, &mut child).is_ok() {
                let found = scan(child, 1);
                let _ = RegCloseKey(child);
                if found {
                    return true;
                }
            }
        }
    }
}

unsafe fn in_use(key: HKEY, sub: PCWSTR) -> bool {
    let read = |value: PCWSTR| -> Option<u64> {
        let mut data = 0u64;
        let mut size = std::mem::size_of::<u64>() as u32;
        RegGetValueW(
            key,
            sub,
            value,
            RRF_RT_REG_QWORD,
            None,
            Some(&mut data as *mut _ as *mut core::ffi::c_void),
            Some(&mut size),
        )
        .is_ok()
        .then_some(data)
    };
    matches!(
        (read(w!("LastUsedTimeStart")), read(w!("LastUsedTimeStop"))),
        (Some(start), Some(0)) if start > 0
    )
}

/// Byte ricevuti da ogni interfaccia di rete attiva (loopback esclusa).
fn net_bytes() -> HashMap<u32, u64> {
    let mut out = HashMap::new();
    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table).is_err() || table.is_null() {
            return out;
        }
        let t = &*table;
        let rows = std::slice::from_raw_parts(t.Table.as_ptr(), t.NumEntries as usize);
        for row in rows {
            if row.Type != IF_TYPE_SOFTWARE_LOOPBACK && row.OperStatus == IfOperStatusUp {
                out.insert(row.InterfaceIndex, row.InOctets);
            }
        }
        FreeMibTable(table as *const core::ffi::c_void);
    }
    out
}

fn cpu_times() -> Option<(u64, u64)> {
    let ft = |f: FILETIME| (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime);
    let (mut idle, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
    // Il tempo "kernel" comprende già quello inattivo.
    Some((ft(idle), ft(kernel) + ft(user)))
}
