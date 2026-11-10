use crate::talking::{self, Chip, Live};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const BASE: &str = "https://api.search.brave.com";
const QUERY_CAP: usize = 400;
const RESULTS: usize = 3;
const CHIP_TEXT: usize = 1_600;

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn plain(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&quot;", "\"").replace("&#x27;", "'").replace("&#39;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

pub fn chip_from_response(query: &str, body: &Value) -> Result<Chip, String> {
    let lines: Vec<String> = body["web"]["results"]
        .as_array()
        .into_iter()
        .flatten()
        .take(RESULTS)
        .map(|r| {
            let (title, snippet) = (plain(r["title"].as_str().unwrap_or("")), plain(r["description"].as_str().unwrap_or("")));
            if snippet.is_empty() { title } else { format!("{title} — {snippet}") }
        })
        .filter(|l| !l.trim().is_empty())
        .collect();
    if lines.is_empty() {
        return Err("no results".into());
    }
    Ok(Chip { query: query.into(), text: lines.join("\n").chars().take(CHIP_TEXT).collect() })
}

fn brave_at(base: &str, key: &str, query: &str) -> Result<Chip, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(10)))
        .max_redirects(0)
        .build()
        .into();
    let resp = agent
        .get(&format!("{base}/res/v1/web/search?q={}&count={RESULTS}", encode(query)))
        .header("Accept", "application/json")
        .header("X-Subscription-Token", key)
        .call()
        .map_err(|_| "couldn't reach Brave".to_string())?;
    match resp.status().as_u16() {
        401 | 403 => return Err("check the Brave key in Models".into()),
        429 => return Err("Brave rate limit, try again shortly".into()),
        s if !(200..300).contains(&s) => return Err(format!("Brave said {s}")),
        _ => {}
    }
    let body = crate::engines::read_json(resp)?;
    chip_from_response(query, &body)
}

pub fn brave(key: &str, query: &str) -> Result<Chip, String> {
    brave_at(BASE, key, query)
}

fn search_with(live: &Arc<Mutex<Live>>, text: &str, fetch: impl Fn(&str) -> Result<Chip, String>) {
    let query: String = text.trim().chars().take(QUERY_CAP).collect();
    let result = fetch(&query);
    let Ok(mut l) = live.lock() else { return };
    match result {
        Ok(chip) => talking::add_chip(&mut l.chips, chip),
        Err(e) => l.status = Some(format!("Search failed: {e}")),
    }
}

pub fn search_into(live: &Arc<Mutex<Live>>, key: &str, text: &str) {
    search_with(live, text, |q| brave(key, q))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::{fake_server, http_reply};
    use serde_json::json;

    fn results() -> Value {
        json!({"web": {"results": [
            {"title": "Rust", "description": "A <strong>fast</strong> language &amp; more"},
            {"title": "Cargo", "description": "The <strong>package</strong> manager"},
            {"title": "Crates", "description": "Registry"},
            {"title": "Fourth", "description": "ignored"},
        ]}})
    }

    #[test]
    fn builds_a_chip_from_the_top_three_results_without_html() {
        let chip = chip_from_response("rust", &results()).unwrap();
        assert_eq!(chip.query, "rust");
        assert_eq!(chip.text, "Rust — A fast language & more\nCargo — The package manager\nCrates — Registry");
    }

    #[test]
    fn a_chip_never_exceeds_the_text_cap() {
        let long = "é".repeat(5_000);
        let chip = chip_from_response("q", &json!({"web": {"results": [{"title": "t", "description": long}]}})).unwrap();
        assert_eq!(chip.text.chars().count(), 1_600);
    }

    #[test]
    fn a_response_with_no_results_is_an_error() {
        assert!(chip_from_response("q", &json!({"web": {"results": []}})).is_err());
        assert!(chip_from_response("q", &json!({})).is_err());
    }

    #[test]
    fn the_query_is_percent_encoded() {
        assert_eq!(encode("a b&c#d?e+f"), "a%20b%26c%23d%3Fe%2Bf");
        assert_eq!(encode("café"), "caf%C3%A9");
        assert_eq!(encode("Az09-._~"), "Az09-._~");
    }

    #[test]
    fn a_search_sends_the_key_and_the_encoded_query() {
        let (port, requests) = fake_server(vec![http_reply("200 OK", "Content-Type: application/json\r\n", &results().to_string())]);
        let chip = brave_at(&format!("http://127.0.0.1:{port}"), "secret", "a b&c").unwrap();
        assert_eq!(chip.query, "a b&c");
        let req = requests.recv().unwrap();
        assert!(req.starts_with("GET /res/v1/web/search?q=a%20b%26c&count=3 HTTP/1.1"), "{req}");
        let lower = req.to_ascii_lowercase();
        assert!(lower.contains("x-subscription-token: secret"), "{req}");
        assert!(lower.contains("accept: application/json"), "{req}");
    }

    #[test]
    fn a_401_is_reported_as_a_bad_key_and_adds_no_chip() {
        let (port, _r) = fake_server(vec![http_reply("401 Unauthorized", "", "{}")]);
        let live = Arc::new(Mutex::new(Live::default()));
        search_with(&live, "rust", |q| brave_at(&format!("http://127.0.0.1:{port}"), "bad", q));
        let l = live.lock().unwrap();
        assert_eq!(l.status.as_deref(), Some("Search failed: check the Brave key in Models"));
        assert!(l.chips.is_empty());
    }

    #[test]
    fn a_429_is_reported_as_a_rate_limit() {
        let (port, _r) = fake_server(vec![http_reply("429 Too Many Requests", "", "{}")]);
        let live = Arc::new(Mutex::new(Live::default()));
        search_with(&live, "rust", |q| brave_at(&format!("http://127.0.0.1:{port}"), "k", q));
        assert_eq!(live.lock().unwrap().status.as_deref(), Some("Search failed: Brave rate limit, try again shortly"));
    }

    #[test]
    fn a_success_adds_a_chip_for_the_trimmed_capped_text_and_nothing_else() {
        let (port, requests) = fake_server(vec![http_reply("200 OK", "", &results().to_string())]);
        let live = Arc::new(Mutex::new(Live::default()));
        search_with(&live, &format!("  {}  ", "x".repeat(500)), |q| brave_at(&format!("http://127.0.0.1:{port}"), "k", q));
        let l = live.lock().unwrap();
        assert_eq!(l.chips.len(), 1);
        assert_eq!(l.chips[0].query, "x".repeat(400));
        assert!(l.status.is_none());
        assert!(requests.recv().unwrap().contains(&format!("q={}&count=3", "x".repeat(400))));
    }

    #[test]
    fn an_unreachable_server_is_a_failure_message_not_a_chip() {
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let live = Arc::new(Mutex::new(Live::default()));
        search_with(&live, "rust", |q| brave_at(&format!("http://127.0.0.1:{port}"), "k", q));
        let l = live.lock().unwrap();
        assert!(l.status.as_deref().unwrap().starts_with("Search failed: "));
        assert!(l.chips.is_empty());
    }
}
