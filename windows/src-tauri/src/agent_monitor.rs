// Phase 5: AI Agent Waiting Gap Monitor (Antigravity, Kiro, OpenCode)
//
// Empirically detects when an AI coding agent is running in:
// 1. Antigravity (`~/.gemini/antigravity/brain/*/.system_generated/logs/transcript.jsonl`)
// 2. Kiro (`~/.config/Kiro/logs/*/window*/exthost/kiro.kiroAgent/Kiro Logs.log`)
// 3. OpenCode (`~/.local/share/opencode/log/opencode.log` + `/proc` process check)
// Plus supports live override/simulation via `~/.cache/coucou/agent-override.json`
// and Tauri commands so all three agents can be verified both empirically and via script.

use crate::db::{AgentWaitRecord, DbState, WeeklyAgentWaitStats};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentLiveStatus {
    pub agent: String,        // "antigravity" | "kiro" | "opencode"
    pub display_name: String, // "Antigravity" | "Kiro" | "OpenCode"
    pub window_class: String, // "antigravity" | "kiro" | "org.kde.konsole"
    pub state: String,        // "idle" | "working" | "done"
    pub detail: String,
    pub started_at: i64,
    pub elapsed_secs: i64,
    pub last_wait_secs: i64,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWaitEventPayload {
    pub agent: String,
    pub display_name: String,
    pub window_class: String,
    pub state: String, // "working" | "done" | "idle"
    pub detail: String,
    pub wait_secs: i64,
    pub timestamp: i64,
    pub weekly_stats: WeeklyAgentWaitStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMonitorSnapshot {
    pub agents: Vec<AgentLiveStatus>,
    pub active_agent: Option<AgentLiveStatus>,
    pub weekly_stats: WeeklyAgentWaitStats,
    pub recent_waits: Vec<AgentWaitRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentOverrideEntry {
    pub state: String, // "working" | "done" | "idle"
    pub detail: String,
    pub updated_at: i64,
}

pub struct AgentMonitorState {
    agents: Mutex<HashMap<String, AgentLiveStatus>>,
    pub dismissed_requests: Mutex<HashMap<String, i64>>,
    pub active_interactive_request: Mutex<Option<(String, String, String)>>,
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn unix_now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/om-raut"))
}

pub fn override_file_path() -> PathBuf {
    let dir = home_dir().join(".cache").join("coucou");
    let _ = fs::create_dir_all(&dir);
    dir.join("agent-override.json")
}

fn read_overrides() -> HashMap<String, AgentOverrideEntry> {
    let path = override_file_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return HashMap::new();
    };
    serde_json::from_str::<HashMap<String, AgentOverrideEntry>>(&raw).unwrap_or_default()
}

pub fn write_agent_override(agent: &str, state: &str, detail: &str) -> Result<PathBuf, String> {
    let path = override_file_path();
    let mut map = read_overrides();
    let clean_agent = agent.trim().to_lowercase();
    if state == "clear" {
        map.remove(&clean_agent);
    } else {
        map.insert(
            clean_agent,
            AgentOverrideEntry {
                state: state.trim().to_lowercase(),
                detail: detail.trim().to_string(),
                updated_at: unix_now(),
            },
        );
    }
    let json = serde_json::to_string_pretty(&map).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

fn file_mtime_secs(path: &Path) -> Option<i64> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    modified
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

fn read_tail_string(path: &Path, max_bytes: u64) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    if len > max_bytes {
        let _ = file.seek(SeekFrom::End(-(max_bytes as i64)));
    }
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// Empirically inspects Antigravity's newest conversation `transcript.jsonl`.
/// Returns `(is_working, detail, source_path)`.
pub fn detect_antigravity_empirical(brain_root: &Path, now: i64) -> (bool, String, String) {
    let Ok(entries) = fs::read_dir(brain_root) else {
        return (false, String::new(), String::new());
    };

    let mut newest_path: Option<PathBuf> = None;
    let mut newest_mtime: i64 = 0;

    for entry in entries.flatten() {
        let candidate = entry
            .path()
            .join(".system_generated")
            .join("logs")
            .join("transcript.jsonl");
        if let Some(mtime) = file_mtime_secs(&candidate) {
            if mtime >= newest_mtime {
                newest_mtime = mtime;
                newest_path = Some(candidate);
            }
        }
    }

    let Some(path) = newest_path else {
        return (false, String::new(), String::new());
    };
    let path_str = path.to_string_lossy().into_owned();

    // If the transcript has not been touched in the last 90 seconds, it is idle.
    if now - newest_mtime > 90 {
        return (false, "Idle".to_string(), path_str);
    }

    let Some(tail) = read_tail_string(&path, 16_384) else {
        return (false, String::new(), path_str);
    };

    let Some(last_line) = tail.lines().rev().find(|l| !l.trim().is_empty()) else {
        return (false, String::new(), path_str);
    };

    let Ok(val) = serde_json::from_str::<serde_json::Value>(last_line) else {
        return (false, String::new(), path_str);
    };

    let step_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let has_tool_calls = val
        .get("tool_calls")
        .and_then(|v| v.as_array())
        .map(|arr| !arr.is_empty())
        .unwrap_or(false);

    if step_type == "USER_INPUT" {
        return (true, "Processing prompt...".to_string(), path_str);
    }

    if step_type == "GENERIC" {
        return (true, "Analyzing tool output...".to_string(), path_str);
    }

    if step_type == "PLANNER_RESPONSE" && has_tool_calls {
        let detail = val
            .get("tool_calls")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.last())
            .and_then(|tc| {
                tc.get("args")
                    .and_then(|a| {
                        a.get("toolAction")
                            .or_else(|| a.get("toolSummary"))
                            .and_then(|s| s.as_str())
                    })
                    .map(|s| s.trim_matches('"').to_string())
                    .or_else(|| tc.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
            })
            .unwrap_or_else(|| "Running tool...".to_string());
        return (true, detail, path_str);
    }

    // A PLANNER_RESPONSE without tool_calls means the agent finished its turn!
    (false, "Turn complete".to_string(), path_str)
}

/// Empirically inspects Kiro's newest `Kiro Logs.log`.
/// Returns `(is_working, detail, source_path)`.
pub fn detect_kiro_empirical(kiro_logs_root: &Path, now: i64) -> (bool, String, String) {
    let Ok(sessions) = fs::read_dir(kiro_logs_root) else {
        return (false, String::new(), String::new());
    };

    let mut newest_log: Option<PathBuf> = None;
    let mut newest_mtime: i64 = 0;

    for sess in sessions.flatten() {
        let sess_path = sess.path();
        if !sess_path.is_dir() {
            continue;
        }
        let Ok(windows) = fs::read_dir(&sess_path) else {
            continue;
        };
        for win in windows.flatten() {
            let candidate = win
                .path()
                .join("exthost")
                .join("kiro.kiroAgent")
                .join("Kiro Logs.log");
            if let Some(mtime) = file_mtime_secs(&candidate) {
                if mtime >= newest_mtime {
                    newest_mtime = mtime;
                    newest_log = Some(candidate);
                }
            }
        }
    }

    let Some(path) = newest_log else {
        return (false, String::new(), String::new());
    };
    let path_str = path.to_string_lossy().into_owned();

    if now - newest_mtime > 90 {
        return (false, "Idle".to_string(), path_str);
    }

    let Some(tail) = read_tail_string(&path, 16_384) else {
        return (false, String::new(), path_str);
    };

    // Walk lines in reverse to find the latest agent-controller queue state
    for line in tail.lines().rev() {
        if line.contains("[agent-controller] Progressing model queue") {
            if line.contains("\"activeExecution\":true") {
                return (true, "Kiro agent executing task".to_string(), path_str);
            }
            if line.contains("\"activeExecution\":false") {
                return (false, "Kiro turn complete".to_string(), path_str);
            }
        }
        if line.contains("[Terminal] execute terminal command done") {
            return (false, "Kiro command finished".to_string(), path_str);
        }
        if line.contains("[Terminal] execute terminal command") {
            return (true, "Kiro running terminal command".to_string(), path_str);
        }
    }

    (false, "Idle".to_string(), path_str)
}

/// Empirically inspects OpenCode's `~/.local/share/opencode/log/opencode.log`.
pub fn detect_opencode_empirical(opencode_log: &Path, now: i64) -> (bool, String, String) {
    let path_str = opencode_log.to_string_lossy().into_owned();
    let Some(mtime) = file_mtime_secs(opencode_log) else {
        return (false, String::new(), path_str);
    };

    if now - mtime > 20 {
        return (false, "Idle".to_string(), path_str);
    }

    let Some(tail) = read_tail_string(opencode_log, 8_192) else {
        return (false, String::new(), path_str);
    };

    let Some(last_line) = tail.lines().rev().find(|l| !l.trim().is_empty()) else {
        return (false, String::new(), path_str);
    };

    if last_line.contains("watcher stopped")
        || last_line.contains("session.idle")
        || last_line.contains("InterruptError")
    {
        return (false, "OpenCode session idle".to_string(), path_str);
    }

    (true, "OpenCode agent active".to_string(), path_str)
}

impl AgentMonitorState {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        map.insert(
            "antigravity".to_string(),
            AgentLiveStatus {
                agent: "antigravity".to_string(),
                display_name: "Antigravity".to_string(),
                window_class: "antigravity".to_string(),
                state: "idle".to_string(),
                detail: "Ready".to_string(),
                started_at: 0,
                elapsed_secs: 0,
                last_wait_secs: 0,
                source_path: String::new(),
            },
        );
        map.insert(
            "kiro".to_string(),
            AgentLiveStatus {
                agent: "kiro".to_string(),
                display_name: "Kiro".to_string(),
                window_class: "kiro".to_string(),
                state: "idle".to_string(),
                detail: "Ready".to_string(),
                started_at: 0,
                elapsed_secs: 0,
                last_wait_secs: 0,
                source_path: String::new(),
            },
        );
        map.insert(
            "opencode".to_string(),
            AgentLiveStatus {
                agent: "opencode".to_string(),
                display_name: "OpenCode".to_string(),
                window_class: "org.kde.konsole".to_string(),
                state: "idle".to_string(),
                detail: "Ready".to_string(),
                started_at: 0,
                elapsed_secs: 0,
                last_wait_secs: 0,
                source_path: String::new(),
            },
        );
        Self {
            agents: Mutex::new(map),
            dismissed_requests: Mutex::new(HashMap::new()),
            active_interactive_request: Mutex::new(None),
        }
    }

    pub fn mark_request_dismissed(&self, request_id: &str) {
        if request_id.is_empty() {
            return;
        }
        let now = unix_now();
        if let Ok(mut map) = self.dismissed_requests.lock() {
            map.retain(|_, ts| now - *ts < 600);
            map.insert(request_id.to_string(), now);
        }
        if let Ok(mut active) = self.active_interactive_request.lock() {
            if active.as_ref().map(|(id, _, _)| id == request_id).unwrap_or(false) {
                *active = None;
            }
        }
    }

    pub fn is_request_dismissed(&self, request_id: &str) -> bool {
        let now = unix_now();
        if let Ok(map) = self.dismissed_requests.lock() {
            if let Some(&ts) = map.get(request_id) {
                return now - ts < 180;
            }
        }
        false
    }

    pub fn snapshot(&self, db: Option<&DbState>) -> AgentMonitorSnapshot {
        let map = self.agents.lock().unwrap();
        let mut list: Vec<AgentLiveStatus> = map.values().cloned().collect();
        list.sort_by(|a, b| a.agent.cmp(&b.agent));

        // Priority for active_agent: working first, then done, else None
        let active_agent = list
            .iter()
            .find(|a| a.state == "working")
            .or_else(|| list.iter().find(|a| a.state == "done"))
            .cloned();

        let weekly_stats = db.map(|d| d.weekly_agent_wait_stats()).unwrap_or_default();
        let recent_waits = db.map(|d| d.list_agent_waits(10)).unwrap_or_default();

        AgentMonitorSnapshot {
            agents: list,
            active_agent,
            weekly_stats,
            recent_waits,
        }
    }

    pub fn update_agent_transition(
        &self,
        agent_key: &str,
        next_working: bool,
        force_done: bool,
        detail: &str,
        source_path: &str,
        db: Option<&DbState>,
    ) -> Option<AgentWaitEventPayload> {
        let now = unix_now();
        let mut map = self.agents.lock().unwrap();
        let entry = map.get_mut(agent_key)?;

        if !source_path.is_empty() {
            entry.source_path = source_path.to_string();
        }

        let prev_state = entry.state.clone();

        if next_working {
            if prev_state != "working" {
                entry.state = "working".to_string();
                entry.started_at = now;
                entry.elapsed_secs = 0;
                entry.detail = if detail.is_empty() {
                    format!("{} agent working...", entry.display_name)
                } else {
                    detail.to_string()
                };
                let weekly_stats = db.map(|d| d.weekly_agent_wait_stats()).unwrap_or_default();
                return Some(AgentWaitEventPayload {
                    agent: entry.agent.clone(),
                    display_name: entry.display_name.clone(),
                    window_class: entry.window_class.clone(),
                    state: "working".to_string(),
                    detail: entry.detail.clone(),
                    wait_secs: 0,
                    timestamp: now,
                    weekly_stats,
                });
            } else {
                entry.elapsed_secs = (now - entry.started_at).max(1);
                if !detail.is_empty() {
                    entry.detail = detail.to_string();
                }
                return None;
            }
        }

        // Transition from working -> done (or explicit force_done)
        if prev_state == "working" || (force_done && prev_state != "done") {
            let wait_secs = if entry.started_at > 0 {
                (now - entry.started_at).max(1)
            } else {
                1
            };
            let started_at = if entry.started_at > 0 {
                entry.started_at
            } else {
                now - wait_secs
            };
            entry.state = "done".to_string();
            entry.elapsed_secs = 0;
            entry.last_wait_secs = wait_secs;
            entry.detail = if detail.is_empty() {
                format!("{} done, review it", entry.display_name)
            } else {
                detail.to_string()
            };

            if let Some(d) = db {
                let _ = d.record_agent_wait(
                    &entry.agent,
                    &entry.window_class,
                    started_at,
                    wait_secs,
                    &entry.detail,
                );
            }
            let weekly_stats = db.map(|d| d.weekly_agent_wait_stats()).unwrap_or_default();

            return Some(AgentWaitEventPayload {
                agent: entry.agent.clone(),
                display_name: entry.display_name.clone(),
                window_class: entry.window_class.clone(),
                state: "done".to_string(),
                detail: entry.detail.clone(),
                wait_secs,
                timestamp: now,
                weekly_stats,
            });
        }

        None
    }

    pub fn acknowledge_done(&self, agent_key: Option<&str>) {
        let mut map = self.agents.lock().unwrap();
        match agent_key {
            Some(k) => {
                if let Some(entry) = map.get_mut(&k.to_lowercase()) {
                    if entry.state == "done" {
                        entry.state = "idle".to_string();
                    }
                }
            }
            None => {
                for entry in map.values_mut() {
                    if entry.state == "done" {
                        entry.state = "idle".to_string();
                    }
                }
            }
        }
        // Also clear any "done" override in agent-override.json
        let mut overrides = read_overrides();
        let mut changed = false;
        for (_, v) in overrides.iter_mut() {
            if v.state == "done" {
                v.state = "idle".to_string();
                changed = true;
            }
        }
        if changed {
            if let Ok(json) = serde_json::to_string_pretty(&overrides) {
                let _ = fs::write(override_file_path(), json);
            }
        }
    }
}

/// Checks if a command string is already executing in `/proc/*/cmdline` so we don't
/// show an approval prompt for an Antigravity command that was already approved and is running.
fn is_command_running_in_proc(cmd_snippet: &str) -> bool {
    let needle = cmd_snippet.trim();
    if needle.is_empty() {
        return false;
    }
    let check_prefix = if needle.len() > 28 { &needle[..28] } else { needle };
    let Ok(entries) = fs::read_dir("/proc") else {
        return false;
    };
    let my_pid = std::process::id().to_string();
    for entry in entries.flatten() {
        let fname = entry.file_name();
        let pid_str = fname.to_string_lossy();
        if !pid_str.chars().all(|c| c.is_ascii_digit()) || pid_str == my_pid {
            continue;
        }
        if let Ok(raw) = fs::read(entry.path().join("cmdline")) {
            let cmd = String::from_utf8_lossy(&raw).replace('\0', " ");
            if cmd.contains(check_prefix) {
                return true;
            }
        }
    }
    false
}

/// Detects if Antigravity's newest conversation `transcript.jsonl` is currently waiting
/// on an interactive question (`ask_question`) or an unsandboxed command approval (`run_command`).
/// Returns `Some((request_id, session_id, agent_id, hook_payload_json))`.
pub fn detect_antigravity_interactive_prompt(
    brain_root: &Path,
    now: i64,
) -> Option<(String, String, String, serde_json::Value)> {
    let entries = fs::read_dir(brain_root).ok()?;
    let mut newest_path: Option<PathBuf> = None;
    let mut newest_conv_id = String::new();
    let mut newest_mtime: i64 = 0;

    for entry in entries.flatten() {
        let conv_dir = entry.path();
        let candidate = conv_dir
            .join(".system_generated")
            .join("logs")
            .join("transcript.jsonl");
        if let Some(mtime) = file_mtime_secs(&candidate) {
            if mtime >= newest_mtime {
                newest_mtime = mtime;
                newest_conv_id = entry.file_name().to_string_lossy().into_owned();
                newest_path = Some(candidate);
            }
        }
    }

    let path = newest_path?;
    if now - newest_mtime > 120 {
        return None;
    }

    let tail = read_tail_string(&path, 24_576)?;
    let last_line = tail.lines().rev().find(|l| !l.trim().is_empty())?;
    let val = serde_json::from_str::<serde_json::Value>(last_line).ok()?;

    let step_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if step_type != "PLANNER_RESPONSE" {
        return None;
    }
    let step_index = val.get("step_index").and_then(|v| v.as_i64()).unwrap_or(0);
    let tool_calls = val.get("tool_calls").and_then(|v| v.as_array())?;
    if tool_calls.is_empty() {
        return None;
    }

    for tc in tool_calls {
        let name = tc.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let Some(args) = tc.get("args").and_then(|v| v.as_object()) else {
            continue;
        };

        // 1. Antigravity ask_question tool call
        if name.ends_with("ask_question") {
            let Some(raw_questions) = args.get("questions").and_then(|v| v.as_array()) else {
                continue;
            };
            let mut normalized = Vec::new();
            for q in raw_questions {
                let q_text = q.get("question").and_then(|v| v.as_str()).unwrap_or("").trim();
                if q_text.is_empty() {
                    continue;
                }
                let multi = q
                    .get("is_multi_select")
                    .or_else(|| q.get("multiSelect"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let opts: Vec<serde_json::Value> = q
                    .get("options")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|o| {
                                if let Some(s) = o.as_str() {
                                    Some(json!({ "label": s, "description": "" }))
                                } else if let Some(lbl) = o.get("label").and_then(|v| v.as_str()) {
                                    let desc = o.get("description").and_then(|v| v.as_str()).unwrap_or("");
                                    Some(json!({ "label": lbl, "description": desc }))
                                } else {
                                    None
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if !opts.is_empty() {
                    normalized.push(json!({
                        "question": q_text,
                        "multiSelect": multi,
                        "options": opts,
                    }));
                }
            }
            if !normalized.is_empty() {
                let req_id = format!("ag-q-{newest_conv_id}-{step_index}");
                let payload = json!({
                    "hook_event_name": "PermissionRequest",
                    "request_id": req_id,
                    "session_id": newest_conv_id,
                    "coucou_agent": "antigravity",
                    "tool_name": "AskUserQuestion",
                    "tool_input": {
                        "questions": normalized,
                        "promptKind": "options",
                        "appLabel": "Antigravity",
                    }
                });
                return Some((req_id, newest_conv_id, "antigravity".to_string(), payload));
            }
        }

        // 2. Antigravity run_command with BypassSandbox: true waiting for approval
        if name.ends_with("run_command") {
            let bypass = args
                .get("BypassSandbox")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !bypass {
                continue;
            }
            let cmd_line = args
                .get("CommandLine")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if cmd_line.is_empty() {
                continue;
            }
            // Wait at least 1s and verify the command isn't already running in /proc
            if now - newest_mtime >= 1 && !is_command_running_in_proc(cmd_line) {
                let summary = args
                    .get("toolSummary")
                    .or_else(|| args.get("toolAction"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Run Command");
                let cwd = args.get("Cwd").and_then(|v| v.as_str()).unwrap_or("");
                let req_id = format!("ag-cmd-{newest_conv_id}-{step_index}");
                let payload = json!({
                    "hook_event_name": "PermissionRequest",
                    "request_id": req_id,
                    "session_id": newest_conv_id,
                    "cwd": cwd,
                    "coucou_agent": "antigravity",
                    "tool_name": summary,
                    "tool_input": {
                        "command": cmd_line,
                        "promptKind": "confirm",
                        "appLabel": "Antigravity",
                    }
                });
                return Some((req_id, newest_conv_id, "antigravity".to_string(), payload));
            }
        }
    }

    None
}

/// Finds the process holding the PTY master (`/dev/ptmx`) for `/dev/pts/<pts_index>`
/// by inspecting `/proc/*/fdinfo/*` for `tty-index: <pts_index>`.
/// Returns `Some((holder_pid, master_fd_path, holder_comm, holder_cmdline))`.
pub fn find_pty_master(pts_index: u32) -> Option<(i32, PathBuf, String, String)> {
    let entries = fs::read_dir("/proc").ok()?;
    let target_tab = format!("tty-index:\t{pts_index}");
    let target_space = format!("tty-index: {pts_index}");

    for entry in entries.flatten() {
        let fname = entry.file_name();
        let pid_str = fname.to_string_lossy();
        let Ok(pid) = pid_str.parse::<i32>() else {
            continue;
        };
        let proc_dir = entry.path();
        let fdinfo_dir = proc_dir.join("fdinfo");
        let Ok(fds) = fs::read_dir(&fdinfo_dir) else {
            continue;
        };
        for fd_entry in fds.flatten() {
            let fd_name = fd_entry.file_name();
            let Ok(info) = fs::read_to_string(fd_entry.path()) else {
                continue;
            };
            if info.lines().any(|l| {
                let t = l.trim();
                t == target_tab || t == target_space
            }) {
                let fd_path = proc_dir.join("fd").join(&fd_name);
                let comm = fs::read_to_string(proc_dir.join("comm"))
                    .unwrap_or_default()
                    .trim()
                    .to_lowercase();
                let cmdline = fs::read(proc_dir.join("cmdline"))
                    .map(|b| String::from_utf8_lossy(&b).replace('\0', " ").to_lowercase())
                    .unwrap_or_default();
                return Some((pid, fd_path, comm, cmdline));
            }
        }
    }
    None
}

/// Writes bytes directly into the PTY master file descriptor for `/dev/pts/<pts_index>`,
/// delivering keystrokes straight to the terminal session (Konsole, VS Code terminal,
/// OpenCode, or normal terminal).
pub fn write_to_pty_master(pts_index: u32, data: &[u8]) -> Result<usize, String> {
    if let Some((_pid, fd_path, _comm, _cmd)) = find_pty_master(pts_index) {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .open(&fd_path)
            .map_err(|e| format!("failed to open PTY master {}: {e}", fd_path.display()))?;
        f.write_all(data)
            .map_err(|e| format!("failed to write to PTY master {}: {e}", fd_path.display()))?;
        let _ = f.flush();
        return Ok(data.len());
    }
    Err(format!("could not locate PTY master for /dev/pts/{pts_index}"))
}

/// Resolves which agent/pill and human-readable label own `/dev/pts/<pts_index>`.
fn classify_pty_owner(pts_index: u32, fg_comm: &str, fg_cmdline: &str) -> (String, String) {
    let fg_lower = format!("{fg_comm} {fg_cmdline}").to_lowercase();
    if fg_lower.contains("opencode") {
        return ("opencode".to_string(), "OpenCode · Terminal".to_string());
    }
    if let Some((holder_pid, _fd_path, holder_comm, holder_cmd)) = find_pty_master(pts_index) {
        let combined = format!("{holder_comm} {holder_cmd}");
        if combined.contains("code") || combined.contains("vscode") {
            return ("vscode".to_string(), "VS Code · Terminal".to_string());
        }
        if combined.contains("konsole") {
            return ("konsole".to_string(), "Konsole".to_string());
        }
        // Check parent of holder_pid (e.g. VS Code's node ptyHost child)
        if let Ok(stat) = fs::read_to_string(format!("/proc/{holder_pid}/stat")) {
            if let Some(rparen) = stat.rfind(')') {
                let fields: Vec<&str> = stat[rparen + 1..].split_whitespace().collect();
                if let Some(ppid_str) = fields.get(1) {
                    if let Ok(ppid) = ppid_str.parse::<i32>() {
                        let pcomm = fs::read_to_string(format!("/proc/{ppid}/comm"))
                            .unwrap_or_default()
                            .to_lowercase();
                        let pcmd = fs::read(format!("/proc/{ppid}/cmdline"))
                            .map(|b| String::from_utf8_lossy(&b).replace('\0', " ").to_lowercase())
                            .unwrap_or_default();
                        if pcomm.contains("code") || pcmd.contains("code") {
                            return ("vscode".to_string(), "VS Code · Terminal".to_string());
                        }
                        if pcomm.contains("konsole") || pcmd.contains("konsole") {
                            return ("konsole".to_string(), "Konsole".to_string());
                        }
                    }
                }
            }
        }
    }
    ("terminal".to_string(), "Terminal".to_string())
}

/// Scans `/proc` for any foreground process on `/dev/pts/<N>` (Konsole, VS Code integrated
/// terminal, OpenCode CLI, or normal terminal) that is waiting for user keyboard input.
pub fn detect_terminal_interactive_prompt(
    seen_waiters: &mut HashMap<(u32, i32), u128>,
    now_ms: u128,
) -> Option<(String, String, String, serde_json::Value)> {
    let entries = fs::read_dir("/proc").ok()?;
    let mut current_waiting: HashMap<(u32, i32), (String, String, String, String)> = HashMap::new();

    const IDLE_SHELLS: &[&str] = &["bash", "zsh", "fish", "sh", "dash", "nu", "tcsh", "csh"];
    const IGNORED_TUIS: &[&str] = &[
        "vim", "nvim", "nano", "vi", "emacs", "less", "more", "man", "top", "htop", "btop",
        "tmux", "screen", "zellij", "watch", "tail", "journalctl", "cat", "coucou", "cargo",
        "rustc", "rust-analyzer", "esbuild", "vite", "node", "bun",
    ];

    for entry in entries.flatten() {
        let fname = entry.file_name();
        let pid_str = fname.to_string_lossy();
        let Ok(pid) = pid_str.parse::<i32>() else {
            continue;
        };
        let proc_dir = entry.path();
        let Ok(stat) = fs::read_to_string(proc_dir.join("stat")) else {
            continue;
        };
        let Some(lparen) = stat.find('(') else {
            continue;
        };
        let Some(rparen) = stat.rfind(')') else {
            continue;
        };
        let comm = stat[lparen + 1..rparen].trim().to_string();
        let comm_lower = comm.to_lowercase();
        let fields: Vec<&str> = stat[rparen + 1..].split_whitespace().collect();
        if fields.len() < 6 {
            continue;
        }
        let state_ch = fields[0];
        if state_ch != "S" {
            continue;
        }
        let pgrp: i32 = fields[2].parse().unwrap_or(-1);
        let session: i32 = fields[3].parse().unwrap_or(-1);
        let tty_nr: i32 = fields[4].parse().unwrap_or(0);
        let tpgid: i32 = fields[5].parse().unwrap_or(-2);

        // Must be a pseudo-terminal slave /dev/pts/<N> (major 136) and in the foreground process group
        if (tty_nr >> 8) != 136 || pgrp <= 0 || pgrp != tpgid {
            continue;
        }
        let pts_index = (tty_nr & 0xff) as u32;

        let cmdline = fs::read(proc_dir.join("cmdline"))
            .map(|b| {
                String::from_utf8_lossy(&b)
                    .split('\0')
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_else(|_| comm.clone());
        let cmd_lower = cmdline.to_lowercase();

        // Skip idle interactive login shells at their normal prompt
        if IDLE_SHELLS.contains(&comm_lower.as_str()) {
            let is_read_prompt = cmd_lower.contains("read ") || cmd_lower.contains("select ");
            if pid == session && !is_read_prompt {
                continue;
            }
            if !is_read_prompt && (cmd_lower == comm_lower || cmd_lower.starts_with('-')) {
                continue;
            }
        }

        // Skip background compilers/servers or full-screen TUIs unless they are explicitly running an interactive prompt script
        if IGNORED_TUIS.contains(&comm_lower.as_str())
            && !cmd_lower.contains("prompt")
            && !cmd_lower.contains("confirm")
            && !cmd_lower.contains("input")
        {
            continue;
        }

        let wchan = fs::read_to_string(proc_dir.join("wchan"))
            .unwrap_or_default()
            .trim()
            .to_string();

        let is_auth_or_pkg = matches!(
            comm_lower.as_str(),
            "sudo" | "su" | "pkexec" | "ssh" | "ssh-keygen" | "gpg" | "apt" | "apt-get" | "dpkg" | "pacman" | "yay" | "dnf"
        );

        let waiting_on_tty = wchan == "n_tty_read"
            || (is_auth_or_pkg
                && matches!(
                    wchan.as_str(),
                    "n_tty_read" | "wait_woken" | "do_select" | "do_sys_poll" | "ep_poll"
                ));

        if !waiting_on_tty {
            continue;
        }

        let cwd = fs::read_link(proc_dir.join("cwd"))
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();

        current_waiting.insert((pts_index, pid), (comm, cmdline, wchan, cwd));
    }

    // Prune waiters that are no longer waiting
    seen_waiters.retain(|k, _| current_waiting.contains_key(k));

    // Record start time for current waiters and pick the first one that has waited >= 400ms
    for (&key, (comm, cmdline, _wchan, cwd)) in &current_waiting {
        let first_seen = *seen_waiters.entry(key).or_insert(now_ms);
        if now_ms.saturating_sub(first_seen) < 400 {
            continue;
        }
        let (pts_index, pid) = key;
        let (agent_id, app_label) = classify_pty_owner(pts_index, comm, cmdline);
        let comm_lower = comm.to_lowercase();
        let cmd_lower = cmdline.to_lowercase();

        let (prompt_kind, tool_label, quick_options) = if comm_lower == "sudo"
            || comm_lower == "su"
            || comm_lower == "pkexec"
            || comm_lower == "ssh"
            || cmd_lower.contains("password")
            || cmd_lower.contains("passphrase")
        {
            (
                "password",
                format!("🔒 Password required ({app_label})"),
                json!([
                    { "label": "Cancel (Ctrl+C)", "value": "__CTRL_C__" }
                ]),
            )
        } else if matches!(
            comm_lower.as_str(),
            "apt" | "apt-get" | "dpkg" | "pacman" | "yay" | "dnf" | "rm" | "cp" | "mv" | "git"
        ) || cmd_lower.contains("[y/n]")
            || cmd_lower.contains("(y/n)")
            || cmd_lower.contains("confirm")
        {
            (
                "confirm",
                format!("{app_label} · Confirm"),
                json!([
                    { "label": "Yes (y)", "value": "y" },
                    { "label": "No (n)", "value": "n" },
                    { "label": "Enter ↵", "value": "__ENTER__" },
                    { "label": "Ctrl+C", "value": "__CTRL_C__" }
                ]),
            )
        } else {
            (
                "input",
                format!("{app_label} · Input / Option"),
                json!([
                    { "label": "Yes (y)", "value": "y" },
                    { "label": "No (n)", "value": "n" },
                    { "label": "1", "value": "1" },
                    { "label": "2", "value": "2" },
                    { "label": "3", "value": "3" },
                    { "label": "Enter ↵", "value": "__ENTER__" },
                    { "label": "Ctrl+C", "value": "__CTRL_C__" }
                ]),
            )
        };

        let req_id = format!("pty-{pts_index}-{pid}");
        let session_id = format!("pts-{pts_index}");
        let display_cmd = if cmdline.trim().is_empty() {
            format!("/dev/pts/{pts_index} ({comm})")
        } else {
            cmdline.trim().to_string()
        };

        let payload = json!({
            "hook_event_name": "PermissionRequest",
            "request_id": req_id,
            "session_id": session_id,
            "cwd": cwd,
            "coucou_agent": agent_id,
            "tool_name": tool_label,
            "tool_input": {
                "command": display_cmd,
                "promptKind": prompt_kind,
                "appLabel": app_label,
                "ptsIndex": pts_index,
                "pid": pid,
                "quickOptions": quick_options,
            }
        });

        return Some((req_id, session_id, agent_id, payload));
    }

    None
}

/// Writes raw keystrokes directly to any currently active foreground terminal PTY (OpenCode, VS Code terminal, Konsole, or shell).
pub fn send_input_to_active_terminal(input_text: &str, append_newline: bool) -> bool {
    let bytes: Vec<u8> = match input_text {
        "__CTRL_C__" => vec![0x03],
        "__ENTER__" => b"\r".to_vec(),
        "allow" => b"y\r".to_vec(),
        "deny" => b"n\r".to_vec(),
        other => {
            if append_newline {
                format!("{other}\r").into_bytes()
            } else {
                other.as_bytes().to_vec()
            }
        }
    };

    let Ok(entries) = fs::read_dir("/proc") else { return false };
    for entry in entries.flatten() {
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else { continue };
        let Some(rparen) = stat.rfind(')') else { continue };
        let fields: Vec<&str> = stat[rparen + 1..].split_whitespace().collect();
        if fields.len() < 6 || fields[0] != "S" { continue };
        let pgrp: i32 = fields[2].parse().unwrap_or(-1);
        let tty_nr: i32 = fields[4].parse().unwrap_or(0);
        let tpgid: i32 = fields[5].parse().unwrap_or(-2);
        if (tty_nr >> 8) == 136 && pgrp > 0 && pgrp == tpgid {
            let pts_index = (tty_nr & 0xff) as u32;
            let cmdline = fs::read(entry.path().join("cmdline"))
                .map(|b| String::from_utf8_lossy(&b).to_lowercase())
                .unwrap_or_default();
            if cmdline.contains("opencode") || cmdline.contains("code") || cmdline.contains("konsole") || cmdline.contains("bash") || cmdline.contains("zsh") {
                if write_to_pty_master(pts_index, &bytes).is_ok() {
                    return true;
                }
            }
        }
    }
    false
}

/// Handles an interactive decision, option pick, or custom text/password submitted from the Coucou island.
/// Returns `true` if handled as an interactive PTY / Antigravity request.
pub fn handle_interactive_input(
    app: &AppHandle,
    request_id: &str,
    input_text: &str,
    append_newline: bool,
) -> bool {
    if let Some(state) = app.try_state::<Arc<AgentMonitorState>>() {
        state.mark_request_dismissed(request_id);
    }

    // 1. Direct Linux PTY terminal input (`pty-<pts_index>-<pid>`)
    if let Some(rest) = request_id.strip_prefix("pty-") {
        let mut parts = rest.split('-');
        if let Some(pts_str) = parts.next() {
            if let Ok(pts_index) = pts_str.parse::<u32>() {
                let bytes: Vec<u8> = match input_text {
                    "__CTRL_C__" => vec![0x03],
                    "__ENTER__" => b"\r".to_vec(),
                    "allow" => b"y\r".to_vec(),
                    "deny" => b"n\r".to_vec(),
                    other => {
                        if append_newline {
                            format!("{other}\r").into_bytes()
                        } else {
                            other.as_bytes().to_vec()
                        }
                    }
                };
                if let Err(err) = write_to_pty_master(pts_index, &bytes) {
                    crate::log::line(format!("pty input error for {request_id}: {err}"));
                } else {
                    crate::log::line(format!("pty input sent to /dev/pts/{pts_index} for {request_id}"));
                }
                return true;
            }
        }
    }

    // 2. Antigravity interactive prompt (`ag-q-...` or `ag-cmd-...`)
    if request_id.starts_with("ag-") {
        let clean = match input_text {
            "__ENTER__" => "",
            "__CTRL_C__" => "",
            other => other,
        };
        if !clean.is_empty() {
            // Copy selected option / text to clipboard so it's immediately ready, and focus Antigravity
            let _ = std::process::Command::new("wl-copy").arg(clean).spawn();
        }
        if let Some(ft) = app.try_state::<Arc<crate::focus_tracker::FocusTrackerState>>() {
            crate::focus_tracker::activate_kwin_window(ft.inner(), "", 0, "antigravity");
        }
        return true;
    }

    false
}

pub fn spawn_agent_monitor(app: AppHandle, state: Arc<AgentMonitorState>) {
    tauri::async_runtime::spawn(async move {
        let home = home_dir();
        let brain_root = home.join(".gemini").join("antigravity").join("brain");
        let kiro_logs_root = home.join(".config").join("Kiro").join("logs");
        let opencode_log = home
            .join(".local")
            .join("share")
            .join("opencode")
            .join("log")
            .join("opencode.log");
        let mut seen_pty_waiters: HashMap<(u32, i32), u128> = HashMap::new();

        loop {
            let now = unix_now();
            let now_ms = unix_now_ms();
            let overrides = read_overrides();
            let db_state = app.try_state::<DbState>();
            let db_ref = db_state.as_ref().map(|s| s.inner());

            // 1. Antigravity
            let (ag_working, ag_force_done, ag_detail, ag_src) =
                if let Some(ov) = overrides.get("antigravity") {
                    if now - ov.updated_at <= 300 {
                        (
                            ov.state == "working",
                            ov.state == "done",
                            ov.detail.clone(),
                            "override".to_string(),
                        )
                    } else {
                        let (w, d, s) = detect_antigravity_empirical(&brain_root, now);
                        (w, false, d, s)
                    }
                } else {
                    let (w, d, s) = detect_antigravity_empirical(&brain_root, now);
                    (w, false, d, s)
                };

            if let Some(evt) = state.update_agent_transition(
                "antigravity",
                ag_working,
                ag_force_done,
                &ag_detail,
                &ag_src,
                db_ref,
            ) {
                let _ = app.emit("agent-wait-event", &evt);
            }

            // 2. Kiro
            let (kiro_working, kiro_force_done, kiro_detail, kiro_src) =
                if let Some(ov) = overrides.get("kiro") {
                    if now - ov.updated_at <= 300 {
                        (
                            ov.state == "working",
                            ov.state == "done",
                            ov.detail.clone(),
                            "override".to_string(),
                        )
                    } else {
                        let (w, d, s) = detect_kiro_empirical(&kiro_logs_root, now);
                        (w, false, d, s)
                    }
                } else {
                    let (w, d, s) = detect_kiro_empirical(&kiro_logs_root, now);
                    (w, false, d, s)
                };

            if let Some(evt) = state.update_agent_transition(
                "kiro",
                kiro_working,
                kiro_force_done,
                &kiro_detail,
                &kiro_src,
                db_ref,
            ) {
                let _ = app.emit("agent-wait-event", &evt);
            }

            // 3. OpenCode
            let (oc_working, oc_force_done, oc_detail, oc_src) =
                if let Some(ov) = overrides.get("opencode") {
                    if now - ov.updated_at <= 300 {
                        (
                            ov.state == "working",
                            ov.state == "done",
                            ov.detail.clone(),
                            "override".to_string(),
                        )
                    } else {
                        let (w, d, s) = detect_opencode_empirical(&opencode_log, now);
                        (w, false, d, s)
                    }
                } else {
                    let (w, d, s) = detect_opencode_empirical(&opencode_log, now);
                    (w, false, d, s)
                };

            if let Some(evt) = state.update_agent_transition(
                "opencode",
                oc_working,
                oc_force_done,
                &oc_detail,
                &oc_src,
                db_ref,
            ) {
                let _ = app.emit("agent-wait-event", &evt);
            }

            // 4. Interactive Input / Option Selection Detector (Antigravity + Terminal / Konsole / VS Code / OpenCode PTY)
            let next_prompt = detect_antigravity_interactive_prompt(&brain_root, now)
                .or_else(|| detect_terminal_interactive_prompt(&mut seen_pty_waiters, now_ms))
                .filter(|(req_id, _, _, _)| !state.is_request_dismissed(req_id));

            let prev_active = state.active_interactive_request.lock().unwrap().clone();
            match (prev_active, next_prompt) {
                (Some((prev_id, prev_sess, prev_agent)), None) => {
                    // The waiting prompt was answered or finished in the terminal/agent: dismiss the card
                    *state.active_interactive_request.lock().unwrap() = None;
                    let dismiss_payload = json!({
                        "hook_event_name": "ApprovalDismiss",
                        "request_id": prev_id,
                        "session_id": prev_sess,
                        "coucou_agent": prev_agent,
                    });
                    let _ = app.emit_to(crate::island::WINDOW_LABEL, "hook", dismiss_payload);
                }
                (prev, Some((req_id, sess_id, agent_id, payload))) => {
                    let is_new = prev
                        .as_ref()
                        .map(|(id, _, _)| id != &req_id)
                        .unwrap_or(true);
                    if is_new {
                        *state.active_interactive_request.lock().unwrap() =
                            Some((req_id, sess_id, agent_id));
                        let _ = app.emit_to(crate::island::WINDOW_LABEL, "hook", payload);
                    }
                }
                (None, None) => {}
            }

            tokio::time::sleep(Duration::from_millis(750)).await;
        }
    });
}

#[tauri::command]
pub fn submit_interactive_input(
    app: AppHandle,
    request_id: String,
    input_text: String,
    append_newline: Option<bool>,
) -> bool {
    let newline = append_newline.unwrap_or(true);
    if handle_interactive_input(&app, &request_id, &input_text, newline) {
        return true;
    }
    // Also try writing directly to active terminal/OpenCode PTY so interactive prompts capture it
    send_input_to_active_terminal(&input_text, newline);

    // If it was a standard hook pipe request (e.g. OpenCode plugin or Claude Code),
    // map "y"/"yes"/"allow" -> "allow", "n"/"no"/"deny" -> "deny", or answer question.
    let lower = input_text.trim().to_lowercase();
    if lower == "y" || lower == "yes" || lower == "allow" {
        crate::pipe::answer(&app, &request_id, "allow");
        return true;
    }
    if lower == "n" || lower == "no" || lower == "deny" || input_text == "__CTRL_C__" {
        crate::pipe::answer(&app, &request_id, "deny");
        return true;
    }
    crate::pipe::answer(&app, &request_id, "allow");
    true
}

#[tauri::command]
pub fn agent_monitor_status(
    state: State<'_, Arc<AgentMonitorState>>,
    db: State<'_, DbState>,
) -> AgentMonitorSnapshot {
    state.snapshot(Some(&db))
}

#[tauri::command]
pub fn agent_monitor_acknowledge(
    state: State<'_, Arc<AgentMonitorState>>,
    agent: Option<String>,
) -> bool {
    state.acknowledge_done(agent.as_deref());
    true
}

#[tauri::command]
pub fn agent_monitor_trigger(
    app: AppHandle,
    state: State<'_, Arc<AgentMonitorState>>,
    db: State<'_, DbState>,
    agent: String,
    target_state: String,
    detail: Option<String>,
) -> Option<AgentWaitEventPayload> {
    let clean_agent = agent.trim().to_lowercase();
    let clean_state = target_state.trim().to_lowercase();
    let clean_detail = detail.unwrap_or_default();
    let _ = write_agent_override(&clean_agent, &clean_state, &clean_detail);
    let evt = state.update_agent_transition(
        &clean_agent,
        clean_state == "working",
        clean_state == "done",
        &clean_detail,
        "manual_trigger",
        Some(&db),
    );
    if let Some(ref payload) = evt {
        let _ = app.emit("agent-wait-event", payload);
    }
    evt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_antigravity_working_and_done_from_transcript_jsonl() {
        let dir = std::env::temp_dir().join(format!("coucou-ag-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let log_dir = dir
            .join("conv-123")
            .join(".system_generated")
            .join("logs");
        fs::create_dir_all(&log_dir).unwrap();
        let transcript = log_dir.join("transcript.jsonl");

        // 1. Write a PLANNER_RESPONSE with tool_calls -> working = true
        fs::write(
            &transcript,
            r#"{"step_index":1,"type":"PLANNER_RESPONSE","status":"DONE","tool_calls":[{"name":"run_command","args":{"toolAction":"Building Phase 5"}}]}"#,
        )
        .unwrap();
        let now = unix_now();
        let (working, detail, _) = detect_antigravity_empirical(&dir, now);
        assert!(working);
        assert_eq!(detail, "Building Phase 5");

        // 2. Write a PLANNER_RESPONSE without tool_calls -> working = false (turn complete)
        fs::write(
            &transcript,
            "{\"step_index\":1,\"type\":\"PLANNER_RESPONSE\",\"status\":\"DONE\",\"tool_calls\":[{\"name\":\"run_command\"}]}\n{\"step_index\":2,\"type\":\"PLANNER_RESPONSE\",\"status\":\"DONE\",\"content\":\"All done!\"}\n",
        )
        .unwrap();
        let (working_after, detail_after, _) = detect_antigravity_empirical(&dir, now);
        assert!(!working_after);
        assert_eq!(detail_after, "Turn complete");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detects_kiro_working_and_done_from_kiro_logs() {
        let dir = std::env::temp_dir().join(format!("coucou-kiro-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let log_dir = dir
            .join("20261011T130445")
            .join("window1")
            .join("exthost")
            .join("kiro.kiroAgent");
        fs::create_dir_all(&log_dir).unwrap();
        let log_file = log_dir.join("Kiro Logs.log");

        // 1. Active execution true -> working
        fs::write(
            &log_file,
            "2026-10-11 19:00:00.000 [info] [agent-controller] Progressing model queue {\"activeExecution\":true,\"queueSize\":1}\n",
        )
        .unwrap();
        let now = unix_now();
        let (working, _, _) = detect_kiro_empirical(&dir, now);
        assert!(working);

        // 2. Active execution false -> done
        fs::write(
            &log_file,
            "2026-10-11 19:00:00.000 [info] [agent-controller] Progressing model queue {\"activeExecution\":true,\"queueSize\":1}\n2026-10-11 19:00:12.000 [info] [agent-controller] Progressing model queue {\"activeExecution\":false,\"queueSize\":0}\n",
        )
        .unwrap();
        let (working_after, _, _) = detect_kiro_empirical(&dir, now);
        assert!(!working_after);

        let _ = fs::remove_dir_all(&dir);
    }
}
