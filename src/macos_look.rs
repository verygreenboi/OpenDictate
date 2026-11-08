//! The two things the window needs that egui has no builder for: a blurred
//! backdrop, and a Dock tile that does not depend on Launch Services having
//! noticed the bundle's icon.

use objc2::{AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSAutoresizingMaskOptions, NSColor, NSImage, NSVisualEffectBlendingMode,
    NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
};
use objc2_foundation::NSData;

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

/// Puts an NSVisualEffectView behind everything the app draws.
///
/// Transparency alone shows whatever is behind the window sharply, which is
/// distracting under text. This is the system's own backdrop blur, the same
/// one Finder and Notes sit on, so what is behind reads as texture instead of
/// as other people's windows.
///
/// Returns false until the window exists, so the caller keeps asking.
pub fn blur_window(min_width: f64) -> bool {
    let Some(mtm) = MainThreadMarker::new() else { return false };
    let app = NSApplication::sharedApplication(mtm);
    for window in app.windows().iter() {
        // The root viewport is a 1x1 anchor with the same title, so the real
        // window is found by being the only one of any size.
        if window.frame().size.width < min_width {
            continue;
        }
        let Some(content) = window.contentView() else { continue };
        {
            window.setOpaque(false);
            window.setBackgroundColor(Some(&NSColor::clearColor()));
            let fx = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), content.frame());
            // UnderWindowBackground is the material meant for exactly this:
            // the whole window sitting on the desktop rather than a sidebar
            // or a popover.
            fx.setMaterial(NSVisualEffectMaterial::UnderWindowBackground);
            fx.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
            // Without Active the blur stops when the window loses focus.
            fx.setState(NSVisualEffectState::Active);
            let fill = NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable;
            fx.setAutoresizingMask(fill);
            // In winit the content view IS the Metal view egui draws into, so
            // a subview sits on top of the drawing and hides it. The blur has
            // to become the container instead, with the renderer back inside
            // it: everything egui paints then composites over the blur.
            window.setContentView(Some(&fx));
            fx.addSubview(&content);
            content.setFrame(fx.bounds());
            content.setAutoresizingMask(fill);
        }
        return true;
    }
    false
}
