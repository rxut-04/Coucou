// Phase 7: Night & Health Mode
// - Wind-down quotes at night (configurable nightStartHour, default 23:00)
// - Gentle eye (20m), water (45m), and posture (60m) reminders after stretches of real activity
// - Wayland-safe idle detection (org.freedesktop.ScreenSaver GetActive + loginctl seat0 IdleHint/LockedHint + KWin/UI activity)
// - Pauses all timers when idle; enforces cooldown so reminders never stack on top of each other

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::focus_tracker::FocusTrackerState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HealthConfig {
    pub enabled: bool,
    pub night_start_hour: u32,
    pub night_end_hour: u32,
    pub eye_interval_secs: u64,
    pub water_interval_secs: u64,
    pub posture_interval_secs: u64,
    pub idle_threshold_secs: u64,
    pub cooldown_between_reminders_secs: u64,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            night_start_hour: 23,
            night_end_hour: 5,
            eye_interval_secs: 1200,     // 20 minutes (20-20-20 rule)
            water_interval_secs: 2700,   // 45 minutes
            posture_interval_secs: 3600, // 60 minutes
            idle_threshold_secs: 180,    // 3 minutes
            cooldown_between_reminders_secs: 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HealthReminderEvent {
    pub kind: String, // "eye" | "water" | "posture" | "night"
    pub title: String,
    pub message: String,
    pub detail: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatusSnapshot {
    pub config: HealthConfig,
    pub config_path: String,
    pub is_night_mode: bool,
    pub current_local_hour: u32,
    pub is_idle: bool,
    pub idle_secs: u64,
    pub idle_source: String,
    pub active_stretch_secs: u64,
    pub eye_elapsed_secs: u64,
    pub eye_remaining_secs: u64,
    pub water_elapsed_secs: u64,
    pub water_remaining_secs: u64,
    pub posture_elapsed_secs: u64,
    pub posture_remaining_secs: u64,
    pub active_reminder: Option<HealthReminderEvent>,
    pub reminders_fired_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct HealthOverrideFile {
    pub force_night_mode: Option<bool>,
    pub force_idle: Option<bool>,
    pub trigger_reminder: Option<String>,
    pub updated_at: Option<u64>,
}

struct HealthInternal {
    config: HealthConfig,
    last_activity_ts: u64,
    active_stretch_secs: u64,
    eye_elapsed_secs: u64,
    water_elapsed_secs: u64,
    posture_elapsed_secs: u64,
    last_reminder_ts: u64,
    active_reminder: Option<HealthReminderEvent>,
    reminders_fired_count: u64,
    force_night_mode: Option<bool>,
    force_idle: Option<bool>,
    idle_source: String,
    last_seen_override_ts: u64,
}

pub struct HealthModeState {
    config_path: PathBuf,
    override_path: PathBuf,
    inner: Mutex<HealthInternal>,
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn resolve_config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let trimmed = xdg.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("coucou");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join(".config").join("coucou");
        }
    }
    PathBuf::from("/tmp/coucou-config")
}

fn resolve_cache_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        let trimmed = xdg.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("coucou");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join(".cache").join("coucou");
        }
    }
    PathBuf::from("/tmp/coucou-cache")
}

/// Returns current local hour (0..23) using `date +%H` or UTC fallback.
pub fn current_local_hour() -> u32 {
    if let Ok(out) = Command::new("date").arg("+%H").output() {
        if out.status.success() {
            if let Ok(s) = String::from_utf8(out.stdout) {
                if let Ok(h) = s.trim().parse::<u32>() {
                    if h < 24 {
                        return h;
                    }
                }
            }
        }
    }
    ((unix_now() / 3600) % 24) as u32
}

pub fn is_hour_in_night_window(hour: u32, start_hour: u32, end_hour: u32) -> bool {
    if start_hour == end_hour {
        return false;
    }
    if start_hour > end_hour {
        // Crosses midnight (e.g., 23:00 -> 05:00)
        hour >= start_hour || hour < end_hour
    } else {
        hour >= start_hour && hour < end_hour
    }
}

/// Empirical Wayland idle & screen lock check on KDE Plasma 6:
/// 1. `busctl --user call org.freedesktop.ScreenSaver /ScreenSaver org.freedesktop.ScreenSaver GetActive`
/// 2. `loginctl show-session <seat0> -p IdleHint -p LockedHint`
pub fn probe_wayland_system_idle() -> (bool, String) {
    if let Ok(out) = Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.freedesktop.ScreenSaver",
            "/ScreenSaver",
            "org.freedesktop.ScreenSaver",
            "GetActive",
        ])
        .output()
    {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().ends_with("true") {
                return (true, "org.freedesktop.ScreenSaver.GetActive=true".to_string());
            }
        }
    }

    if let Ok(list_out) = Command::new("loginctl")
        .args(["list-sessions", "--no-legend"])
        .output()
    {
        if list_out.status.success() {
            let text = String::from_utf8_lossy(&list_out.stdout);
            for line in text.lines() {
                if line.contains("seat0") {
                    if let Some(session_id) = line.split_whitespace().next() {
                        if let Ok(show_out) = Command::new("loginctl")
                            .args([
                                "show-session",
                                session_id,
                                "-p",
                                "IdleHint",
                                "-p",
                                "LockedHint",
                            ])
                            .output()
                        {
                            if show_out.status.success() {
                                let props = String::from_utf8_lossy(&show_out.stdout);
                                if props.contains("LockedHint=yes") {
                                    return (
                                        true,
                                        format!("loginctl session {session_id} LockedHint=yes"),
                                    );
                                }
                                if props.contains("IdleHint=yes") {
                                    return (
                                        true,
                                        format!("loginctl session {session_id} IdleHint=yes"),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    (false, "wayland-active".to_string())
}

pub fn build_reminder_event(kind: &str, active_stretch_secs: u64) -> HealthReminderEvent {
    let mins = std::cmp::max(1, active_stretch_secs / 60);
    let now = unix_now();
    match kind {
        "eye" => HealthReminderEvent {
            kind: "eye".to_string(),
            title: "👀 20-20-20 Eye Care".to_string(),
            message: "Look 20 feet away for 20 seconds to relax your eyes 👀".to_string(),
            detail: format!("Active stretch: {mins}m · Blink slowly & soften your gaze"),
            timestamp: now,
        },
        "water" => HealthReminderEvent {
            kind: "water".to_string(),
            title: "💧 Hydration Check".to_string(),
            message: "Take a sip of water — hydrated brains debug 2x faster 💧".to_string(),
            detail: format!("Active stretch: {mins}m · Keep water within reach"),
            timestamp: now,
        },
        "posture" => HealthReminderEvent {
            kind: "posture".to_string(),
            title: "🧘 Posture & Shoulders".to_string(),
            message: "Unclench your jaw, drop your shoulders, and sit tall 🧘".to_string(),
            detail: format!("Active stretch: {mins}m · Roll your shoulders back for 10s"),
            timestamp: now,
        },
        _ => HealthReminderEvent {
            kind: "night".to_string(),
            title: "🌙 Night Wind-Down".to_string(),
            message: "🌙 It's past wind-down hour — commit a clean checkpoint and rest well.".to_string(),
            detail: "Tomorrow's fresh mind solves in 10 minutes what midnight takes 2 hours to debug".to_string(),
            timestamp: now,
        },
    }
}

impl HealthModeState {
    pub fn new() -> Self {
        let config_dir = resolve_config_dir();
        let _ = fs::create_dir_all(&config_dir);
        let config_path = config_dir.join("health-config.json");

        if !config_path.exists() {
            let def = HealthConfig::default();
            if let Ok(pretty) = serde_json::to_string_pretty(&def) {
                let _ = fs::write(&config_path, pretty);
            }
        }

        let cache_dir = resolve_cache_dir();
        let _ = fs::create_dir_all(&cache_dir);
        let override_path = cache_dir.join("health-override.json");

        Self::with_paths(config_path, override_path)
    }

    pub fn with_paths(config_path: PathBuf, override_path: PathBuf) -> Self {
        let config = if let Ok(raw) = fs::read_to_string(&config_path) {
            serde_json::from_str::<HealthConfig>(&raw).unwrap_or_default()
        } else {
            HealthConfig::default()
        };

        let now = unix_now();
        Self {
            config_path,
            override_path,
            inner: Mutex::new(HealthInternal {
                config,
                last_activity_ts: now,
                active_stretch_secs: 0,
                eye_elapsed_secs: 0,
                water_elapsed_secs: 0,
                posture_elapsed_secs: 0,
                last_reminder_ts: 0,
                active_reminder: None,
                reminders_fired_count: 0,
                force_night_mode: None,
                force_idle: None,
                idle_source: "wayland-active".to_string(),
                last_seen_override_ts: 0,
            }),
        }
    }

    pub fn reload_config_if_present(&self) {
        if let Ok(raw) = fs::read_to_string(&self.config_path) {
            if let Ok(parsed) = serde_json::from_str::<HealthConfig>(&raw) {
                if let Ok(mut guard) = self.inner.lock() {
                    guard.config = parsed;
                }
            }
        }
    }

    pub fn record_activity(&self) {
        let now = unix_now();
        if let Ok(mut guard) = self.inner.lock() {
            guard.last_activity_ts = now;
            if guard.force_idle != Some(true) {
                guard.idle_source = "user-activity".to_string();
            }
        }
    }

    pub fn acknowledge_reminder(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.active_reminder = None;
        }
    }

    pub fn trigger_kind(&self, kind: &str) -> Option<HealthReminderEvent> {
        let now = unix_now();
        let mut guard = self.inner.lock().ok()?;
        match kind {
            "idle" => {
                guard.force_idle = Some(true);
                guard.idle_source = "manual-idle-override".to_string();
                None
            }
            "active" => {
                guard.force_idle = Some(false);
                guard.last_activity_ts = now;
                guard.idle_source = "manual-active-override".to_string();
                None
            }
            "night" => {
                guard.force_night_mode = Some(true);
                let evt = build_reminder_event("night", guard.active_stretch_secs);
                guard.active_reminder = Some(evt.clone());
                guard.last_reminder_ts = now;
                guard.reminders_fired_count += 1;
                Some(evt)
            }
            "day" => {
                guard.force_night_mode = Some(false);
                if guard.active_reminder.as_ref().map(|r| r.kind.as_str()) == Some("night") {
                    guard.active_reminder = None;
                }
                None
            }
            "eye" | "water" | "posture" => {
                // Fast-forward elapsed counter and fire cleanly without stacking:
                // any previous reminder is replaced (never stacked).
                match kind {
                    "eye" => guard.eye_elapsed_secs = 0,
                    "water" => guard.water_elapsed_secs = 0,
                    "posture" => guard.posture_elapsed_secs = 0,
                    _ => {}
                }
                let stretch = std::cmp::max(guard.active_stretch_secs, 1200);
                let evt = build_reminder_event(kind, stretch);
                guard.active_reminder = Some(evt.clone());
                guard.last_reminder_ts = now;
                guard.reminders_fired_count += 1;
                Some(evt)
            }
            _ => None,
        }
    }

    /// Runs every 2 seconds in the background daemon (or during unit/CLI tests).
    /// Returns a `HealthReminderEvent` only when a single non-stacking reminder fires.
    pub fn tick(&self, delta_secs: u64, kwin_recently_active: bool) -> Option<HealthReminderEvent> {
        self.reload_config_if_present();

        // Check override file for CLI fast-forward / demo triggers
        if let Ok(raw) = fs::read_to_string(&self.override_path) {
            if let Ok(ovr) = serde_json::from_str::<HealthOverrideFile>(&raw) {
                let ts = ovr.updated_at.unwrap_or(0);
                let should_apply = self
                    .inner
                    .lock()
                    .map(|g| ts > g.last_seen_override_ts)
                    .unwrap_or(false);
                if should_apply {
                    if let Ok(mut g) = self.inner.lock() {
                        g.last_seen_override_ts = ts;
                        if let Some(fnm) = ovr.force_night_mode {
                            g.force_night_mode = Some(fnm);
                        }
                        if let Some(fi) = ovr.force_idle {
                            g.force_idle = Some(fi);
                            if !fi {
                                g.last_activity_ts = unix_now();
                            }
                        }
                    }
                    if let Some(kind) = ovr.trigger_reminder.as_deref() {
                        if let Some(evt) = self.trigger_kind(kind) {
                            return Some(evt);
                        }
                    }
                }
            }
        }

        let (sys_idle, sys_source) = probe_wayland_system_idle();
        let now = unix_now();

        let mut guard = self.inner.lock().ok()?;
        if !guard.config.enabled {
            return None;
        }

        if kwin_recently_active && guard.force_idle != Some(true) {
            guard.last_activity_ts = now;
        }

        let inactivity_secs = now.saturating_sub(guard.last_activity_ts);
        let is_idle = match guard.force_idle {
            Some(true) => {
                guard.idle_source = "override-idle".to_string();
                true
            }
            Some(false) => {
                guard.idle_source = "override-active".to_string();
                false
            }
            None => {
                if sys_idle {
                    guard.idle_source = sys_source;
                    true
                } else if inactivity_secs >= guard.config.idle_threshold_secs {
                    guard.idle_source = format!("inactivity-{}s", inactivity_secs);
                    true
                } else {
                    guard.idle_source = sys_source;
                    false
                }
            }
        };

        // When idle on Wayland, pause all timers!
        if is_idle {
            return None;
        }

        guard.active_stretch_secs += delta_secs;
        guard.eye_elapsed_secs += delta_secs;
        guard.water_elapsed_secs += delta_secs;
        guard.posture_elapsed_secs += delta_secs;

        // Enforce cooldown so reminders NEVER stack on top of each other
        let since_last_reminder = now.saturating_sub(guard.last_reminder_ts);
        if guard.last_reminder_ts > 0
            && since_last_reminder < guard.config.cooldown_between_reminders_secs
        {
            return None;
        }

        // Priority order: posture (60m) -> water (45m) -> eye (20m)
        // Only ONE reminder can fire on a tick; any due counter resets cleanly.
        let fired_kind = if guard.posture_elapsed_secs >= guard.config.posture_interval_secs {
            guard.posture_elapsed_secs = 0;
            Some("posture")
        } else if guard.water_elapsed_secs >= guard.config.water_interval_secs {
            guard.water_elapsed_secs = 0;
            Some("water")
        } else if guard.eye_elapsed_secs >= guard.config.eye_interval_secs {
            guard.eye_elapsed_secs = 0;
            Some("eye")
        } else {
            None
        };

        if let Some(kind) = fired_kind {
            let evt = build_reminder_event(kind, guard.active_stretch_secs);
            guard.active_reminder = Some(evt.clone());
            guard.last_reminder_ts = now;
            guard.reminders_fired_count += 1;
            return Some(evt);
        }

        None
    }

    pub fn snapshot(&self) -> HealthStatusSnapshot {
        self.reload_config_if_present();
        let now = unix_now();
        let hour = current_local_hour();
        let guard = self.inner.lock().unwrap();

        let is_night_mode = guard.force_night_mode.unwrap_or_else(|| {
            is_hour_in_night_window(hour, guard.config.night_start_hour, guard.config.night_end_hour)
        });

        let inactivity_secs = now.saturating_sub(guard.last_activity_ts);
        let is_idle = match guard.force_idle {
            Some(v) => v,
            None => {
                guard.idle_source.contains("GetActive=true")
                    || guard.idle_source.contains("Hint=yes")
                    || inactivity_secs >= guard.config.idle_threshold_secs
            }
        };

        HealthStatusSnapshot {
            config: guard.config.clone(),
            config_path: self.config_path.to_string_lossy().to_string(),
            is_night_mode,
            current_local_hour: hour,
            is_idle,
            idle_secs: if is_idle {
                std::cmp::max(inactivity_secs, guard.config.idle_threshold_secs)
            } else {
                inactivity_secs
            },
            idle_source: guard.idle_source.clone(),
            active_stretch_secs: guard.active_stretch_secs,
            eye_elapsed_secs: guard.eye_elapsed_secs,
            eye_remaining_secs: guard
                .config
                .eye_interval_secs
                .saturating_sub(guard.eye_elapsed_secs),
            water_elapsed_secs: guard.water_elapsed_secs,
            water_remaining_secs: guard
                .config
                .water_interval_secs
                .saturating_sub(guard.water_elapsed_secs),
            posture_elapsed_secs: guard.posture_elapsed_secs,
            posture_remaining_secs: guard
                .config
                .posture_interval_secs
                .saturating_sub(guard.posture_elapsed_secs),
            active_reminder: guard.active_reminder.clone(),
            reminders_fired_count: guard.reminders_fired_count,
        }
    }

    pub fn write_override_trigger(&self, kind: &str) -> Option<HealthReminderEvent> {
        let now = unix_now();
        let mut ovr = if let Ok(raw) = fs::read_to_string(&self.override_path) {
            serde_json::from_str::<HealthOverrideFile>(&raw).unwrap_or_default()
        } else {
            HealthOverrideFile::default()
        };

        match kind {
            "night" => {
                ovr.force_night_mode = Some(true);
                ovr.trigger_reminder = Some("night".to_string());
            }
            "day" => {
                ovr.force_night_mode = Some(false);
                ovr.trigger_reminder = None;
            }
            "idle" => {
                ovr.force_idle = Some(true);
                ovr.trigger_reminder = None;
            }
            "active" => {
                ovr.force_idle = Some(false);
                ovr.trigger_reminder = None;
            }
            other => {
                ovr.trigger_reminder = Some(other.to_string());
            }
        }
        ovr.updated_at = Some(now);
        if let Ok(pretty) = serde_json::to_string_pretty(&ovr) {
            let _ = fs::write(&self.override_path, pretty);
        }
        self.trigger_kind(kind)
    }
}

pub fn start(app: AppHandle) {
    let state = app.state::<Arc<HealthModeState>>().inner().clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(2));
        let kwin_recent = app
            .try_state::<Arc<FocusTrackerState>>()
            .and_then(|ft| {
                ft.last_change
                    .lock()
                    .ok()
                    .and_then(|opt| opt.map(|inst| inst.elapsed() < Duration::from_secs(10)))
            })
            .unwrap_or(false);

        if let Some(evt) = state.tick(2, kwin_recent) {
            let _ = app.emit("health-reminder-event", &evt);
        }
    });
}

#[tauri::command]
pub fn health_mode_status(state: State<'_, Arc<HealthModeState>>) -> HealthStatusSnapshot {
    state.snapshot()
}

#[tauri::command]
pub fn health_mode_trigger(
    app: AppHandle,
    state: State<'_, Arc<HealthModeState>>,
    kind: String,
) -> HealthStatusSnapshot {
    if let Some(evt) = state.write_override_trigger(kind.trim()) {
        let _ = app.emit("health-reminder-event", &evt);
    }
    state.snapshot()
}

#[tauri::command]
pub fn health_mode_activity_ping(state: State<'_, Arc<HealthModeState>>) -> bool {
    state.record_activity();
    true
}

#[tauri::command]
pub fn health_mode_acknowledge(state: State<'_, Arc<HealthModeState>>) -> bool {
    state.acknowledge_reminder();
    true
}

pub fn handle_cli() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    let idx = args
        .iter()
        .position(|a| a == "--health-status" || a == "--health-fast-forward")?;

    let state = HealthModeState::new();
    match args[idx].as_str() {
        "--health-status" => {
            let _ = state.tick(0, false);
            let snap = state.snapshot();
            println!(
                "{}",
                serde_json::to_string_pretty(&snap).unwrap_or_else(|_| "{}".to_string())
            );
            Some(0)
        }
        "--health-fast-forward" => {
            let kind = args.get(idx + 1).map(|s| s.as_str()).unwrap_or("eye");
            let evt = state.write_override_trigger(kind);
            let snap = state.snapshot();
            let out = serde_json::json!({
                "triggered": kind,
                "firedEvent": evt,
                "status": snap,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".to_string())
            );
            Some(0)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn night_window_handles_midnight_wrap_and_daytime() {
        assert!(is_hour_in_night_window(23, 23, 5));
        assert!(is_hour_in_night_window(0, 23, 5));
        assert!(is_hour_in_night_window(4, 23, 5));
        assert!(!is_hour_in_night_window(5, 23, 5));
        assert!(!is_hour_in_night_window(14, 23, 5));
    }

    #[test]
    fn idle_pauses_timers_and_cooldown_prevents_stacking() {
        let dir = std::env::temp_dir().join(format!("coucou-health-test-{}", std::process::id()));
        let state = HealthModeState::with_paths(
            dir.join("nonexistent-config.json"),
            dir.join("nonexistent-override.json"),
        );
        {
            let mut g = state.inner.lock().unwrap();
            g.config.eye_interval_secs = 10;
            g.config.water_interval_secs = 10;
            g.config.posture_interval_secs = 10;
            g.config.cooldown_between_reminders_secs = 60;
            g.force_idle = Some(true);
        }

        // While idle, ticking advances 0 timers and fires nothing
        assert!(state.tick(20, false).is_none());
        let snap_idle = state.snapshot();
        assert!(snap_idle.is_idle);
        assert_eq!(snap_idle.active_stretch_secs, 0);
        assert_eq!(snap_idle.eye_elapsed_secs, 0);

        // Resume active: tick 10s -> exactly ONE reminder fires (posture has highest priority when all hit 10s), never stacked
        state.trigger_kind("active");
        let first = state.tick(10, true).expect("should fire one reminder");
        assert_eq!(first.kind, "posture");

        // Immediately ticking again within the 60s cooldown must NOT stack a second reminder
        assert!(state.tick(2, true).is_none());
        assert_eq!(state.snapshot().reminders_fired_count, 1);
    }
}
