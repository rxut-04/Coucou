// Process Tracker - monitors running applications and windows
// Core engine for Coucou OS personal session monitor

use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

/// Represents a detected running application
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetectedApp {
    pub id: String,
    pub name: String,
    pub process_name: String,
    pub window_title: String,
    pub pid: u32,
    pub is_active: bool,        // Currently focused
    pub is_minimized: bool,
    pub monitor_index: i32,
    pub window_rect: WindowRect,
    pub detected_at: u64,
    pub last_seen: u64,
    pub cpu_usage: f64,
    pub memory_mb: f64,
    pub icon_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Application definition in the registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDefinition {
    pub id: String,
    pub display_name: String,
    pub process_names: Vec<String>,     // e.g., ["firefox.exe", "chrome.exe"]
    pub window_class: String,            // X11 window class
    pub icon_path: String,
    pub color: String,                  // Hex color for the island pill
    pub category: AppCategory,
    pub is_enabled: bool,
    pub click_action: ClickAction,
    pub hotkey: Option<String>,
    pub auto_hide: bool,                 // Hide from island when not running
    pub show_in_compact: bool,           // Show in compact mode too
    pub order: i32,                     // Display order
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppCategory {
    Browser,
    Editor,
    Terminal,
    Communication,
    Media,
    System,
    Custom,
}

#[derive(Debug, Clone, Serialize,Deserialize)]
pub enum ClickAction {
    BringToFront,
    CloseWindow,
    MinimizeWindow,
    MaximizeWindow,
    RunCommand(String),
    OpenUrl(String),
}

/// Session statistics for weekly recap
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    pub app_id: String,
    pub total_focus_time_secs: u64,
    pub focus_count: u64,
    pub last_opened: u64,
    pub daily_breakdown: HashMap<String, u64>, // "2026-10-09" -> seconds
}

/// Process tracker engine
pub struct ProcessTracker {
    apps: Arc<Mutex<HashMap<String, DetectedApp>>>,
    registry: Arc<Mutex<HashMap<String, AppDefinition>>>,
    stats: Arc<Mutex<HashMap<String, SessionStats>>>,
    last_poll: Instant,
    poll_interval: Duration,
    change_callbacks: Vec<Box<dyn Fn(Vec<DetectedApp>) + Send>>,
}

impl ProcessTracker {
    pub fn new() -> Self {
        let mut tracker = Self {
            apps: Arc::new(Mutex::new(HashMap::new())),
            registry: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(HashMap::new())),
            last_poll: Instant::now(),
            poll_interval: Duration::from_secs(2),
            change_callbacks: Vec::new(),
        };
        tracker.load_default_registry();
        tracker
    }

    fn load_default_registry(&mut self) {
        let defaults = vec![
            AppDefinition {
                id: "firefox".to_string(),
                display_name: "Firefox".to_string(),
                process_names: vec!["firefox".to_string()],
                window_class: "MozillaWindowClass".to_string(),
                icon_path: "/usr/share/icons/firefox.png".to_string(),
                color: "#FF7139".to_string(),
                category: AppCategory::Browser,
                is_enabled: true,
                click_action: ClickAction::BringToFront,
                hotkey: None,
                auto_hide: true,
                show_in_compact: true,
                order: 0,
            },
            AppDefinition {
                id: "chrome".to_string(),
                display_name: "Chrome".to_string(),
                process_names: vec!["chrome".to_string()],
                window_class: "Chrome_WidgetWin_1".to_string(),
                icon_path: "/usr/share/icons/chrome.png".to_string(),
                color: "#4285F4".to_string(),
                category: AppCategory::Browser,
                is_enabled: true,
                click_action: ClickAction::BringToFront,
                hotkey: None,
                auto_hide: true,
                show_in_compact: true,
                order: 1,
            },
            AppDefinition {
                id: "vscode".to_string(),
                display_name: "VS Code".to_string(),
                process_names: vec!["code".to_string()],
                window_class: "Code".to_string(),
                icon_path: "/usr/share/icons/code.png".to_string(),
                color: "#007ACC".to_string(),
                category: AppCategory::Editor,
                is_enabled: true,
                click_action: ClickAction::BringToFront,
                hotkey: None,
                auto_hide: true,
                show_in_compact: true,
                order: 2,
            },
            AppDefinition {
                id: "terminal".to_string(),
                display_name: "Terminal".to_string(),
                process_names: vec!["konsole".to_string(), "gnome-terminal".to_string(), "xterm".to_string(), "alacritty".to_string(), "kitty".to_string()],

                window_class: "".to_string(),
                icon_path: "/usr/share/icons/terminal.png".to_string(),
                color: "#333333".to_string(),
                category: AppCategory::Terminal,
                is_enabled: true,
                click_action: ClickAction::BringToFront,
                hotkey: None,
                auto_hide: true,
                show_in_compact: true,
                order: 3,
            },
            AppDefinition {
                id: "discord".to_string(),
                display_name: "Discord".to_string(),
                process_names: vec!["discord".to_string(), "Discord".to_string()],
                window_class: "Discord".to_string(),
                icon_path: "/usr/share/icons/discord.png".to_string(),
                color: "#5865F2".to_string(),
                category: AppCategory::Communication,
                is_enabled: true,
                click_action: ClickAction::BringToFront,
                hotkey: None,
                auto_hide: true,
                show_in_compact: true,
                order: 4,
            },
            AppDefinition {
                id: "spotify".to_string(),
                display_name: "Spotify".to_string(),
                process_names: vec!["spotify".to_string()],
                window_class: "Spotify".to_string(),
                icon_path: "/usr/share/icons/spotify.png".to_string(),
                color: "#1DB954".to_string(),
                category: AppCategory::Media,
                is_enabled: true,
                click_action: ClickAction::BringToFront,
                hotkey: None,
                auto_hide: true,
                show_in_compact: true,
                order: 5,
            },
        ];

        for app in defaults {
            tracker.registry.lock().unwrap().insert(app.id.clone(), app);
        }
    }

    /// Poll for process changes - called periodically
    pub fn poll(&mut self) -> Vec<DetectedApp> {
        if self.last_poll.elapsed() < self.poll_interval {
            return self.apps.lock().unwrap().values().cloned().collect();
        }
        self.last_poll = Instant::now();

        let detected = self.scan_processes();
        let mut apps = self.apps.lock().unwrap();
        let previous: HashMap<String, DetectedApp> = apps.clone();
        *apps = detected.clone();

        // Notify callbacks if changes detected
        if apps.len() != previous.len()
            || apps.keys().collect::<std::collections::HashSet<_>>()
                != previous.keys().collect::<std::collections::HashSet<_>>()
        {
            for cb in &self.change_callbacks {
                cb(detected.values().cloned().collect());
            }
        }

        detected.into_values().collect()
    }

    /// Scan running processes using pgrep
    fn scan_processes(&self) -> HashMap<String, DetectedApp> {
        let mut detected = HashMap::new();
        let registry = self.registry.lock().unwrap();

        for (_, def) in registry.iter().filter(|(_, d)| d.is_enabled) {
            for proc_name in &def.process_names {
                if let Ok(output) = Command::new("pgrep")
                    .args(["-f", proc_name])
                    .output()
                {
                    if output.status.success() {
                        let pids = String::from_utf8_lossy(&output.stdout);
                        for pid_str in pids.lines() {
                            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                                let key = format!("{}-{}", def.id, pid);
                                detected.insert(key, DetectedApp {
                                    id: def.id.clone(),
                                    name: def.display_name.clone(),
                                    process_name: proc_name.clone(),
                                    window_title: String::new(),
                                    pid,
                                    is_active: false,
                                    is_minimized: false,
                                    monitor_index: 0,
                                    window_rect: WindowRect { x: 0, y: 0, width: 0, height: 0 },
                                    detected_at: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_millis() as u64,
                                });
                            }
                        }
                    }
                }
            }
        }

        detected
    }

    pub fn register_callback<F: Fn(Vec<DetectedApp>) + Send + 'static>(&mut self, cb: F) {
        self.change_callbacks.push(Box::new(cb));
    }
}
