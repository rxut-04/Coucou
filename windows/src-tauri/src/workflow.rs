// Workflow tracking: legacy active-window polling is intentionally removed.
// The Linux/KWin DBus path is in focus_tracker.rs and drives the live window list.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub pid: u32,
    pub class: String,
    pub title: String,
}

pub struct WorkflowTracker {
    current: Arc<Mutex<Option<WindowInfo>>>,
    focused_at: Arc<Mutex<Option<Instant>>>,
    terminal_processes: Arc<Mutex<HashMap<u32, (Instant, String)>>>,
}

impl WorkflowTracker {
    pub fn new() -> Self {
        Self {
            current: Arc::new(Mutex::new(None)),
            focused_at: Arc::new(Mutex::new(None)),
            terminal_processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Legacy xdotool/wmctrl detection is intentionally disabled on Wayland.
    /// The live KWin snapshot is handled in focus_tracker.rs.
    pub fn get_active_window() -> Option<WindowInfo> {
        None
    }

    pub fn update_active_window(&self) -> bool {
        false
    }

    pub fn active_window_class(&self) -> String {
        String::new()
    }

    pub fn active_window_duration(&self) -> u64 {
        0
    }

    pub fn active_window_title(&self) -> String {
        String::new()
    }

    pub fn scan_terminal_processes(&self) -> Vec<(u32, String)> {
        Vec::new()
    }

    pub fn monitor_terminal_process(&self, _pid: u32, _command: String) {}

    pub fn get_monitored_processes(&self) -> Vec<(u32, String)> {
        Vec::new()
    }
}

static WORKFLOW: OnceLock<WorkflowTracker> = OnceLock::new();

pub fn workflow() -> &'static WorkflowTracker {
    WORKFLOW.get_or_init(WorkflowTracker::new)
}

#[tauri::command]
pub fn workflow_active_window() -> Option<serde_json::Value> {
    None
}

#[tauri::command]
pub fn workflow_terminal_finished() -> Vec<(u32, String)> {
    workflow().scan_terminal_processes()
}

#[tauri::command]
pub fn workflow_terminal_processes() -> Vec<(u32, String)> {
    workflow().get_monitored_processes()
}

#[tauri::command]
pub fn workflow_update_active_window() -> bool {
    workflow().update_active_window()
}
