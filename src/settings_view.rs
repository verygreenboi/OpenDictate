//! The Settings window: Models, Settings (shortcuts, language, output),
//! Personalization and About. Plus the "press your shortcut" capture overlay.

use crate::app::{DictateApp, SettingsTab};
use crate::engines;
use crate::settings::{Action, SpeechEngine, TextEngine, Tone};
use crate::theme::{self, b, m, t};
use crate::views::{VERSION, icon_button, kbd, nav_item};
use eframe::egui::{self, Align, Align2, Color32, CornerRadius, Layout, Margin, Sense, Stroke, vec2};

pub fn settings_window(app: &mut DictateApp, ctx: &egui::Context) {
    let screen = ctx.content_rect();
    let size = vec2((screen.width() - 80.0).min(940.0), (screen.height() - 80.0).min(660.0));
    let modal = egui::Modal::new(egui::Id::new("settings"))
        .backdrop_color(Color32::from_black_alpha(150))
        .frame(
            egui::Frame::new()
                .fill(Color32::from_rgb(0x34, 0x34, 0x34))
                .corner_radius(12)
                .stroke(Stroke::new(1.0, Color32::from_rgb(0x44, 0x44, 0x44))),
        );
    let resp = modal.show(ctx, |ui| {
        ui.set_min_size(size);
        ui.set_max_size(size);
        let tab = app.settings_tab.unwrap_or(SettingsTab::General);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let (nav, _) = ui.allocate_exact_size(vec2(202.0, size.y), Sense::hover());
            ui.painter().rect_filled(nav, CornerRadius { nw: 12, sw: 12, ne: 0, se: 0 }, Color32::from_rgb(0x3A, 0x3A, 0x3A));
            let mut nav_ui = ui.new_child(
                egui::UiBuilder::new().max_rect(nav.shrink2(vec2(12.0, 22.0))).layout(Layout::top_down(Align::Min)),
            );
            for (tab_, icon, label) in [
                (SettingsTab::Models, "mic", "Models"),
                (SettingsTab::General, "gear", "Settings"),
                (SettingsTab::Audio, "mic", "Audio input"),
                (SettingsTab::Personalization, "spark", "Personalization"),
                (SettingsTab::About, "info", "About"),
            ] {
                if nav_item(&mut nav_ui, icon, label, tab == tab_).clicked() {
                    app.settings_tab = Some(tab_);
                    if tab_ == SettingsTab::Models {
                        app.refresh_health(ctx);
                    }
                    if tab_ == SettingsTab::Audio {
                        app.refresh_mics();
                    }
                }
            }
            ui.vertical(|ui| {
                ui.set_width(size.x - 202.0);
                ui.set_height(size.y);
                ui.allocate_ui_with_layout(vec2(size.x - 202.0, 36.0), Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(14.0);
                    if icon_button(ui, "close", "Close").clicked() {
                        app.settings_tab = None;
                    }
                });
                egui::ScrollArea::vertical().id_salt(("settings-scroll", tab as u8)).auto_shrink(false).show(ui, |ui| {
                    egui::Frame::new().inner_margin(Margin { left: 36, right: 36, top: 0, bottom: 30 }).show(ui, |ui| {
                        ui.set_width(size.x - 202.0 - 72.0);
                        match tab {
                            SettingsTab::Models => models_tab(app, ui),
                            SettingsTab::General => general_tab(app, ui),
                            SettingsTab::Audio => audio_tab(app, ui),
                            SettingsTab::Personalization => personal_tab(app, ui),
                            SettingsTab::About => about_tab(app, ui),
                        }
                    });
                });
            });
        });
    });
    if resp.should_close() && app.capturing_key.is_none() {
        app.settings_tab = None;
    }
}

fn section(ui: &mut egui::Ui, icon: &str, title: &str) {
    ui.add_space(18.0);
    ui.label(m(format!("{icon}  {title}"), 16.5, Color32::from_rgb(0xC8, 0xC8, 0xC8)));
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, Color32::from_rgb(0x46, 0x46, 0x46));
    ui.add_space(12.0);
}

/// A settings row: title and description on the left, the control on the right.
fn row(ui: &mut egui::Ui, title: &str, desc: &str, control_w: f32, control: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal_top(|ui| {
        let left = ui.available_width() - control_w - 16.0;
        ui.vertical(|ui| {
            ui.set_width(left);
            ui.label(m(title, 14.0, theme::TEXT));
            if !desc.is_empty() {
                ui.label(t(desc, 13.5, Color32::from_rgb(0xB8, 0xB8, 0xB8)));
            }
        });
        ui.add_space(16.0);
        ui.vertical(|ui| {
            ui.set_width(control_w);
            control(ui);
        });
    });
    ui.add_space(14.0);
}

fn models_tab(app: &mut DictateApp, ui: &mut egui::Ui) {
    ui.label(b("Models", 24.0, theme::TEXT));
    ui.label(t("You choose what turns your voice into text and what tidies it up. Nothing runs anywhere you didn't pick.", 13.5, theme::MUTED));
    ui.add_space(10.0);
    egui::Frame::new()
        .fill(Color32::from_rgb(0x2E, 0x3A, 0x5E))
        .stroke(Stroke::new(1.0, Color32::from_rgb(0x44, 0x5A, 0x96)))
        .corner_radius(10)
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(m("What you need", 14.0, theme::TEXT));
            ui.label(t(
                "• Speech model (required): turns your voice into text. A downloaded Whisper model is all Dictate needs.\n\
                 • Text model (optional): removes filler words, fixes corrections and formats. Translate and Ask anything need it. \
                 Without one, Dictate pastes exactly what you said.",
                13.0,
                Color32::from_rgb(0xD8, 0xDE, 0xF0),
            ));
        });
    let ctx = ui.ctx().clone();
    let health = app.health.lock().unwrap().clone();
    let mut changed = false;

    section(ui, "🎤", "Speech to text");
    row(ui, "Engine", "Where your recordings are turned into text.", 300.0, |ui| {
        changed |= ui.radio_value(&mut app.s.speech_engine, SpeechEngine::WhisperLocal, "whisper.cpp on this Mac").changed();
        changed |= ui.radio_value(&mut app.s.speech_engine, SpeechEngine::OpenAiCompatible, "OpenAI-compatible server").changed();
    });
    match app.s.speech_engine {
        SpeechEngine::WhisperLocal => {
            row(ui, "whisper.cpp program", "The whisper-cli command (brew install whisper-cpp).", 300.0, |ui| {
                changed |= ui.add(egui::TextEdit::singleline(&mut app.s.whisper_bin).desired_width(300.0)).changed();
                match &health.whisper_bin {
                    Some(p) => ui.label(t(format!("✔ Found at {}", p.display()), 12.5, theme::GREEN)),
                    None if health.checked => ui.label(t("✖ Not found", 12.5, theme::RED)),
                    None => ui.label(t("Checking…", 12.5, theme::MUTED)),
                };
            });
            let local = engines::local_whisper_models();
            row(ui, "Model", "A ggml model file. Bigger models are more accurate but slower.", 300.0, |ui| {
                let current = engines::speech_model_label(&app.s);
                egui::ComboBox::from_id_salt("wmodel").width(300.0).selected_text(current).show_ui(ui, |ui| {
                    if local.is_empty() {
                        ui.label("No models downloaded yet");
                    }
                    for p in &local {
                        let s = p.to_string_lossy().into_owned();
                        let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
                        changed |= ui.selectable_value(&mut app.s.whisper_model, s, name).changed();
                    }
                });
                ui.label(t("Or paste the path to your own model file:", 12.0, theme::FAINT));
                changed |= ui.add(egui::TextEdit::singleline(&mut app.s.whisper_model).desired_width(300.0)).changed();
            });
            ui.label(m("Download a model", 14.0, theme::TEXT));
            ui.add_space(4.0);
            for choice in engines::WHISPER_MODELS {
                let have = local.iter().any(|p| p.file_name().is_some_and(|f| f == choice.file));
                egui::Frame::new().fill(Color32::from_rgb(0x2E, 0x2E, 0x2E)).corner_radius(8).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(m(format!("{}  ·  {}", choice.label, choice.size), 13.5, theme::TEXT));
                            ui.label(t(choice.note, 12.5, theme::MUTED));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if let Some((f, p)) = &app.download {
                                if f == choice.file {
                                    let p = p.lock().unwrap();
                                    let frac = if p.total > 0 { p.done as f32 / p.total as f32 } else { 0.0 };
                                    ui.add(egui::ProgressBar::new(frac).desired_width(180.0).show_percentage());
                                    return;
                                }
                            }
                            if have {
                                let path = crate::settings::models_dir().join(choice.file).to_string_lossy().into_owned();
                                if app.s.whisper_model == path {
                                    ui.label(t("✔ In use", 13.0, theme::GREEN));
                                } else if ui.button("Use").clicked() {
                                    app.s.whisper_model = path;
                                    changed = true;
                                }
                            } else if ui.add_enabled(app.download.is_none(), egui::Button::new("Download")).clicked() {
                                let p: engines::SharedProgress = Default::default();
                                engines::download_whisper(choice.file, p.clone(), ctx.clone());
                                app.download = Some((choice.file.to_string(), p));
                            }
                        });
                    });
                });
                ui.add_space(4.0);
            }
            ui.add_space(10.0);
        }
        SpeechEngine::OpenAiCompatible => {
            row(ui, "Server address", "Base URL ending in /v1 (OpenAI, Groq, a local faster-whisper server…).", 300.0, |ui| {
                changed |= ui.add(egui::TextEdit::singleline(&mut app.s.speech_url).desired_width(300.0)).changed();
            });
            row(ui, "API key", "Leave empty for servers that don't need one.", 300.0, |ui| {
                changed |= ui.add(egui::TextEdit::singleline(&mut app.s.speech_key).password(true).desired_width(300.0)).changed();
            });
            row(ui, "Model name", "For example whisper-1 or whisper-large-v3-turbo.", 300.0, |ui| {
                changed |= ui.add(egui::TextEdit::singleline(&mut app.s.speech_model).desired_width(300.0)).changed();
            });
        }
    }
    row(ui, "Spoken language", "\"auto\" detects it, or set a code like en, es, fr, yo.", 300.0, |ui| {
        changed |= ui.add(egui::TextEdit::singleline(&mut app.s.language).desired_width(120.0)).changed();
    });

    section(ui, "✨", "Text clean-up");
    row(ui, "Engine", "What tidies up, translates and answers.", 300.0, |ui| {
        changed |= ui.radio_value(&mut app.s.text_engine, TextEngine::Ollama, "Ollama on this Mac").changed();
        changed |= ui.radio_value(&mut app.s.text_engine, TextEngine::OpenAiCompatible, "OpenAI-compatible server").changed();
        changed |= ui.radio_value(&mut app.s.text_engine, TextEngine::Off, "Off — paste exactly what I said").changed();
    });
    match app.s.text_engine {
        TextEngine::Ollama => {
            row(ui, "Ollama address", "", 300.0, |ui| {
                changed |= ui.add(egui::TextEdit::singleline(&mut app.s.ollama_url).desired_width(300.0)).changed();
                match &health.ollama {
                    Some(Ok(list)) => {
                        ui.label(t(format!("✔ Running · {} model{}", list.len(), if list.len() == 1 { "" } else { "s" }), 12.5, theme::GREEN));
                    }
                    Some(Err(_)) => {
                        ui.horizontal(|ui| {
                            ui.label(t("✖ Not running", 12.5, theme::RED));
                            if ui.small_button("Start Ollama").clicked() {
                                match engines::start_ollama() {
                                    Ok(()) => app.notify("Starting Ollama…", false),
                                    Err(e) => app.notify(e, true),
                                }
                                let h = app.health.clone();
                                let c = ctx.clone();
                                let url = app.s.ollama_url.clone();
                                std::thread::spawn(move || {
                                    std::thread::sleep(std::time::Duration::from_secs(3));
                                    h.lock().unwrap().ollama = Some(engines::ollama_models(&url));
                                    c.request_repaint();
                                });
                            }
                        });
                    }
                    None => {
                        ui.label(t("Checking…", 12.5, theme::MUTED));
                    }
                }
            });
            let list = health.ollama.clone().and_then(|r| r.ok()).unwrap_or_default();
            row(ui, "Model", "Small models (3–8B) are quick enough for dictation.", 300.0, |ui| {
                egui::ComboBox::from_id_salt("omodel").width(300.0).selected_text(app.s.ollama_model.clone()).show_ui(ui, |ui| {
                    if list.is_empty() {
                        ui.label("No models found");
                    }
                    for name in &list {
                        changed |= ui.selectable_value(&mut app.s.ollama_model, name.clone(), name).changed();
                    }
                });
                changed |= ui.add(egui::TextEdit::singleline(&mut app.s.ollama_model).desired_width(300.0)).changed();
            });
            row(ui, "Download a model", "Any name from the Ollama library.", 300.0, |ui| {
                ui.horizontal(|ui| {
                    for sug in ["llama3.2:3b", "qwen2.5:3b", "gemma3:4b"] {
                        if ui.small_button(sug).clicked() {
                            app.pull_name = sug.into();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut app.pull_name).hint_text("model name").desired_width(190.0));
                    let busy = app.download.is_some();
                    if ui.add_enabled(!busy && !app.pull_name.trim().is_empty(), egui::Button::new("Download")).clicked() {
                        let p: engines::SharedProgress = Default::default();
                        engines::pull_ollama(&app.s.ollama_url, &app.pull_name, p.clone(), ctx.clone());
                        app.download = Some((format!("ollama:{}", app.pull_name.trim()), p));
                        app.s.ollama_model = app.pull_name.trim().to_string();
                        changed = true;
                    }
                });
                if let Some((f, p)) = &app.download {
                    if f.starts_with("ollama:") {
                        let p = p.lock().unwrap();
                        let frac = if p.total > 0 { p.done as f32 / p.total as f32 } else { 0.0 };
                        ui.add(egui::ProgressBar::new(frac).desired_width(300.0).text(p.status.clone()));
                    }
                }
            });
        }
        TextEngine::OpenAiCompatible => {
            row(ui, "Server address", "Base URL ending in /v1.", 300.0, |ui| {
                changed |= ui.add(egui::TextEdit::singleline(&mut app.s.text_url).desired_width(300.0)).changed();
                ui.horizontal_wrapped(|ui| {
                    for (name, url) in [
                        ("LM Studio", "http://localhost:1234/v1"),
                        ("OpenAI", "https://api.openai.com/v1"),
                        ("OpenRouter", "https://openrouter.ai/api/v1"),
                        ("Groq", "https://api.groq.com/openai/v1"),
                    ] {
                        if ui.small_button(name).clicked() {
                            app.s.text_url = url.into();
                            changed = true;
                        }
                    }
                });
            });
            row(ui, "API key", "Leave empty for local servers.", 300.0, |ui| {
                changed |= ui.add(egui::TextEdit::singleline(&mut app.s.text_key).password(true).desired_width(300.0)).changed();
            });
            let list = health.remote_text.clone().and_then(|r| r.ok()).unwrap_or_default();
            row(ui, "Model name", "", 300.0, |ui| {
                if !list.is_empty() {
                    egui::ComboBox::from_id_salt("rmodel").width(300.0).selected_text(app.s.text_model.clone()).show_ui(ui, |ui| {
                        for name in &list {
                            changed |= ui.selectable_value(&mut app.s.text_model, name.clone(), name).changed();
                        }
                    });
                }
                changed |= ui.add(egui::TextEdit::singleline(&mut app.s.text_model).desired_width(300.0)).changed();
                if ui.small_button("Fetch model list").clicked() {
                    app.refresh_health(&ctx);
                }
                if let Some(Err(e)) = &health.remote_text {
                    ui.label(t(e, 12.0, theme::RED));
                }
            });
        }
        TextEngine::Off => {}
    }

    section(ui, "🧪", "Try it");
    ui.label(t("Type something messy, the way people talk, and see what your text model makes of it.", 13.5, theme::MUTED));
    ui.add(egui::TextEdit::multiline(&mut app.test_input).desired_width(f32::INFINITY).desired_rows(2));
    let running = app.test_running.load(std::sync::atomic::Ordering::SeqCst);
    let btn = egui::Button::new(b(if running { "Running…" } else { "Run test" }, 14.0, Color32::WHITE)).fill(theme::ACCENT);
    if ui.add_enabled(!running, btn).clicked() {
        app.run_test(&ctx);
    }
    if let Some(r) = app.test_output.lock().unwrap().clone() {
        egui::Frame::new().fill(Color32::from_rgb(0x2A, 0x2A, 0x2A)).corner_radius(8).inner_margin(Margin::same(10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            match r {
                Ok(text) => ui.label(t(text, 14.0, theme::TEXT)),
                Err(e) => ui.label(t(e, 13.5, theme::RED)),
            };
        });
    }

    if changed {
        app.changed();
        app.refresh_health(&ctx);
    }
}

fn general_tab(app: &mut DictateApp, ui: &mut egui::Ui) {
    ui.label(b("Settings", 24.0, theme::TEXT));
    section(ui, "⌨", "Shortcuts");
    ui.horizontal(|ui| {
        let mut hold = app.s.hold_to_talk;
        ui.selectable_value(&mut hold, false, "Press to start, press to stop");
        ui.selectable_value(&mut hold, true, "Hold while speaking");
        if hold != app.s.hold_to_talk {
            app.s.hold_to_talk = hold;
            app.changed();
        }
    });
    ui.add_space(14.0);
    let mut keys_changed = false;
    for a in Action::ALL {
        let mut desc = if app.s.hold_to_talk {
            format!("Hold to keep {}, let go to finish.", a.verb())
        } else {
            format!("Press to start and stop {}.", a.verb())
        };
        if a == Action::Ask {
            desc.push_str(" Select text first to have it rewritten.");
        }
        row(ui, a.label(), &desc, 204.0, |ui| {
            let mut remove = None;
            for (i, k) in app.s.keys(a).iter().enumerate() {
                egui::Frame::new()
                    .stroke(Stroke::new(1.0, Color32::from_rgb(0x55, 0x55, 0x55)))
                    .corner_radius(8)
                    .inner_margin(Margin::symmetric(6, 5))
                    .show(ui, |ui| {
                        ui.set_width(190.0);
                        ui.horizontal(|ui| {
                            kbd(ui, k);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if icon_button(ui, "close", "Remove").clicked() {
                                    remove = Some(i);
                                }
                            });
                        });
                    });
            }
            if let Some(i) = remove {
                app.s.keys_mut(a).remove(i);
                keys_changed = true;
            }
            let label = if app.capturing_key == Some(a) { "Press keys…" } else { "Add another" };
            let btn = egui::Button::new(t(label, 14.0, theme::TEXT)).stroke(Stroke::new(1.0, Color32::from_rgb(0x55, 0x55, 0x55))).fill(Color32::TRANSPARENT);
            if ui.add(btn).clicked() {
                app.capturing_key = Some(a);
            }
        });
    }
    for e in app.hotkey_errors.clone() {
        ui.label(t(format!("⚠ {e}"), 12.5, theme::RED));
    }
    if keys_changed {
        app.changed();
        app.register_hotkeys();
    }

    section(ui, "🌐", "Language");
    let langs = [
        "Spanish", "French", "German", "Portuguese", "Italian", "English", "Yoruba", "Igbo", "Hausa", "Chinese", "Japanese", "Korean", "Arabic", "Hindi",
    ];
    row(ui, "Translation target", "Language the Translate shortcut writes in.", 204.0, |ui| {
        let mut v = app.s.translate_to.clone();
        egui::ComboBox::from_id_salt("tr").width(204.0).selected_text(v.clone()).show_ui(ui, |ui| {
            for l in langs {
                ui.selectable_value(&mut v, l.to_string(), l);
            }
        });
        if v != app.s.translate_to {
            app.s.translate_to = v;
            app.changed();
        }
    });
    row(ui, "Spoken language", "\"auto\" to detect, or a code like en.", 204.0, |ui| {
        if ui.add(egui::TextEdit::singleline(&mut app.s.language).desired_width(120.0)).changed() {
            app.changed();
        }
    });

    section(ui, "📋", "Output");
    let mut c = false;
    row(ui, "Paste into the app I'm using", "Otherwise the text is only copied.", 204.0, |ui| {
        c |= ui.checkbox(&mut app.s.auto_paste, "").changed();
    });
    row(ui, "Keep my clipboard", "Put back what was on the clipboard after pasting.", 204.0, |ui| {
        c |= ui.checkbox(&mut app.s.restore_clipboard, "").changed();
    });
    row(ui, "Show floating indicator", "A small pill at the bottom of the screen while listening.", 204.0, |ui| {
        c |= ui.checkbox(&mut app.s.show_indicator, "").changed();
    });
    if c {
        app.changed();
    }
    ui.label(t(
        "Pasting needs Accessibility access and recording needs Microphone access (System Settings › Privacy & Security).",
        12.5,
        theme::FAINT,
    ));
}

fn audio_tab(app: &mut DictateApp, ui: &mut egui::Ui) {
    ui.label(b("Audio input", 24.0, theme::TEXT));
    section(ui, "🎙", "Microphone");
    let mut changed = false;
    row(ui, "Microphone", "Which microphone records your voice.", 260.0, |ui| {
        let default_label = match &app.default_mic {
            Some(n) => format!("System default ({n})"),
            None => "System default".to_string(),
        };
        let selected = if app.s.microphone.is_empty() { default_label.clone() } else { app.s.microphone.clone() };
        let mut mic = app.s.microphone.clone();
        egui::ComboBox::from_id_salt("mic").width(260.0).selected_text(selected).show_ui(ui, |ui| {
            ui.selectable_value(&mut mic, String::new(), default_label);
            for name in &app.mics {
                ui.selectable_value(&mut mic, name.clone(), name);
            }
        });
        if !mic.is_empty() && !app.mics.contains(&mic) {
            ui.label(t("⚠ This microphone isn't connected; the system default is used.", 12.5, theme::RED));
        }
        if mic != app.s.microphone {
            app.s.microphone = mic;
            changed = true;
        }
        if ui.small_button("Refresh list").clicked() {
            app.refresh_mics();
        }
    });
    row(ui, "Input level", "Speak to check the bar moves. Aim for the middle when talking normally.", 260.0, |ui| {
        let level = app.mic_test.as_ref().map(|r| r.level()).unwrap_or(0.0);
        let (r, _) = ui.allocate_exact_size(vec2(260.0, 10.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(r, 5.0, Color32::from_rgb(0x26, 0x26, 0x26));
        let frac = (level * 2.0).clamp(0.0, 1.0);
        if frac > 0.0 {
            let mut fill = r;
            fill.set_width(r.width() * frac);
            let c = if frac > 0.95 { theme::RED } else if frac > 0.08 { theme::GREEN } else { theme::FAINT };
            p.rect_filled(fill, 5.0, c);
        }
        ui.add_space(4.0);
        let testing = app.mic_test.is_some();
        let label = if testing { "Stop test" } else { "Test microphone" };
        let btn = egui::Button::new(m(label, 13.5, Color32::WHITE)).fill(if testing { theme::RED } else { theme::ACCENT });
        if ui.add_enabled(app.recording.is_none(), btn).clicked() {
            app.toggle_mic_test();
        }
        if let Some(r) = &app.mic_test {
            if let Some(e) = r.error() {
                ui.label(t(format!("⚠ The microphone stopped: {e}"), 12.5, theme::RED));
            } else if r.sample_count() == 0 && r.elapsed() > 1.5 {
                ui.label(t("⚠ No sound is arriving from this microphone. Try another one or reconnect it.", 12.5, theme::RED));
            } else if r.elapsed() > 1.5 && r.peak() == 0.0 {
                // Exact digital silence on a live device is what macOS sends when access is blocked.
                ui.label(t("⚠ macOS is blocking the microphone (it only sends silence).", 12.5, theme::RED));
                if ui.small_button("Open microphone privacy settings").clicked() {
                    open_mic_privacy();
                }
            } else {
                ui.label(t(if frac < 0.03 { "Not hearing anything yet…" } else { "Hearing you ✔" }, 12.5, theme::MUTED));
            }
            ui.label(t(r.info(), 11.5, theme::FAINT));
        }
    });
    row(ui, "Input boost", "Turn up if your words come out quiet or are missed.", 260.0, |ui| {
        changed |= ui.add(egui::Slider::new(&mut app.s.input_gain, 0.5..=4.0).step_by(0.25).suffix("×")).changed();
    });
    if changed {
        app.changed();
        // Restart a running test so it uses the new microphone and boost.
        if app.mic_test.is_some() {
            app.mic_test = None;
            app.toggle_mic_test();
        }
    }
    section(ui, "🔒", "Permission");
    row(
        ui,
        "Microphone access",
        "macOS asks once whether Open Dictate may use the microphone. If you said no, or the bar never moves, switch Open Dictate on in Privacy & Security › Microphone, then quit and reopen it.",
        260.0,
        |ui| {
            if ui.button("Open microphone privacy settings").clicked() {
                open_mic_privacy();
            }
        },
    );
}

pub fn open_mic_privacy() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
        .spawn();
}

fn personal_tab(app: &mut DictateApp, ui: &mut egui::Ui) {
    ui.label(b("Personalization", 24.0, theme::TEXT));
    section(ui, "🎨", "Tone");
    let w = (ui.available_width() - 24.0) / 4.0;
    ui.horizontal(|ui| {
        for tone in Tone::ALL {
            let active = app.s.tone == tone;
            let resp = egui::Frame::new()
                .fill(if active { Color32::from_rgb(0x2E, 0x3A, 0x5E) } else { Color32::from_rgb(0x2C, 0x2C, 0x2C) })
                .stroke(Stroke::new(1.0, if active { theme::ACCENT } else { Color32::from_rgb(0x44, 0x44, 0x44) }))
                .corner_radius(10)
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(w - 22.0);
                    ui.set_min_height(56.0);
                    ui.label(m(tone.label(), 14.0, theme::TEXT));
                    ui.label(t(tone.blurb(), 12.0, theme::MUTED));
                });
            if ui.interact(resp.response.rect, ui.id().with(tone.label()), Sense::click()).clicked() {
                app.s.tone = tone;
                app.changed();
            }
        }
    });
    section(ui, "🧹", "Clean-up");
    let mut c = false;
    row(ui, "Remove filler words", "Drops um, uh, like, you know.", 60.0, |ui| {
        c |= ui.checkbox(&mut app.s.remove_fillers, "").changed();
    });
    row(ui, "Follow my corrections", "\"Tuesday, no wait, Wednesday\" becomes \"Wednesday\".", 60.0, |ui| {
        c |= ui.checkbox(&mut app.s.fix_corrections, "").changed();
    });
    row(ui, "Auto-format", "Turns spoken lists into bullet points and splits paragraphs.", 60.0, |ui| {
        c |= ui.checkbox(&mut app.s.auto_format, "").changed();
    });
    section(ui, "📝", "Your own instructions");
    ui.label(t("Anything else your text model should always do, e.g. \"Use British spelling.\"", 13.5, theme::MUTED));
    c |= ui.add(egui::TextEdit::multiline(&mut app.s.custom_instructions).desired_width(f32::INFINITY).desired_rows(3)).changed();
    section(ui, "⏱", "Time saved");
    row(ui, "Your typing speed", "Used to work out the time saved on Home.", 220.0, |ui| {
        c |= ui.add(egui::Slider::new(&mut app.s.typing_wpm, 15.0..=120.0).suffix(" WPM")).changed();
    });
    if c {
        app.changed();
    }
    section(ui, "👁", "What your text model is told");
    egui::Frame::new().fill(Color32::from_rgb(0x2A, 0x2A, 0x2A)).corner_radius(8).inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(t(engines::polish_prompt(&app.s, false), 12.5, theme::MUTED));
    });
}

fn about_tab(app: &mut DictateApp, ui: &mut egui::Ui) {
    ui.label(b("About", 24.0, theme::TEXT));
    ui.add_space(8.0);
    ui.label(t(format!("Open Dictate v{VERSION}"), 14.0, theme::TEXT));
    ui.label(t(
        "Voice dictation that runs on the models you choose. Speak in any app; your words are transcribed, tidied up and pasted where your cursor is.",
        13.5,
        theme::MUTED,
    ));
    section(ui, "📁", "Your data");
    let dir = crate::settings::data_dir();
    ui.label(t(dir.display().to_string(), 13.0, theme::MUTED));
    if ui.button("Open folder").clicked() {
        let _ = std::process::Command::new("open").arg(&dir).spawn();
    }
    section(ui, "📊", "Totals");
    ui.label(t(
        format!("{} dictations · {} words · {} in history", app.stats.sessions, app.stats.words, app.history.len()),
        13.5,
        theme::MUTED,
    ));
}

pub fn capture_shortcut(app: &mut DictateApp, ctx: &egui::Context) {
    let Some(action) = app.capturing_key else { return };
    let mut done: Option<Option<String>> = None;
    ctx.input(|i| {
        for e in &i.events {
            if let egui::Event::Key { key, pressed: true, modifiers, .. } = e {
                if *key == egui::Key::Escape {
                    done = Some(None);
                    break;
                }
                let mut parts = Vec::new();
                if modifiers.ctrl {
                    parts.push("Ctrl");
                }
                if modifiers.alt {
                    parts.push("Alt");
                }
                if modifiers.shift {
                    parts.push("Shift");
                }
                if modifiers.mac_cmd {
                    parts.push("Cmd");
                }
                parts.push(key.name());
                done = Some(Some(parts.join("+")));
                break;
            }
        }
    });
    egui::Area::new(egui::Id::new("capture")).order(egui::Order::Tooltip).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
        egui::Frame::new()
            .fill(Color32::from_rgb(0x1C, 0x1C, 0x1C))
            .stroke(Stroke::new(1.0, theme::ACCENT))
            .corner_radius(12)
            .inner_margin(Margin::same(20))
            .show(ui, |ui| {
                ui.label(b(format!("Press the shortcut for {}", action.label()), 16.0, theme::TEXT));
                ui.label(t("Function keys (F13–F19) or a combination like ⌥ Space work best. Esc cancels.", 13.0, theme::MUTED));
            });
    });
    if let Some(r) = done {
        app.capturing_key = None;
        if let Some(k) = r {
            match k.parse::<global_hotkey::hotkey::HotKey>() {
                Ok(_) => {
                    if !app.s.keys(action).contains(&k) {
                        app.s.keys_mut(action).push(k);
                        app.changed();
                        app.register_hotkeys();
                    }
                }
                Err(_) => app.notify(format!("{k} can't be used as a shortcut"), true),
            }
        }
    }
}
