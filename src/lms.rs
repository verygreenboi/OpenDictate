use crate::settings::{Settings, TextEngine};
use std::path::PathBuf;

const PORT: u16 = 1234;

pub fn find() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".lmstudio/bin/lms"));
    let on_path = std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).map(|d| d.join("lms"));
    home.into_iter().chain(on_path).find(|p| p.is_file())
}

fn port_of(url: &str) -> Option<u16> {
    let rest = url.trim().split_once("://").map_or(url.trim(), |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next()?.rsplit('@').next()?;
    authority.rsplit_once(':').filter(|(_, p)| !p.contains(']')).and_then(|(_, p)| p.parse().ok())
}

pub fn is_lm_studio(s: &Settings) -> bool {
    s.text_engine == TextEngine::OpenAiCompatible && crate::talking::is_local(&s.text_url) && port_of(&s.text_url) == Some(PORT)
}

pub fn steps(server_running: bool, loaded: &[String], model: &str) -> Vec<Vec<String>> {
    let model = model.trim();
    let mut out = Vec::new();
    if !server_running {
        out.push(vec!["server".into(), "start".into()]);
    }
    if !model.is_empty() && !loaded.iter().any(|l| l == model) {
        out.push(vec!["load".into(), model.into(), "-y".into()]);
    }
    out
}

fn parse_running(json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json).ok().and_then(|v| v["running"].as_bool()).unwrap_or(false)
}

fn parse_loaded(json: &str) -> Vec<String> {
    let Ok(serde_json::Value::Array(models)) = serde_json::from_str(json) else { return Vec::new() };
    models.iter().flat_map(|m| [m["identifier"].as_str(), m["modelKey"].as_str()]).flatten().map(String::from).collect()
}

fn ensure_with(s: &Settings, run: &dyn Fn(&[String]) -> Result<String, String>) -> Result<(), String> {
    let model = if s.meeting_model.trim().is_empty() { &s.text_model } else { &s.meeting_model };
    let running = parse_running(&run(&["server".into(), "status".into(), "--json".into()])?);
    let loaded = if running { parse_loaded(&run(&["ps".into(), "--json".into()])?) } else { Vec::new() };
    steps(running, &loaded, model).iter().try_for_each(|step| run(step).map(drop))
}

pub fn ensure(s: &Settings) -> Result<(), String> {
    let lms = find().ok_or("lms is not installed")?;
    ensure_with(s, &|args| {
        let out = std::process::Command::new(&lms).args(args).output().map_err(|e| format!("lms {}: {e}", args.join(" ")))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(format!("lms {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn lm(url: &str, model: &str, meeting: &str) -> Settings {
        Settings {
            text_engine: TextEngine::OpenAiCompatible,
            text_url: url.into(),
            text_model: model.into(),
            meeting_model: meeting.into(),
            ..Settings::default()
        }
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn starts_the_server_and_loads_the_model_when_neither_is_running() {
        assert_eq!(steps(false, &[], "phi"), vec![strs(&["server", "start"]), strs(&["load", "phi", "-y"])]);
    }

    #[test]
    fn only_loads_the_model_when_the_server_is_already_up() {
        assert_eq!(steps(true, &strs(&["other"]), "phi"), vec![strs(&["load", "phi", "-y"])]);
    }

    #[test]
    fn does_nothing_when_the_model_is_already_loaded() {
        assert!(steps(true, &strs(&["phi"]), "phi").is_empty());
    }

    #[test]
    fn loads_nothing_when_no_model_is_chosen() {
        assert_eq!(steps(false, &[], " "), vec![strs(&["server", "start"])]);
    }

    #[test]
    fn is_not_lm_studio_for_ollama_or_a_cloud_url() {
        let mut ollama = lm("http://localhost:1234/v1", "m", "");
        ollama.text_engine = TextEngine::Ollama;
        assert!(!is_lm_studio(&ollama));
        assert!(!is_lm_studio(&lm("https://api.openai.com/v1", "m", "")));
        assert!(!is_lm_studio(&lm("http://localhost:11434/v1", "m", "")));
        assert!(!is_lm_studio(&lm("http://localhost/v1", "m", "")));
    }

    #[test]
    fn is_lm_studio_for_a_local_openai_compatible_server_on_1234() {
        assert!(is_lm_studio(&lm("http://localhost:1234/v1", "m", "")));
        assert!(is_lm_studio(&lm("http://127.0.0.1:1234", "m", "")));
        assert!(is_lm_studio(&lm("http://[::1]:1234/v1", "m", "")));
    }

    #[test]
    fn reads_whether_the_server_runs_from_status_json() {
        assert!(parse_running(r#"{"running":true,"port":1234}"#));
        assert!(!parse_running(r#"{"running":false}"#));
        assert!(!parse_running("The server is not running."));
    }

    #[test]
    fn reads_identifiers_and_model_keys_of_loaded_models() {
        let json = r#"[{"identifier":"phi-4-mini-instruct-chat-mlx","modelKey":"phi-4-mini","path":"x/y"}]"#;
        assert_eq!(parse_loaded(json), strs(&["phi-4-mini-instruct-chat-mlx", "phi-4-mini"]));
        assert!(parse_loaded("[]").is_empty());
        assert!(parse_loaded("garbage").is_empty());
    }

    fn recorded(s: &Settings, status: &str, ps: &str) -> Vec<Vec<String>> {
        let calls = RefCell::new(Vec::new());
        let run = |a: &[String]| -> Result<String, String> {
            calls.borrow_mut().push(a.to_vec());
            Ok(match a[0].as_str() {
                "server" if a[1] == "status" => status.to_string(),
                "ps" => ps.to_string(),
                _ => String::new(),
            })
        };
        ensure_with(s, &run).unwrap();
        calls.into_inner()
    }

    #[test]
    fn ensure_runs_no_start_and_no_load_when_the_model_is_up() {
        let calls = recorded(&lm("http://localhost:1234/v1", "phi", ""), r#"{"running":true}"#, r#"[{"identifier":"phi","modelKey":"phi"}]"#);
        assert_eq!(calls, vec![strs(&["server", "status", "--json"]), strs(&["ps", "--json"])]);
    }

    #[test]
    fn ensure_starts_the_server_and_loads_the_meeting_model_over_the_text_model() {
        let calls = recorded(&lm("http://localhost:1234/v1", "small", "big"), r#"{"running":false}"#, "[]");
        assert_eq!(calls, vec![strs(&["server", "status", "--json"]), strs(&["server", "start"]), strs(&["load", "big", "-y"])]);
    }

    #[test]
    fn ensure_reports_a_failing_step() {
        let run = |a: &[String]| -> Result<String, String> {
            if a[0] == "load" { Err("no such model".into()) } else { Ok(r#"{"running":true}"#.into()) }
        };
        assert!(ensure_with(&lm("http://localhost:1234/v1", "x", ""), &run).unwrap_err().contains("no such model"));
    }
}
