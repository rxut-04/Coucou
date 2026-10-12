// Chat through the locally installed OpenCode CLI (`opencode run --format json`).
//
// Works out of the box without adding an API key in Coucou: OpenCode uses its
// own configured providers and built-in free models (`opencode/*`), and streams
// JSON events over stdout.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::chat::{self, Chat, ChatContext, ChatReply, ModelInfo};
use crate::i18n::t;
use crate::island::WINDOW_LABEL;

pub const DEFAULT_MODEL: &str = "opencode/muse-spark-1.3-contributor-free";
const DEPRECATED_MODELS: &[&str] = &["opencode/exo-free", "exo-free"];

const FALLBACK_MODELS: &[&str] = &[
    "opencode/muse-spark-1.3-contributor-free",
    "opencode/mimo-v2.6-flash-free",
    "opencode/longcat-2.5-preview-free",
    "opencode/ling-3.1-flash-free",
    "opencode/ling-3.0-flash-fin-free",
    "opencode/nemotron-3-ultra-free",
    "opencode/nemotron-3.5-lightning-free",
    "opencode/big-pickle",
    "opencode/space-bunny-free",
    "opencode/step-5-preview-free",
];

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Locates the `opencode` executable even when Coucou is launched from a GUI
/// `.desktop` entry whose `PATH` lacks `~/.opencode/bin`.
pub fn find_binary() -> Option<PathBuf> {
    if let Some(custom) = std::env::var_os("OPENCODE_BIN") {
        let p = PathBuf::from(custom);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(home) = home_dir() {
        let candidates = [
            home.join(".opencode").join("bin").join("opencode"),
            home.join(".local").join("bin").join("opencode"),
            home.join(".bun").join("bin").join("opencode"),
            home.join(".npm-global").join("bin").join("opencode"),
        ];
        for c in candidates {
            if c.is_file() {
                return Some(c);
            }
        }
    }
    for sys in ["/usr/local/bin/opencode", "/usr/bin/opencode"] {
        let p = PathBuf::from(sys);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(if cfg!(windows) { "opencode.exe" } else { "opencode" });
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn is_installed() -> bool {
    find_binary().is_some()
}

fn is_deprecated(model: &str) -> bool {
    let trimmed = model.trim();
    DEPRECATED_MODELS.iter().any(|d| *d == trimmed)
}

/// Reads `~/.local/state/opencode/model.json` for the user's favorite and
/// recently used models in OpenCode.
fn state_models() -> Vec<String> {
    let Some(home) = home_dir() else { return Vec::new() };
    let state_path = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local").join("state"))
        .join("opencode")
        .join("model.json");
    let Ok(bytes) = std::fs::read(state_path) else { return Vec::new() };
    let Ok(val) = serde_json::from_slice::<Value>(&bytes) else { return Vec::new() };

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for key in ["favorite", "recent"] {
        if let Some(arr) = val.get(key).and_then(Value::as_array) {
            for item in arr {
                let provider = item.get("providerID").and_then(Value::as_str).unwrap_or("").trim();
                let model = item.get("modelID").and_then(Value::as_str).unwrap_or("").trim();
                if !provider.is_empty() && !model.is_empty() {
                    let full = format!("{provider}/{model}");
                    if !is_deprecated(&full) && seen.insert(full.clone()) {
                        out.push(full);
                    }
                }
            }
        }
    }
    out
}

/// The best default model for OpenCode: the user's most recent non-deprecated
/// model from `~/.local/state/opencode/model.json`, or `DEFAULT_MODEL`.
pub fn default_model() -> String {
    state_models()
        .into_iter()
        .next()
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

pub fn resolve_model(configured: &str) -> String {
    let m = configured.trim();
    if m.is_empty() || m == "default" || is_deprecated(m) {
        default_model()
    } else {
        m.to_string()
    }
}

fn format_label(id: &str) -> String {
    id.strip_prefix("opencode/").unwrap_or(id).to_string()
}

fn augmented_path(exe: &Path) -> std::ffi::OsString {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(parent) = exe.parent() {
        paths.push(parent.to_path_buf());
    }
    if let Some(home) = home_dir() {
        paths.push(home.join(".opencode").join("bin"));
        paths.push(home.join(".local").join("bin"));
    }
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(paths).unwrap_or_default()
}

/// Lists available models from OpenCode (`~/.local/state/opencode/model.json` +
/// `opencode models` CLI output + fallback free models).
pub async fn models() -> Result<Vec<ModelInfo>, String> {
    let exe = find_binary().ok_or_else(|| {
        "OpenCode is not installed. Install it at ~/.opencode/bin/opencode first.".to_string()
    })?;

    let mut ordered = Vec::<String>::new();
    let mut seen = HashSet::<String>::new();

    for m in state_models() {
        if seen.insert(m.clone()) {
            ordered.push(m);
        }
    }

    let output = tokio::time::timeout(
        Duration::from_secs(8),
        Command::new(&exe)
            .arg("models")
            .env("PATH", augmented_path(&exe))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output(),
    )
    .await;

    if let Ok(Ok(out)) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let m = line.trim();
            if !m.is_empty() && m.contains('/') && !is_deprecated(m) && seen.insert(m.to_string()) {
                ordered.push(m.to_string());
            }
        }
    }

    for &fallback in FALLBACK_MODELS {
        if !is_deprecated(fallback) && seen.insert(fallback.to_string()) {
            ordered.push(fallback.to_string());
        }
    }

    Ok(ordered
        .into_iter()
        .map(|id| ModelInfo {
            label: format_label(&id),
            id,
        })
        .collect())
}

fn scratch_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("coucou-chat");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn build_prompt(
    system: &str,
    history: &[Value],
    first: bool,
    context: Option<&ChatContext>,
    query: &str,
) -> String {
    let mut out = String::new();
    out.push_str("[System instructions: ");
    out.push_str(system);
    out.push_str(
        " Answer the user directly in concise Markdown without invoking file/shell tools unless explicitly asked.]\n\n",
    );

    if !history.is_empty() {
        out.push_str("Conversation so far:\n");
        for turn in history {
            let role = match turn.get("role").and_then(Value::as_str) {
                Some("assistant") => "Assistant",
                _ => "User",
            };
            if let Some(content) = turn.get("content").and_then(Value::as_str) {
                out.push_str(role);
                out.push_str(": ");
                out.push_str(content);
                out.push_str("\n\n");
            }
        }
    }

    let user_part = match context.filter(|_| first) {
        Some(ChatContext::Window { app_name, title, url }) => {
            format!("{}\n\n{query}", chat::window_line(app_name, title, url.as_deref()))
        }
        Some(ChatContext::File { name, .. }) => format!("Attached file: {name}\n\n{query}"),
        None => query.to_string(),
    };

    out.push_str("User: ");
    out.push_str(&user_part);
    out
}

/// Parses one JSON event line emitted by `opencode run --format json`.
fn parse_json_event(line: &str) -> Option<Result<(Option<String>, String), String>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    let val: Value = serde_json::from_str(trimmed).ok()?;
    match val.get("type").and_then(Value::as_str)? {
        "text" => {
            let part = val.get("part")?;
            let text = part.get("text").and_then(Value::as_str)?;
            let id = part.get("id").and_then(Value::as_str).map(str::to_string);
            Some(Ok((id, text.to_string())))
        }
        "error" => {
            let err = val.get("error")?;
            let msg = err
                .get("message")
                .and_then(Value::as_str)
                .or_else(|| err.pointer("/data/message").and_then(Value::as_str))
                .or_else(|| val.get("message").and_then(Value::as_str))
                .unwrap_or("OpenCode returned an error.");
            Some(Err(msg.to_string()))
        }
        _ => None,
    }
}

async fn run_once(
    exe: &Path,
    model: &str,
    prompt: &str,
    attached_file: Option<&str>,
    mut on_delta: impl FnMut(String),
) -> Result<String, String> {
    let mut cmd = Command::new(exe);
    cmd.current_dir(scratch_dir())
        .env("PATH", augmented_path(exe))
        .arg("run")
        .arg("--format")
        .arg("json");

    if !model.trim().is_empty() && model != "default" {
        cmd.arg("-m").arg(model.trim());
    }
    if let Some(file_path) = attached_file {
        cmd.arg("-f").arg(file_path);
    }
    cmd.arg(prompt)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start OpenCode: {e}"))?;

    let stdout = child.stdout.take().ok_or_else(|| "OpenCode stdout unavailable".to_string())?;
    let mut lines = BufReader::new(stdout).lines();

    let mut parts: Vec<(Option<String>, String)> = Vec::new();
    let mut last_error: Option<String> = None;

    while let Ok(Some(line)) = lines.next_line().await {
        match parse_json_event(&line) {
            Some(Ok((part_id, text))) => {
                if let Some(id) = &part_id {
                    if let Some(existing) = parts.iter_mut().find(|(eid, _)| eid.as_ref() == Some(id)) {
                        existing.1 = text;
                    } else {
                        parts.push((part_id, text));
                    }
                } else {
                    parts.push((None, text));
                }
                let combined: String = parts
                    .iter()
                    .map(|(_, t)| t.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                let visible = combined.trim().to_string();
                if !visible.is_empty() {
                    on_delta(visible);
                }
            }
            Some(Err(err_msg)) => {
                last_error = Some(err_msg);
            }
            None => {}
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("OpenCode process failed: {e}"))?;

    let final_text = parts
        .into_iter()
        .map(|(_, t)| t)
        .collect::<Vec<_>>()
        .join("\n\n")
        .trim()
        .to_string();

    if !final_text.is_empty() {
        return Ok(final_text);
    }
    if let Some(err) = last_error {
        return Err(err);
    }
    if !status.success() {
        return Err(format!("OpenCode exited with status {status}."));
    }
    Err(t("No response text."))
}

pub async fn send(
    app: &AppHandle,
    chat: &Chat,
    model: &str,
    query: String,
    context: Option<ChatContext>,
) -> Result<ChatReply, String> {
    let exe = find_binary().ok_or_else(|| {
        t("OpenCode is not installed. Install it at ~/.opencode/bin/opencode first.")
    })?;

    let turn = chat.begin(chat::OPENCODE);
    let system = chat::system_prompt(false);
    let prompt = build_prompt(&system, &turn.history, turn.first, context.as_ref(), &query);
    let attached_file = match context.as_ref().filter(|_| turn.first) {
        Some(ChatContext::File { path, .. }) => Some(path.as_str()),
        _ => None,
    };

    let chosen_model = resolve_model(model);
    let mut answer = run_once(&exe, &chosen_model, &prompt, attached_file, |visible| {
        let _ = app.emit_to(WINDOW_LABEL, "chat-delta", visible);
    })
    .await;

    // If a stale or deprecated model failed, automatically retry once with the
    // default active OpenCode model.
    let fallback = default_model();
    if answer.is_err() && chosen_model != fallback {
        answer = run_once(&exe, &fallback, &prompt, attached_file, |visible| {
            let _ = app.emit_to(WINDOW_LABEL, "chat-delta", visible);
        })
        .await;
    }

    let answer = answer?;
    let plain = chat::plain_question(turn.first, context.as_ref(), &query);
    chat.commit(
        &turn,
        json!({ "role": "user", "content": plain }),
        json!({ "role": "assistant", "content": answer }),
        &plain,
        &answer,
    );
    Ok(ChatReply { text: answer })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_and_error_json_events() {
        let text_line = r#"{"type":"text","timestamp":1,"sessionID":"s","part":{"id":"p1","type":"text","text":"Hello world"}}"#;
        assert_eq!(
            parse_json_event(text_line),
            Some(Ok((Some("p1".to_string()), "Hello world".to_string())))
        );

        let err_line = r#"{"type":"error","timestamp":2,"sessionID":"s","error":{"type":"provider.invalid-request","message":"Model exo-free has been deprecated.","status":410}}"#;
        assert_eq!(
            parse_json_event(err_line),
            Some(Err("Model exo-free has been deprecated.".to_string()))
        );

        let step_line = r#"{"type":"step_start","timestamp":1,"sessionID":"s"}"#;
        assert_eq!(parse_json_event(step_line), None);
    }

    #[test]
    fn deprecated_models_resolve_to_default() {
        let resolved = resolve_model("opencode/exo-free");
        assert_ne!(resolved, "opencode/exo-free");
        assert!(!resolved.is_empty());
    }
}
