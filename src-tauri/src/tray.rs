//! Icona nella tray: tre stati, due varianti (barra chiara e barra scura) e sei
//! dimensioni, scelte al volo. Più il menu del clic destro.
//!
//! Le PNG si generano da `assets/tray-*.svg` con `npm run icons` e finiscono
//! dentro l'eseguibile: nessun file da cercare a runtime.

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Wry};

use crate::i18n::{duration_label, t, Lang};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconState {
    Off,
    System,
    Display,
}

/// Dimensioni disponibili, in pixel. 16 è il 100%, 24 il 150%, 32 il 200%.
pub const SIZES: [u32; 6] = [16, 20, 24, 32, 40, 48];

/// La dimensione più piccola che non vada ingrandita: rimpicciolire un'icona
/// la lascia nitida, ingrandirla la sfoca.
pub fn pick_size(dpi: u32) -> u32 {
    let target = (16 * dpi).div_ceil(96);
    SIZES.iter().copied().find(|&s| s >= target).unwrap_or(48)
}

macro_rules! png {
    ($state:literal, $bar:literal, $size:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/icons/tray/",
            $state,
            "-",
            $bar,
            "-",
            $size,
            ".png"
        ))
    };
}

macro_rules! sizes {
    ($state:literal, $bar:literal) => {
        [
            png!($state, $bar, "16"),
            png!($state, $bar, "20"),
            png!($state, $bar, "24"),
            png!($state, $bar, "32"),
            png!($state, $bar, "40"),
            png!($state, $bar, "48"),
        ]
    };
}

/// `[stato][barra: 0 scura, 1 chiara][dimensione]`
static ICONS: [[[&[u8]; 6]; 2]; 3] = [
    [sizes!("off", "dark"), sizes!("off", "light")],
    [sizes!("system", "dark"), sizes!("system", "light")],
    [sizes!("display", "dark"), sizes!("display", "light")],
];

pub fn icon_bytes(state: IconState, light_bar: bool, size: u32) -> &'static [u8] {
    let s = match state {
        IconState::Off => 0,
        IconState::System => 1,
        IconState::Display => 2,
    };
    let z = SIZES.iter().position(|&x| x == size).unwrap_or(0);
    ICONS[s][usize::from(light_bar)][z]
}

pub fn icon(state: IconState, light_bar: bool, size: u32) -> Image<'static> {
    Image::from_bytes(icon_bytes(state, light_bar, size)).expect("PNG generata da npm run icons")
}

/// Le voci del menu che cambiano con lo stato. Il menu si ricostruisce solo
/// quando cambiano lingua o durate; per il resto si aggiornano testi e spunte.
pub struct TrayMenu {
    pub menu: Menu<Wry>,
    status: MenuItem<Wry>,
    toggle: MenuItem<Wry>,
    screen: CheckMenuItem<Wry>,
}

pub struct MenuView<'a> {
    pub status: &'a str,
    pub active: bool,
    pub display: bool,
}

impl TrayMenu {
    pub fn build(app: &AppHandle, lang: Lang, durations: &[u32]) -> tauri::Result<TrayMenu> {
        let status = MenuItem::with_id(app, "status", "", false, None::<&str>)?;
        let toggle = MenuItem::with_id(app, "toggle", t(lang, "menu.start"), true, None::<&str>)?;

        let for_items: Vec<MenuItem<Wry>> = durations
            .iter()
            .map(|m| {
                MenuItem::with_id(
                    app,
                    format!("for:{m}"),
                    duration_label(lang, *m),
                    true,
                    None::<&str>,
                )
            })
            .collect::<tauri::Result<_>>()?;
        let forever = MenuItem::with_id(
            app,
            "for:never",
            t(lang, "menu.forever"),
            true,
            None::<&str>,
        )?;
        let until = MenuItem::with_id(app, "until", t(lang, "menu.until"), true, None::<&str>)?;
        let sep_a = PredefinedMenuItem::separator(app)?;
        let mut sub_items: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = Vec::new();
        for item in &for_items {
            sub_items.push(item);
        }
        sub_items.push(&forever);
        sub_items.push(&sep_a);
        sub_items.push(&until);
        let start_for = Submenu::with_items(app, t(lang, "menu.start_for"), true, &sub_items)?;

        let screen = CheckMenuItem::with_id(
            app,
            "screen",
            t(lang, "menu.also_screen"),
            true,
            false,
            None::<&str>,
        )?;
        let screen_off = MenuItem::with_id(
            app,
            "screen_off",
            t(lang, "menu.screen_off"),
            true,
            None::<&str>,
        )?;
        let open = MenuItem::with_id(app, "open", t(lang, "menu.open"), true, None::<&str>)?;
        let settings = MenuItem::with_id(
            app,
            "settings",
            t(lang, "menu.settings"),
            true,
            None::<&str>,
        )?;
        let quit = MenuItem::with_id(app, "quit", t(lang, "menu.quit"), true, None::<&str>)?;

        let menu = Menu::with_items(
            app,
            &[
                &status,
                &PredefinedMenuItem::separator(app)?,
                &toggle,
                &start_for,
                &screen,
                &screen_off,
                &PredefinedMenuItem::separator(app)?,
                &open,
                &settings,
                &PredefinedMenuItem::separator(app)?,
                &quit,
            ],
        )?;

        Ok(TrayMenu {
            menu,
            status,
            toggle,
            screen,
        })
    }

    pub fn update(&self, lang: Lang, view: &MenuView) {
        let _ = self.status.set_text(view.status);
        let _ = self.toggle.set_text(t(
            lang,
            if view.active {
                "menu.stop"
            } else {
                "menu.start"
            },
        ));
        let _ = self.screen.set_checked(view.display);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_follows_dpi() {
        assert_eq!(pick_size(96), 16);
        assert_eq!(pick_size(120), 20);
        assert_eq!(pick_size(144), 24);
        assert_eq!(pick_size(168), 32);
        assert_eq!(pick_size(192), 32);
        assert_eq!(pick_size(240), 40);
        assert_eq!(pick_size(288), 48);
        assert_eq!(pick_size(480), 48);
    }

    #[test]
    fn every_icon_is_a_png() {
        for state in [IconState::Off, IconState::System, IconState::Display] {
            for light in [false, true] {
                for size in SIZES {
                    let bytes = icon_bytes(state, light, size);
                    assert_eq!(
                        &bytes[..8],
                        b"\x89PNG\r\n\x1a\n",
                        "{state:?} {light} {size}"
                    );
                    let img = Image::from_bytes(bytes).unwrap();
                    assert_eq!((img.width(), img.height()), (size, size));
                }
            }
        }
    }
}
