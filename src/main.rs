mod app;
mod audio;
mod engines;
mod settings;
mod settings_view;
mod stats;
mod theme;
mod views;

#[cfg(target_os = "macos")]
mod macos_bundle {
    use std::path::Path;
    use std::process::Command;

    const PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key><string>open-dictate</string>
    <key>CFBundleIdentifier</key><string>app.opendictate.desktop</string>
    <key>CFBundleName</key><string>Open Dictate</string>
    <key>CFBundleDisplayName</key><string>Open Dictate</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>VERSION</string>
    <key>CFBundleVersion</key><string>VERSION</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSMicrophoneUsageDescription</key><string>Open Dictate listens while you hold or press your dictation shortcut, and sends the recording only to the speech model you chose.</string>
    <key>NSAppleEventsUsageDescription</key><string>Open Dictate pastes your dictated text into the app you are using.</string>
</dict>
</plist>
"#;

    fn same_file(a: &Path, b: &Path) -> bool {
        match (std::fs::read(a), std::fs::read(b)) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    /// Copies this program into an app bundle (when it changed) and opens that instead.
    /// Returns true when the bundled copy was started and this process should exit.
    pub fn relaunch() -> bool {
        let Ok(exe) = std::env::current_exe() else { return false };
        if exe.to_string_lossy().contains(".app/Contents/MacOS/") {
            return false; // already running as an app
        }
        let bundle = crate::settings::data_dir().join("Open Dictate.app");
        let macos = bundle.join("Contents/MacOS");
        let target = macos.join("open-dictate");
        let plist = PLIST.replace("VERSION", env!("CARGO_PKG_VERSION"));
        let plist_path = bundle.join("Contents/Info.plist");
        let fresh = !same_file(&exe, &target) || std::fs::read_to_string(&plist_path).ok().as_deref() != Some(plist.as_str());
        if fresh {
            // Remove before copying so a running copy keeps its own file.
            let _ = std::fs::remove_file(&target);
            if std::fs::create_dir_all(&macos).is_err()
                || std::fs::write(&plist_path, &plist).is_err()
                || std::fs::copy(&exe, &target).is_err()
            {
                return false;
            }
            // Sign the bundle so macOS ties the microphone permission to it.
            let _ = Command::new("codesign").args(["--force", "--deep", "--sign", "-"]).arg(&bundle).output();
        }
        Command::new("open").arg(&bundle).status().is_ok_and(|s| s.success())
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--mic-check") {
        // Records two seconds from every microphone and prints how loud it was.
        for name in audio::input_devices() {
            match audio::Recording::start(&name, 1.0) {
                Ok(r) => {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let info = r.info();
                    let clip = r.finish();
                    let peak = clip.samples.iter().fold(0.0f32, |m, v| m.max(v.abs()));
                    println!("{name}: {info} -> {:.1}s, peak {peak:.4}", clip.seconds());
                }
                Err(e) => println!("{name}: {e}"),
            }
        }
        return Ok(());
    }
    let screenshot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from);

    // macOS only lets proper app bundles ask for the microphone; a bare program
    // just gets silence. Re-open ourselves as "Open Dictate.app" for normal use.
    #[cfg(target_os = "macos")]
    if screenshot.is_none() && !args.iter().any(|a| a == "--no-bundle") && macos_bundle::relaunch() {
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Open Dictate")
            .with_inner_size([1360.0, 920.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Open Dictate",
        options,
        Box::new(move |cc| Ok(Box::new(app::DictateApp::new(cc, screenshot)))),
    )
}
