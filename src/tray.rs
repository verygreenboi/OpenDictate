//! The menu bar icon, so closing the window leaves the shortcuts running.
//!
//! Without this there is no way back to a hidden window, and quitting would
//! mean force-quitting. The app keeps its Dock icon as well; this is an extra
//! way in, not a replacement for it.

use eframe::egui;
use std::sync::mpsc::{Receiver, channel};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// What the menu bar icon is saying.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Recording,
    Thinking,
}

pub struct Tray {
    /// Held so the icon stays in the menu bar; dropping it removes the icon.
    icon: TrayIcon,
    pub show: MenuId,
    pub quit: MenuId,
}

impl Tray {
    /// Shows what the app is doing by lighting the icon rather than writing
    /// next to it. Text in the menu bar is noise, and macOS already puts its
    /// own orange dot up there whenever the microphone is live.
    ///
    /// Idle is a template image, so the system tints it for a light or dark
    /// menu bar. The busy states are coloured, which means they cannot be
    /// templates: a template image keeps only its alpha.
    pub fn set_state(&self, state: State) {
        let (color, glow, tip) = match state {
            State::Idle => ([255, 255, 255], false, "Open Dictate"),
            State::Recording => ([255, 69, 58], true, "Listening"),
            State::Thinking => ([255, 179, 64], false, "Thinking…"),
        };
        if let Some(i) = bars(color, glow) {
            let _ = if matches!(state, State::Idle) {
                self.icon.set_icon_templated(Some(i))
            } else {
                self.icon.set_icon(Some(i))
            };
        }
        let _ = self.icon.set_tooltip(Some(tip));
    }
}

/// Three bars, like the mark in the sidebar. Drawn here so the app needs no
/// asset file. `glow` adds a soft halo behind them, which is what makes
/// recording read as lit rather than merely a different colour.
fn bars(color: [u8; 3], glow: bool) -> Option<Icon> {
    const N: u32 = 32;
    const W: u32 = 4;
    // (left edge, height)
    let bars = [(6u32, 12u32), (14, 22), (22, 16)];
    let mut rgba = vec![0u8; (N * N * 4) as usize];
    if glow {
        let mid = N as f32 / 2.0;
        for y in 0..N {
            for x in 0..N {
                let dx = (x as f32 - mid) / mid;
                let dy = (y as f32 - mid) / mid;
                let fade = (1.0 - (dx * dx + dy * dy).sqrt()).max(0.0);
                let i = ((y * N + x) * 4) as usize;
                rgba[i..i + 3].copy_from_slice(&color);
                rgba[i + 3] = (fade.powf(2.2) * 90.0) as u8;
            }
        }
    }
    for (x0, h) in bars {
        for y in (N - h) / 2..(N + h) / 2 {
            for x in x0..x0 + W {
                let i = ((y * N + x) * 4) as usize;
                rgba[i..i + 3].copy_from_slice(&color);
                rgba[i + 3] = 255;
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
    if let Some(i) = bars([255, 255, 255], false) {
        #[cfg(target_os = "macos")]
        {
            builder = builder.with_icon_templated(i);
        }
        #[cfg(not(target_os = "macos"))]
        {
            builder = builder.with_icon(i);
        }
    }

    let tray = Tray { icon: builder.build().ok()?, show: ids.0, quit: ids.1 };
    Some((tray, rx))
}
