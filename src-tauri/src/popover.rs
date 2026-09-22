//! Il pannello che si apre dall'icona: posizione accanto alla tray, apertura e
//! chiusura, altezza adattata al contenuto.
//!
//! La finestra è dichiarata in `tauri.conf.json` e creata una volta sola
//! all'avvio (trappola 1: quelle create da codice a runtime restavano
//! bianche). Si nasconde, non si chiude mai (trappola 2).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindow};

pub const LABEL: &str = "popover";
pub const WIDTH: f64 = 340.0;

/// Il rettangolo dell'icona nella tray, in pixel fisici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Default)]
pub struct PopoverState {
    anchor: Option<Anchor>,
    /// Quando si è nascosto perché ha perso il focus. Un clic sull'icona
    /// mentre il pannello è aperto prima gli toglie il focus (e lo nasconde),
    /// poi arriva come clic: senza questo, il pannello si riaprirebbe subito.
    hidden_by_blur_at: Option<Instant>,
}

pub type Shared = Mutex<PopoverState>;

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

pub fn is_visible(app: &AppHandle) -> bool {
    window(app).is_some_and(|w| w.is_visible().unwrap_or(false))
}

pub fn toggle(app: &AppHandle, anchor: Option<Anchor>) {
    if is_visible(app) {
        hide(app);
        return;
    }
    let just_hidden = app
        .state::<Shared>()
        .lock()
        .unwrap()
        .hidden_by_blur_at
        .is_some_and(|at| at.elapsed() < Duration::from_millis(300));
    if !just_hidden {
        show(app, anchor);
    }
}

pub fn show(app: &AppHandle, anchor: Option<Anchor>) {
    let Some(win) = window(app) else { return };
    if anchor.is_some() {
        app.state::<Shared>().lock().unwrap().anchor = anchor;
    }
    place(app, &win);
    let _ = win.show();
    let _ = win.set_focus();
    let _ = app.emit_to(LABEL, "moka://popover-shown", ());
}

pub fn hide(app: &AppHandle) {
    if let Some(win) = window(app) {
        let _ = win.hide();
    }
}

pub fn on_blur(app: &AppHandle) {
    if is_visible(app) {
        app.state::<Shared>().lock().unwrap().hidden_by_blur_at = Some(Instant::now());
        hide(app);
    }
}

/// Adatta l'altezza al contenuto (misurato dalla pagina) e riposiziona.
pub fn fit(app: &AppHandle, content_height: f64) {
    let Some(win) = window(app) else { return };
    let height = content_height.clamp(120.0, 720.0).ceil();
    let _ = win.set_size(LogicalSize::new(WIDTH, height));
    place(app, &win);
}

/// Accanto all'icona, dentro l'area di lavoro del monitor (cioè fuori dalla
/// barra delle applicazioni, ovunque sia). Senza icona di riferimento, in
/// basso a destra del monitor principale.
fn place(app: &AppHandle, win: &WebviewWindow) {
    let anchor = app.state::<Shared>().lock().unwrap().anchor;
    let monitor = match anchor {
        Some(a) => app
            .monitor_from_point(f64::from(a.x), f64::from(a.y))
            .ok()
            .flatten(),
        None => None,
    }
    .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let Ok(size) = win.outer_size() else { return };

    let area = monitor.work_area();
    let margin = (12.0 * monitor.scale_factor()).round() as i32;
    let (left, top) = (area.position.x, area.position.y);
    let (right, bottom) = (left + area.size.width as i32, top + area.size.height as i32);
    let (w, h) = (size.width as i32, size.height as i32);

    let (mut x, mut y) = match anchor {
        Some(a) => {
            let cx = a.x + a.w as i32 / 2;
            let above = a.y - h - margin;
            // Barra in alto: sopra non c'è spazio, si apre sotto l'icona.
            let y = if above >= top {
                above
            } else {
                a.y + a.h as i32 + margin
            };
            (cx - w / 2, y)
        }
        None => (right - w - margin, bottom - h - margin),
    };
    x = x.clamp(left + margin, (right - w - margin).max(left + margin));
    y = y.clamp(top + margin, (bottom - h - margin).max(top + margin));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

/// Il rettangolo dell'icona, da quello che Tauri dà negli eventi della tray.
pub fn anchor_from_rect(rect: &tauri::Rect) -> Anchor {
    let pos = rect.position.to_physical::<i32>(1.0);
    let size = rect.size.to_physical::<u32>(1.0);
    Anchor {
        x: pos.x,
        y: pos.y,
        w: size.width,
        h: size.height,
    }
}
