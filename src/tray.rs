//! The menu bar icon, so closing the window leaves the shortcuts running.
//!
//! Without this there is no way back to a hidden window, and quitting would
//! mean force-quitting. The app keeps its Dock icon as well; this is an extra
//! way in, not a replacement for it.

use eframe::egui;
use std::sync::mpsc::{Receiver, channel};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub struct Tray {
    /// Held only so the icon stays in the menu bar; dropping it removes the icon.
    _icon: TrayIcon,
    pub show: MenuId,
    pub quit: MenuId,
}

/// Three bars, like the mark in the sidebar. Drawn here so the app needs no
/// asset file. On macOS this is a template image, so only the alpha matters and
/// the system tints it for the light or dark menu bar.
fn icon() -> Option<Icon> {
    const N: u32 = 32;
    const W: u32 = 4;
    // (left edge, height)
    let bars = [(6u32, 12u32), (14, 22), (22, 16)];
    let mut rgba = vec![0u8; (N * N * 4) as usize];
    for (x0, h) in bars {
        for y in (N - h) / 2..(N + h) / 2 {
            for x in x0..x0 + W {
                let i = ((y * N + x) * 4) as usize;
                rgba[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
    }
    Icon::from_rgba(rgba, N, N).ok()
}

/// Builds the menu bar icon, returning it with the channel its clicks arrive on.
///
/// The handler asks for a repaint because a hidden window draws no frames, and
/// a menu event that arrives between frames would otherwise sit unread.
pub fn build(ctx: &egui::Context) -> Option<(Tray, Receiver<MenuEvent>)> {
    let show = MenuItem::new("Show Open Dictate", true, None);
    let quit = MenuItem::new("Quit Open Dictate", true, None);
    let ids = (show.id().clone(), quit.id().clone());

    let menu = Menu::new();
    menu.append(&show).ok()?;
    menu.append(&PredefinedMenuItem::separator()).ok()?;
    menu.append(&quit).ok()?;

    let (tx, rx) = channel();
    let ctx = ctx.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let _ = tx.send(e);
        ctx.request_repaint();
    }));

    let mut builder = TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("Open Dictate");
    if let Some(i) = icon() {
        #[cfg(target_os = "macos")]
        {
            builder = builder.with_icon_templated(i);
        }
        #[cfg(not(target_os = "macos"))]
        {
            builder = builder.with_icon(i);
        }
    }

    let tray = Tray { _icon: builder.build().ok()?, show: ids.0, quit: ids.1 };
    Some((tray, rx))
}
