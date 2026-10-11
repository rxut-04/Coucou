// Phase 4: Browser Intent — localhost-only token-protected endpoint (127.0.0.1:49172)
// and editable domain rules classifier (productive, neutral, distracting).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::db::DbState;

pub const ENDPOINT_ADDR: &str = "127.0.0.1:49172";
pub const ENDPOINT_URL: &str = "http://127.0.0.1:49172/api/browser/domain";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainRules {
    pub productive: Vec<String>,
    pub neutral: Vec<String>,
    pub distracting: Vec<String>,
}

impl Default for DomainRules {
    fn default() -> Self {
        Self {
            productive: vec![
                "github.com".into(),
                "gitlab.com".into(),
                "docs.rs".into(),
                "crates.io".into(),
                "rust-lang.org".into(),
                "stackoverflow.com".into(),
                "developer.mozilla.org".into(),
                "tauri.app".into(),
                "chatgpt.com".into(),
                "claude.ai".into(),
                "gemini.google.com".into(),
                "npmjs.com".into(),
                "localhost".into(),
                "127.0.0.1".into(),
            ],
            neutral: vec![
                "google.com".into(),
                "duckduckgo.com".into(),
                "search.brave.com".into(),
                "brave.com".into(),
                "wikipedia.org".into(),
                "news.ycombinator.com".into(),
                "archlinux.org".into(),
                "ubuntu.com".into(),
                "kde.org".into(),
            ],
            distracting: vec![
                "youtube.com".into(),
                "twitter.com".into(),
                "x.com".into(),
                "reddit.com".into(),
                "instagram.com".into(),
                "facebook.com".into(),
                "netflix.com".into(),
                "twitch.tv".into(),
                "tiktok.com".into(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BrowserDomainEvent {
    pub domain: String,
    pub category: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserIntentStatus {
    pub current_domain: String,
    pub category: String,
    pub rules_path: String,
    pub token_path: String,
    pub endpoint: String,
}

pub struct BrowserIntentState {
    pub current_domain: Mutex<String>,
    pub current_category: Mutex<String>,
    pub token: String,
    pub token_path: PathBuf,
    pub rules_path: PathBuf,
}

impl Default for BrowserIntentState {
    fn default() -> Self {
        let config_dir = crate::platform::config_dir();
        let _ = fs::create_dir_all(&config_dir);
        let token_path = config_dir.join("browser-token.txt");
        let rules_path = config_dir.join("domain-rules.json");
        let token = load_or_create_token(&token_path);
        let _ = ensure_domain_rules_file(&rules_path);
        sync_extension_config(&token);

        Self {
            current_domain: Mutex::new(String::new()),
            current_category: Mutex::new("neutral".to_string()),
            token,
            token_path,
            rules_path,
        }
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn load_or_create_token(path: &Path) -> String {
    if let Ok(existing) = fs::read_to_string(path) {
        let trimmed = existing.trim().to_string();
        if trimmed.len() >= 16 {
            return trimmed;
        }
    }
    // Generate a deterministic-format high-entropy local token using /dev/urandom
    let mut bytes = [0u8; 24];
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut bytes);
    } else {
        let now = unix_now() as u64;
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = ((now.wrapping_mul(6364136223846793005).wrapping_add(i as u64)) >> 16) as u8;
        }
    }
    let mut hex = String::with_capacity(48);
    for b in &bytes {
        hex.push_str(&format!("{b:02x}"));
    }
    let token = format!("coucou_tok_{hex}");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if fs::write(path, format!("{token}\n")).is_ok() {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    token
}

fn sync_extension_config(token: &str) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(windows_dir) = manifest_dir.parent() {
        let ext_dir = windows_dir.join("brave-extension");
        if ext_dir.exists() || fs::create_dir_all(&ext_dir).is_ok() {
            let cfg_js = format!(
                "// Auto-synced by Coucou from ~/.config/coucou/browser-token.txt\nexport const COUCOU_ENDPOINT = \"{}\";\nexport const COUCOU_TOKEN = \"{}\";\n",
                ENDPOINT_URL, token
            );
            let _ = fs::write(ext_dir.join("config.js"), cfg_js);
        }
    }
}

pub fn ensure_domain_rules_file(path: &Path) -> DomainRules {
    if let Ok(raw) = fs::read_to_string(path) {
        if let Ok(parsed) = serde_json::from_str::<DomainRules>(&raw) {
            return parsed;
        }
    }
    let default_rules = DomainRules::default();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(pretty) = serde_json::to_string_pretty(&default_rules) {
        let _ = fs::write(path, format!("{pretty}\n"));
        // Also mirror to windows/config/domain-rules.json for convenient repo access
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        if let Some(windows_dir) = manifest_dir.parent() {
            let repo_cfg_dir = windows_dir.join("config");
            if fs::create_dir_all(&repo_cfg_dir).is_ok() {
                let repo_rules = repo_cfg_dir.join("domain-rules.json");
                if !repo_rules.exists() {
                    let _ = fs::write(repo_rules, format!("{pretty}\n"));
                }
            }
        }
    }
    default_rules
}

pub fn load_domain_rules(primary_path: &Path) -> DomainRules {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(windows_dir) = manifest_dir.parent() {
        let repo_rules = windows_dir.join("config/domain-rules.json");
        if let Ok(raw) = fs::read_to_string(&repo_rules) {
            if let Ok(parsed) = serde_json::from_str::<DomainRules>(&raw) {
                return parsed;
            }
        }
    }
    ensure_domain_rules_file(primary_path)
}

/// Sanitizes any input string (domain or accidental full URL) down to ONLY the lowercase hostname.
pub fn extract_clean_domain(raw: &str) -> String {
    let mut s = raw.trim().to_lowercase();
    if s.is_empty() {
        return String::new();
    }
    if let Some(idx) = s.find("://") {
        let scheme = &s[..idx];
        if matches!(scheme, "chrome" | "brave" | "about" | "edge" | "moz-extension" | "chrome-extension" | "devtools") {
            return String::new();
        }
        s = s[idx + 3..].to_string();
    }
    if let Some(slash) = s.find('/') {
        s.truncate(slash);
    }
    if let Some(q) = s.find('?') {
        s.truncate(q);
    }
    if let Some(hash) = s.find('#') {
        s.truncate(hash);
    }
    if let Some(at) = s.rfind('@') {
        s = s[at + 1..].to_string();
    }
    if let Some(colon) = s.find(':') {
        s.truncate(colon);
    }
    let trimmed = s.trim_matches('.').strip_prefix("www.").unwrap_or(s.trim_matches('.'));
    trimmed.to_string()
}

pub fn classify_domain(domain: &str, rules: &DomainRules) -> &'static str {
    let clean = extract_clean_domain(domain);
    if clean.is_empty() {
        return "neutral";
    }
    let matches_rule = |rule: &str| -> bool {
        let r = rule.trim().to_lowercase();
        let r = r.strip_prefix("www.").unwrap_or(&r);
        !r.is_empty() && (clean == r || clean.ends_with(&format!(".{r}")))
    };

    if rules.distracting.iter().any(|r| matches_rule(r)) {
        return "distracting";
    }
    if rules.productive.iter().any(|r| matches_rule(r)) {
        return "productive";
    }
    if rules.neutral.iter().any(|r| matches_rule(r)) {
        return "neutral";
    }
    "neutral"
}

pub fn start(app: AppHandle) {
    let state = app.state::<Arc<BrowserIntentState>>().inner().clone();
    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::bind(ENDPOINT_ADDR).await {
            Ok(l) => l,
            Err(err) => {
                eprintln!("[coucou-browser] could not bind {ENDPOINT_ADDR}: {err}");
                return;
            }
        };
        while let Ok((stream, _peer)) = listener.accept().await {
            let app_clone = app.clone();
            let state_clone = state.clone();
            tauri::async_runtime::spawn(async move {
                let _ = handle_http_connection(stream, app_clone, state_clone).await;
            });
        }
    });
}

async fn handle_http_connection(
    mut stream: TcpStream,
    app: AppHandle,
    state: Arc<BrowserIntentState>,
) -> std::io::Result<()> {
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }
    let req = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req.lines();
    let first_line = lines.next().unwrap_or("");
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    if method == "OPTIONS" {
        let resp = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: POST, GET, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, X-Coucou-Token\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        stream.write_all(resp.as_bytes()).await?;
        return Ok(());
    }

    let mut header_token = String::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("x-coucou-token") {
                header_token = v.trim().to_string();
            }
        }
    }

    let body_str = req
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.trim())
        .or_else(|| req.split_once("\n\n").map(|(_, b)| b.trim()))
        .unwrap_or("");

    let body_json: serde_json::Value = serde_json::from_str(body_str).unwrap_or(serde_json::Value::Null);
    let body_token = body_json
        .get("token")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let provided_token = if !header_token.is_empty() {
        header_token.as_str()
    } else {
        body_token
    };

    if provided_token != state.token {
        let err_body = serde_json::json!({
            "ok": false,
            "error": "Unauthorized: invalid or missing X-Coucou-Token"
        })
        .to_string();
        let resp = format!(
            "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            err_body.len(),
            err_body
        );
        stream.write_all(resp.as_bytes()).await?;
        return Ok(());
    }

    if method == "GET" && path.starts_with("/api/browser/status") {
        let cur_domain = state.current_domain.lock().unwrap().clone();
        let cur_cat = state.current_category.lock().unwrap().clone();
        let status_body = serde_json::json!({
            "ok": true,
            "domain": cur_domain,
            "category": cur_cat,
            "rulesPath": state.rules_path.to_string_lossy(),
            "endpoint": ENDPOINT_URL,
        })
        .to_string();
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status_body.len(),
            status_body
        );
        stream.write_all(resp.as_bytes()).await?;
        return Ok(());
    }

    if method == "POST" && path.starts_with("/api/browser/domain") {
        let raw_domain = body_json
            .get("domain")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let clean_domain = extract_clean_domain(raw_domain);
        if clean_domain.is_empty() {
            let bad = serde_json::json!({ "ok": false, "error": "empty or internal domain" }).to_string();
            let resp = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                bad.len(),
                bad
            );
            stream.write_all(resp.as_bytes()).await?;
            return Ok(());
        }

        let rules = load_domain_rules(&state.rules_path);
        let category = classify_domain(&clean_domain, &rules).to_string();
        {
            *state.current_domain.lock().unwrap() = clean_domain.clone();
            *state.current_category.lock().unwrap() = category.clone();
        }

        if let Some(db) = app.try_state::<DbState>() {
            let _ = db.record_browser_session(&clean_domain, &category, "active_tab", 0);
        }

        let payload = BrowserDomainEvent {
            domain: clean_domain.clone(),
            category: category.clone(),
            timestamp: unix_now(),
        };
        let _ = app.emit("browser-domain", &payload);

        let ok_body = serde_json::json!({
            "ok": true,
            "domain": clean_domain,
            "category": category,
            "rulesPath": state.rules_path.to_string_lossy(),
        })
        .to_string();
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            ok_body.len(),
            ok_body
        );
        stream.write_all(resp.as_bytes()).await?;
        return Ok(());
    }

    let not_found = r#"{"ok":false,"error":"not found"}"#;
    let resp = format!(
        "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        not_found.len(),
        not_found
    );
    stream.write_all(resp.as_bytes()).await?;
    Ok(())
}

#[tauri::command]
pub fn browser_intent_status(state: State<'_, Arc<BrowserIntentState>>) -> BrowserIntentStatus {
    BrowserIntentStatus {
        current_domain: state.current_domain.lock().unwrap().clone(),
        category: state.current_category.lock().unwrap().clone(),
        rules_path: state.rules_path.to_string_lossy().into_owned(),
        token_path: state.token_path.to_string_lossy().into_owned(),
        endpoint: ENDPOINT_URL.to_string(),
    }
}

#[tauri::command]
pub fn browser_intent_record_session(
    db: State<'_, DbState>,
    intent: String,
    duration_secs: i64,
    domain: Option<String>,
    category: Option<String>,
) -> Option<crate::db::BrowserSessionRecord> {
    db.record_browser_session(
        domain.as_deref().unwrap_or("browser"),
        category.as_deref().unwrap_or("neutral"),
        &intent,
        duration_secs,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_only_clean_domain_from_any_input() {
        assert_eq!(
            extract_clean_domain("https://www.github.com/Louis-CFM/coucou?tab=readme#sec"),
            "github.com"
        );
        assert_eq!(
            extract_clean_domain("http://docs.rs:8080/tauri/latest"),
            "docs.rs"
        );
        assert_eq!(extract_clean_domain("youtube.com"), "youtube.com");
        assert_eq!(extract_clean_domain("brave://extensions"), "");
        assert_eq!(extract_clean_domain("chrome://settings"), "");
    }

    #[test]
    fn classifies_domains_into_productive_neutral_and_distracting() {
        let rules = DomainRules::default();
        assert_eq!(classify_domain("github.com", &rules), "productive");
        assert_eq!(classify_domain("gist.github.com", &rules), "productive");
        assert_eq!(classify_domain("docs.rs", &rules), "productive");
        assert_eq!(classify_domain("google.com", &rules), "neutral");
        assert_eq!(classify_domain("en.wikipedia.org", &rules), "neutral");
        assert_eq!(classify_domain("youtube.com", &rules), "distracting");
        assert_eq!(classify_domain("m.youtube.com", &rules), "distracting");
        assert_eq!(classify_domain("reddit.com", &rules), "distracting");
    }
}
