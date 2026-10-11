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
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
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
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
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
        }
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

        loop {
            let now = unix_now();
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

            tokio::time::sleep(Duration::from_millis(1200)).await;
        }
    });
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
