//! The two things the window needs that egui has no builder for: a blurred
//! backdrop, and a Dock tile that does not depend on Launch Services having
//! noticed the bundle's icon.

use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSAutoresizingMaskOptions, NSBox, NSBoxType, NSColor, NSImage, NSTitlePosition,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindowOrderingMode,
};
use objc2_foundation::NSData;
use std::cell::RefCell;

thread_local! {
    /// The wash of paper over the blur. Kept so a theme change can retint it
    /// without rebuilding the backdrop.
    static TINT: RefCell<Option<Retained<NSBox>>> = const { RefCell::new(None) };
}

/// Sets the Dock tile from the icon we ship.
///
/// The bundle already names it in CFBundleIconFile, but Launch Services keys
/// its icon cache to the bundle path, so replacing an app in place can leave
/// the Dock showing whatever was there first - the generic tile, for an app
/// that once had no icon. Setting it on the running NSApplication skips that
/// cache entirely.
pub fn set_dock_icon(png: &[u8]) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let data = NSData::with_bytes(png);
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else { return };
    unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
}

fn ns_color(rgba: [f64; 4]) -> Retained<NSColor> {
    unsafe { NSColor::colorWithSRGBRed_green_blue_alpha(rgba[0], rgba[1], rgba[2], rgba[3]) }
}

/// Puts a blurred backdrop behind the whole window, titlebar included.
///
/// Transparency alone shows what is behind the window sharply, which is
/// distracting under text. This is the system's own blur, the same one Finder
/// and Notes sit on.
///
/// It goes into the window's frame view rather than its content view, for two
/// reasons. The content view is the Metal view egui draws into - not a
/// container holding one - so anything added to it covers the drawing instead
/// of sitting under it. And the content view stops below the titlebar, which
/// is why the bar had nothing behind it at all once the window went
/// transparent.
///
/// Returns false until the window exists, so the caller keeps asking.
pub fn blur_window(min_width: f64, tint: [f64; 4]) -> bool {
    let Some(mtm) = MainThreadMarker::new() else { return false };
    let app = NSApplication::sharedApplication(mtm);
    for window in app.windows().iter() {
        // The root viewport is a 1x1 anchor with the same title, so the real
        // window is found by being the only one of any size.
        if window.frame().size.width < min_width {
            continue;
        }
        let Some(content) = window.contentView() else { continue };
        let Some(frame_view) = (unsafe { content.superview() }) else { continue };
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        // Otherwise the titlebar paints its own material over the blur and
        // the top of the window is a different surface from the rest.
        window.setTitlebarAppearsTransparent(true);

        let bounds = frame_view.bounds();
        let fill =
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable;

        let fx = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), bounds);
        // UnderWindowBackground is the material meant for exactly this: a
        // whole window sitting on the desktop, not a sidebar or a popover.
        // Measured against the same pixels with and without the window:
        // HUDWindow passes about 40% more of the backdrop than
        // UnderWindowBackground, which is the most of any material here.
        fx.setMaterial(NSVisualEffectMaterial::HUDWindow);
        fx.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        // Without Active the blur stops when the window loses focus.
        fx.setState(NSVisualEffectState::Active);
        fx.setAutoresizingMask(fill);

        // The paper wash, over the blur and under everything egui draws, so
        // the titlebar and the page are one surface. egui leaves its own
        // background clear once this exists.
        let wash = NSBox::initWithFrame(NSBox::alloc(mtm), bounds);
        wash.setBoxType(NSBoxType::Custom);
        wash.setBorderWidth(0.0);
        wash.setTitlePosition(NSTitlePosition::NoTitle);
        wash.setFillColor(&ns_color(tint));
        wash.setAutoresizingMask(fill);
        fx.addSubview(&wash);

        frame_view.addSubview_positioned_relativeTo(&fx, NSWindowOrderingMode::Below, None);
        TINT.with(|t| *t.borrow_mut() = Some(wash));
        return true;
    }
    false
}

/// Repaints the wash after a theme change.
pub fn set_backdrop_tint(tint: [f64; 4]) {
    if MainThreadMarker::new().is_none() {
        return;
    }
    TINT.with(|t| {
        if let Some(wash) = t.borrow().as_ref() {
            wash.setFillColor(&ns_color(tint));
        }
    });
}
