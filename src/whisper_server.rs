use crate::settings::{Settings, SpeechEngine};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub fn find_server_bin(whisper_cli: &str) -> Option<PathBuf> {
    let beside = crate::engines::find_whisper_bin(whisper_cli)
        .and_then(|cli| cli.parent().map(|d| d.join("whisper-server")))
        .filter(|p| p.is_file());
    beside.or_else(|| crate::engines::find_whisper_bin("whisper-server"))
}

fn inference_fields<'a>(language: &'a str, prompt: &'a str) -> Vec<(&'static str, &'a str)> {
    let language = if language.is_empty() { "auto" } else { language };
    let mut fields = vec![("response_format", "json"), ("language", language)];
    if !prompt.is_empty() {
        fields.push(("prompt", prompt));
    }
    fields
}

pub struct WhisperServer {
    child: Option<std::process::Child>,
    port: u16,
}

impl WhisperServer {
    pub fn start(s: &Settings, stop: &AtomicBool, dir: &Path) -> Result<WhisperServer, String> {
        let bin = find_server_bin(&s.whisper_bin).ok_or("whisper-server not found")?;
        if !Path::new(&s.whisper_model).is_file() {
            return Err("No speech model chosen".into());
        }
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .map_err(|e| format!("no free port: {e}"))?
            .port();
        let stderr = std::fs::File::create(dir.join("whisper-server.log")).map(std::process::Stdio::from).unwrap_or_else(|_| std::process::Stdio::null());
        let child = std::process::Command::new(bin)
            .arg("-m")
            .arg(&s.whisper_model)
            .args(["--host", "127.0.0.1", "--port", &port.to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(stderr)
            .spawn()
            .map_err(|e| format!("could not run whisper-server: {e}"))?;
        let mut server = WhisperServer { child: Some(child), port };
        server.wait_until_ready(stop)?;
        Ok(server)
    }

    fn wait_until_ready(&mut self, stop: &AtomicBool) -> Result<(), String> {
        let probe = crate::engines::agent(2);
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if stop.load(Ordering::Relaxed) {
                return Err("stopped while whisper-server was loading".into());
            }
            if let Some(status) = self.child.as_mut().and_then(|c| c.try_wait().ok().flatten()) {
                return Err(format!("whisper-server exited: {status}"));
            }
            if probe.get(format!("http://127.0.0.1:{}/", self.port)).call().is_ok_and(|r| r.status() == 200) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err("whisper-server did not become ready in 30 s".into())
    }

    pub fn transcribe(&self, s: &Settings, wav: &[u8]) -> Result<String, String> {
        let prompt = crate::engines::dictionary_prompt(s);
        let fields = inference_fields(s.language.trim(), &prompt);
        let (content_type, body) = crate::engines::multipart(&fields, wav);
        let resp = crate::engines::agent(180)
            .post(format!("http://127.0.0.1:{}/inference", self.port))
            .header("Content-Type", content_type)
            .send(&body[..])
            .map_err(|e| format!("whisper-server unreachable: {e}"))?;
        let v = crate::engines::read_json(resp)?;
        Ok(crate::engines::clean_transcript(v["text"].as_str().unwrap_or_default()))
    }
}

impl Drop for WhisperServer {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

pub fn transcribe_stretch(
    server: &mut Option<WhisperServer>,
    s: &Settings,
    wav: &[u8],
    fallback: &dyn Fn(&[u8]) -> Result<String, String>,
) -> Result<String, String> {
    if s.speech_engine == SpeechEngine::WhisperLocal {
        if let Some(srv) = server.as_ref() {
            match srv.transcribe(s, wav) {
                Ok(text) => return Ok(text),
                Err(_) => *server = None,
            }
        }
    }
    fallback(wav)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("od-ws-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn executable(dir: &std::path::Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn finds_whisper_server_next_to_the_configured_cli() {
        let dir = temp_dir("sibling");
        let cli = executable(&dir, "whisper-cli");
        let server = executable(&dir, "whisper-server");
        assert_eq!(find_server_bin(cli.to_str().unwrap()), Some(server));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn returns_none_when_no_server_binary_exists() {
        let dir = temp_dir("none");
        let cli = executable(&dir, "whisper-cli");
        let found = find_server_bin(cli.to_str().unwrap());
        assert!(found.is_none_or(|p| !p.starts_with(&dir)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn falls_back_to_whisper_cli_when_the_server_request_fails() {
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let mut server = Some(WhisperServer { child: None, port: closed });
        let calls = std::cell::Cell::new(0);
        let cli = |_: &[u8]| {
            calls.set(calls.get() + 1);
            Ok("from the cli".to_string())
        };
        let out = transcribe_stretch(&mut server, &Settings::default(), b"wav", &cli);
        assert_eq!(out, Ok("from the cli".to_string()));
        assert_eq!(calls.get(), 1);
        assert!(server.is_none());
    }

    #[test]
    fn uses_the_cli_when_there_is_no_server() {
        let mut server = None;
        let cli = |_: &[u8]| Ok("cli".to_string());
        assert_eq!(transcribe_stretch(&mut server, &Settings::default(), b"w", &cli), Ok("cli".to_string()));
    }

    #[test]
    fn asks_the_server_to_detect_the_language_when_set_to_auto() {
        for lang in ["", "auto"] {
            assert!(inference_fields(lang, "").contains(&("language", "auto")));
        }
    }

    #[test]
    fn passes_an_explicit_language_through() {
        let fields = inference_fields("fr", "Vocabulary: x.");
        assert!(fields.contains(&("language", "fr")));
        assert!(fields.contains(&("prompt", "Vocabulary: x.")));
    }

    #[test]
    fn stopping_while_the_server_loads_returns_promptly() {
        let dir = temp_dir("loading");
        let cli = executable(&dir, "whisper-cli");
        let server = dir.join("whisper-server");
        std::fs::write(&server, "#!/bin/sh\nexec sleep 60\n").unwrap();
        std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o755)).unwrap();
        let model = dir.join("model.bin");
        std::fs::write(&model, "x").unwrap();
        let mut s = Settings::default();
        s.whisper_bin = cli.to_str().unwrap().into();
        s.whisper_model = model.to_str().unwrap().into();
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            flag.store(true, Ordering::Relaxed);
        });
        let began = Instant::now();
        let result = WhisperServer::start(&s, &stop, &dir);
        assert!(result.is_err());
        assert!(began.elapsed() < Duration::from_secs(2), "{:?}", began.elapsed());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn real_settings() -> Settings {
        let mut s = Settings::default();
        s.whisper_model = std::env::var("WHISPER_MODEL").expect("set WHISPER_MODEL to a ggml model file");
        s
    }

    fn fixture() -> Vec<u8> {
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/question.wav")).unwrap()
    }

    #[test]
    #[ignore = "needs WHISPER_MODEL and whisper-server"]
    fn transcribes_a_wav_through_a_real_server() {
        let s = real_settings();
        let server = WhisperServer::start(&s, &AtomicBool::new(false), &std::env::temp_dir()).unwrap();
        let text = server.transcribe(&s, &fixture()).unwrap();
        assert!(text.to_lowercase().contains("cost estimate"), "{text}");
    }

    #[test]
    #[ignore = "needs WHISPER_MODEL, whisper-server and whisper-cli"]
    fn times_server_calls_against_cli_calls() {
        let s = real_settings();
        let wav = fixture();
        let server = WhisperServer::start(&s, &AtomicBool::new(false), &std::env::temp_dir()).unwrap();
        server.transcribe(&s, &wav).unwrap();
        let t = std::time::Instant::now();
        for _ in 0..10 {
            server.transcribe(&s, &wav).unwrap();
        }
        let server_avg = t.elapsed() / 10;
        let t = std::time::Instant::now();
        for _ in 0..3 {
            crate::engines::transcribe(&s, &wav).unwrap();
        }
        let cli_avg = t.elapsed() / 3;
        println!("server avg {server_avg:?} over 10, cli avg {cli_avg:?} over 3");
        assert!(server_avg < cli_avg);
    }
}
