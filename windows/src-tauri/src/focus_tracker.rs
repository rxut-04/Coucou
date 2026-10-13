// Focus Tracker - monitors active windows with KWin DBus signals and emits events
// to the island. The legacy xdotool / wmctrl polling path is intentionally gone.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use zbus::{connection, interface};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[derive(Default)]
pub struct WindowEntry {
    pub id: String,
    pub resource_class: String,
    pub caption: String,
    pub pid: i32,
    pub active: bool,
    pub minimized: bool,
}

/// Shared state for the focus tracker.
#[derive(Debug, Clone)]
pub struct FocusTrackerState {
    pub last_window_title: Arc<Mutex<String>>,
    pub running: Arc<AtomicBool>,
    pub last_change: Arc<Mutex<Option<std::time::Instant>>>,
    pub windows: Arc<Mutex<Vec<WindowEntry>>>,
    pub app_handle: Arc<Mutex<Option<AppHandle>>>,
}

impl Default for FocusTrackerState {
    fn default() -> Self {
        Self {
            last_window_title: Arc::new(Mutex::new(String::new())),
            running: Arc::new(AtomicBool::new(false)),
            last_change: Arc::new(Mutex::new(None)),
            windows: Arc::new(Mutex::new(Vec::new())),
            app_handle: Arc::new(Mutex::new(None)),
        }
    }
}

impl FocusTrackerState {
    fn emit_focus_changed(&self, title: &str) {
        if let Some(app) = self.app_handle.lock().unwrap().clone() {
            let _ = app.emit("focus-changed", title);
        }
    }

    fn emit_windows_changed(&self) {
        let snapshot = self.windows.lock().unwrap().clone();
        if let Some(app) = self.app_handle.lock().unwrap().clone() {
            let _ = app.emit("windows-changed", &snapshot);
        }
    }

    fn apply_snapshot(&self, snapshot: Vec<WindowEntry>) {
        let active_title = snapshot.iter().find(|w| w.active).map(|w| w.caption.clone());
        *self.windows.lock().unwrap() = snapshot;

        if let Some(title) = active_title {
            let mut last = self.last_window_title.lock().unwrap();
            if *last != title {
                *last = title.clone();
                let mut change_time = self.last_change.lock().unwrap();
                *change_time = Some(std::time::Instant::now());
                self.emit_focus_changed(&title);
            }
        }
        self.emit_windows_changed();
    }
}

#[derive(Clone)]
struct FocusTrackerObject {
    state: Arc<FocusTrackerState>,
}

#[interface(name = "com.coucou.FocusTracker")]
impl FocusTrackerObject {
    fn window_changed(&self, resource_class: &str, caption: &str, pid: &str) {
        let Ok(pid) = pid.trim().parse::<i32>() else {
            eprintln!("WINDOWS invalid WindowChanged pid: {pid}");
            return;
        };
        let active = caption.trim().to_string();
        let mut snapshot = self.state.windows.lock().unwrap().clone();
        let resource = resource_class.trim().to_string();

        let mut saw_match = false;
        for win in &mut snapshot {
            win.active = win.pid == pid && win.resource_class == resource;
            if win.pid == pid && win.resource_class == resource {
                win.caption = active.clone();
                saw_match = true;
            }
        }
        if !saw_match {
            snapshot.push(WindowEntry {
                id: format!("legacy:{pid}:{resource}:{active}"),
                resource_class: resource,
                caption: active.clone(),
                pid,
                active: true,
                minimized: false,
            });
        }

        let mut last = self.state.last_window_title.lock().unwrap();
        if *last != active {
            *last = active.clone();
            let mut change_time = self.state.last_change.lock().unwrap();
            *change_time = Some(std::time::Instant::now());
            drop(change_time);
            self.state.emit_focus_changed(&active);
        }
        drop(last);

        self.state.apply_snapshot(snapshot);
    }

    fn windows_snapshot(&self, payload: &str) {
        let parsed: Vec<WindowEntry> = match serde_json::from_str(payload) {
            Ok(parsed) => parsed,
            Err(err) => {
                let excerpt: String = payload.chars().take(200).collect();
                eprintln!("WINDOWS parse error: {err} payload={excerpt}");
                return;
            }
        };
        eprintln!("WINDOWS count={}", parsed.len());
        let normalized = parsed
            .into_iter()
            .map(|mut w| {
                w.resource_class = w.resource_class.trim().to_string();
                w.caption = w.caption.trim().to_string();
                w
            })
            .collect();
        self.state.apply_snapshot(normalized);
    }

    fn list_windows(&self) -> String {
        let snapshot = self.state.windows.lock().unwrap().clone();
        serde_json::to_string(&snapshot).unwrap_or_default()
    }

    fn activate_window(&self, id: &str, pid: i32, resource_class: &str) -> bool {
        activate_kwin_window(&self.state, id, pid, resource_class)
    }
}

async fn serve_dbus(app: AppHandle, state: Arc<FocusTrackerState>) -> zbus::Result<()> {
    let conn = connection::Builder::session()?
        .name("com.coucou.FocusTracker")?
        .serve_at("/com/coucou/FocusTracker", FocusTrackerObject { state })?
        .build()
        .await?;
    let app_for_install = app.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || install_kwin_script(&app_for_install)).await;
    let _conn = conn;
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

/// Start the focus tracker background task.
pub fn start(app: AppHandle) {
    let state = app.state::<Arc<FocusTrackerState>>().inner().clone();
    {
        let mut handle = state.app_handle.lock().unwrap();
        *handle = Some(app.clone());
    }

    state.running.store(true, Ordering::SeqCst);

    let state_for_dbus = state.clone();
    let app_for_dbus = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = serve_dbus(app_for_dbus, state_for_dbus).await {
            eprintln!("[coucou] focus tracker DBus error: {err}");
        }
    });
}

/// Tauri command to get the current window title.
#[tauri::command]
pub fn focus_tracker_current_title(state: State<'_, Arc<FocusTrackerState>>) -> String {
    state.last_window_title.lock().unwrap().clone()
}

/// Tauri command to check if the tracker is running.
#[tauri::command]
pub fn focus_tracker_is_running(state: State<'_, Arc<FocusTrackerState>>) -> bool {
    state.running.load(Ordering::SeqCst)
}

/// Tauri command to check if currently coding (focused on a code editor).
#[tauri::command]
pub fn focus_tracker_is_coding(state: State<'_, Arc<FocusTrackerState>>) -> bool {
    let title = state.last_window_title.lock().unwrap().clone();
    let coding_terms = ["code", "vscode", "visual studio", "cursor", "jetbrains", "intellij", "terminal", "bash", "zsh", "vim", "emacs"];
    let title_lower = title.to_lowercase();
    coding_terms.iter().any(|term| title_lower.contains(term))
}

/// The focused window as the workflow engine sees it: `name` is the window's
/// resource class (KWin's `resourceClass`, e.g. `code_code`, `org.kde.konsole`).
///
/// `focus_tracker_window_info` returns exactly this, so the island and Rust
/// agree on the contract (workflow.ts used to read a `name` field that was
/// never sent, which threw on every tick).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub pid: i32,
    pub name: String,
    pub title: String,
}

fn active_window_info(windows: &[WindowEntry]) -> WindowInfo {
    match windows.iter().find(|w| w.active) {
        Some(active) => WindowInfo {
            pid: active.pid,
            name: active.resource_class.clone(),
            title: active.caption.clone(),
        },
        None => WindowInfo { pid: 0, name: String::new(), title: String::new() },
    }
}

/// Tauri command to get info about the current window.
#[tauri::command]
pub fn focus_tracker_window_info(
    state: State<'_, Arc<FocusTrackerState>>,
) -> serde_json::Value {
    let title = state.last_window_title.lock().unwrap().clone();
    let info = active_window_info(&state.windows.lock().unwrap());
    let is_coding = crate::classify::classify(&info.name) != crate::classify::AppType::Other;
    serde_json::json!({
        "pid": info.pid,
        "name": info.name,
        "title": if info.title.is_empty() { title } else { info.title },
        "is_coding": is_coding,
        "is_running": state.running.load(Ordering::SeqCst),
    })
}

#[tauri::command]
pub fn focus_tracker_windows(state: State<'_, Arc<FocusTrackerState>>) -> Vec<WindowEntry> {
    state.windows.lock().unwrap().clone()
}

/// Activates and raises a desktop window on Wayland via KWin's Scripting DBus API.
pub fn activate_kwin_window(
    state: &Arc<FocusTrackerState>,
    id: &str,
    pid: i32,
    resource_class: &str,
) -> bool {
    let clean_id = id.trim();
    let clean_class = resource_class.trim().to_lowercase();
    if clean_id.is_empty() && pid <= 0 && clean_class.is_empty() {
        return false;
    }

    let id_json = serde_json::to_string(clean_id).unwrap_or_else(|_| "\"\"".to_string());
    let class_json = serde_json::to_string(&clean_class).unwrap_or_else(|_| "\"\"".to_string());

    let script_content = format!(
        r#"// Coucou one-shot window activation script (Wayland KWin 6)
(function() {{
    var targetId = {id_json};
    var targetPid = {pid};
    var targetClass = {class_json};
    var source = typeof workspace.windowList === "function"
        ? workspace.windowList()
        : (typeof workspace.clientList === "function" ? workspace.clientList() : (workspace.windows || []));
    var list = [];
    if (Array.isArray(source)) {{
        list = source;
    }} else if (source && typeof source.length === "number") {{
        for (var i = 0; i < source.length; i++) list.push(source[i]);
    }}
    var matched = null;
    for (var i = 0; i < list.length; i++) {{
        var w = list[i];
        if (!w) continue;
        var wid = w.internalId !== undefined && w.internalId !== null ? w.internalId.toString() : "";
        if (targetId && wid === targetId) {{
            matched = w;
            break;
        }}
    }}
    if (!matched && targetPid > 0) {{
        for (var j = 0; j < list.length; j++) {{
            var w2 = list[j];
            if (!w2) continue;
            var wpid = Number(w2.pid || 0);
            var wcls = String(w2.resourceClass || w2.windowClass || "").toLowerCase();
            if (wpid === targetPid && (!targetClass || wcls === targetClass || wcls.indexOf(targetClass) !== -1)) {{
                matched = w2;
                break;
            }}
        }}
    }}
    if (!matched && targetClass) {{
        for (var k = 0; k < list.length; k++) {{
            var w3 = list[k];
            if (!w3) continue;
            var cls3 = String(w3.resourceClass || w3.windowClass || "").toLowerCase();
            if (cls3 === targetClass) {{
                matched = w3;
                break;
            }}
        }}
    }}
    if (!matched && targetClass) {{
        for (var m = 0; m < list.length; m++) {{
            var w4 = list[m];
            if (!w4) continue;
            var cls4 = String(w4.resourceClass || w4.windowClass || "").toLowerCase();
            if (cls4.indexOf(targetClass) !== -1) {{
                matched = w4;
                break;
            }}
        }}
    }}
    if (!matched && targetClass) {{
        for (var n = 0; n < list.length; n++) {{
            var w5 = list[n];
            if (!w5) continue;
            var cap5 = String(w5.caption || w5.title || "").toLowerCase();
            if (cap5.indexOf(targetClass) !== -1) {{
                matched = w5;
                break;
            }}
        }}
    }}
    if (matched) {{
        if (matched.minimized) matched.minimized = false;
        if (workspace.activeWindow !== undefined) {{
            workspace.activeWindow = matched;
        }} else if (workspace.activeClient !== undefined) {{
            workspace.activeClient = matched;
        }}
    }}
}})();
"#
    );

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    let script_path = runtime_dir.join("coucou-activate-window.js");
    if std::fs::write(&script_path, script_content).is_err() {
        return false;
    }

    let mut snapshot = state.windows.lock().unwrap().clone();
    let mut changed = false;
    for w in &mut snapshot {
        let is_target = (!clean_id.is_empty() && w.id == clean_id)
            || (clean_id.is_empty() && pid > 0 && w.pid == pid)
            || (clean_id.is_empty()
                && pid <= 0
                && !clean_class.is_empty()
                && w.resource_class.to_lowercase().contains(&clean_class));
        if is_target {
            w.active = true;
            w.minimized = false;
            changed = true;
        } else if w.active {
            w.active = false;
        }
    }
    if changed {
        state.apply_snapshot(snapshot);
    }

    // Run KWin Scripting DBus calls on a detached thread so that when KWin
    // synchronously calls com.coucou.FocusTracker.WindowChanged during `start`,
    // our zbus session loop is never blocked waiting on KWin.
    std::thread::spawn(move || {
        let _ = busctl_call(&[
            "--user", "call", "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting",
            "unloadScript", "s", "coucou-activate-window",
        ]);
        let path_str = script_path.to_string_lossy();
        let loaded = busctl_call(&[
            "--user", "call", "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting",
            "loadScript", "ss", &path_str, "coucou-activate-window",
        ]);
        if loaded.is_none() || loaded.as_deref() == Some("i -1") {
            return;
        }
        let _ = busctl_call(&[
            "--user", "call", "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting",
            "start", "",
        ]);
        std::thread::sleep(Duration::from_millis(120));
        let _ = busctl_call(&[
            "--user", "call", "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting",
            "unloadScript", "s", "coucou-activate-window",
        ]);
    });

    true
}

pub fn close_kwin_window(
    state: &Arc<FocusTrackerState>,
    id: &str,
    pid: i32,
    resource_class: &str,
) -> bool {
    let clean_id = id.trim();
    let clean_class = resource_class.trim().to_lowercase();
    if clean_id.is_empty() && pid <= 0 && clean_class.is_empty() {
        return false;
    }

    let id_json = serde_json::to_string(clean_id).unwrap_or_else(|_| "\"\"".to_string());
    let class_json = serde_json::to_string(&clean_class).unwrap_or_else(|_| "\"\"".to_string());

    let script_content = format!(
        r#"// Coucou one-shot window close script (Wayland KWin 6)
(function() {{
    var targetId = {id_json};
    var targetPid = {pid};
    var targetClass = {class_json};
    var source = typeof workspace.windowList === "function"
        ? workspace.windowList()
        : (typeof workspace.clientList === "function" ? workspace.clientList() : (workspace.windows || []));
    var list = [];
    if (Array.isArray(source)) {{
        list = source;
    }} else if (source && typeof source.length === "number") {{
        for (var i = 0; i < source.length; i++) list.push(source[i]);
    }}
    var matched = null;
    for (var i = 0; i < list.length; i++) {{
        var w = list[i];
        if (!w) continue;
        var wid = w.internalId !== undefined && w.internalId !== null ? w.internalId.toString() : "";
        if (targetId && wid === targetId) {{
            matched = w;
            break;
        }}
    }}
    if (!matched && targetPid > 0) {{
        for (var j = 0; j < list.length; j++) {{
            var w2 = list[j];
            if (!w2) continue;
            var wpid = Number(w2.pid || 0);
            var wcls = String(w2.resourceClass || w2.windowClass || "").toLowerCase();
            if (wpid === targetPid && (!targetClass || wcls === targetClass || wcls.indexOf(targetClass) !== -1)) {{
                matched = w2;
                break;
            }}
        }}
    }}
    if (!matched && targetClass) {{
        for (var k = 0; k < list.length; k++) {{
            var w3 = list[k];
            if (!w3) continue;
            var cls3 = String(w3.resourceClass || w3.windowClass || "").toLowerCase();
            if (cls3 === targetClass || cls3.indexOf(targetClass) !== -1) {{
                matched = w3;
                break;
            }}
        }}
    }}
    if (matched) {{
        if (typeof matched.closeWindow === "function") {{
            matched.closeWindow();
        }}
    }}
}})();
"#
    );

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    let script_path = runtime_dir.join("coucou-close-window.js");
    let _ = std::fs::write(&script_path, script_content);

    let mut snapshot = state.windows.lock().unwrap().clone();
    let before_len = snapshot.len();
    snapshot.retain(|w| {
        let is_target = (!clean_id.is_empty() && w.id == clean_id)
            || (clean_id.is_empty() && pid > 0 && w.pid == pid)
            || (clean_id.is_empty()
                && pid <= 0
                && !clean_class.is_empty()
                && w.resource_class.to_lowercase().contains(&clean_class));
        !is_target
    });
    if snapshot.len() != before_len {
        state.apply_snapshot(snapshot);
    }

    std::thread::spawn(move || {
        let _ = busctl_call(&[
            "--user", "call", "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting",
            "unloadScript", "s", "coucou-close-window",
        ]);
        let path_str = script_path.to_string_lossy();
        let loaded = busctl_call(&[
            "--user", "call", "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting",
            "loadScript", "ss", &path_str, "coucou-close-window",
        ]);
        if loaded.is_some() && loaded.as_deref() != Some("i -1") {
            let _ = busctl_call(&[
                "--user", "call", "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting",
                "start", "",
            ]);
            std::thread::sleep(Duration::from_millis(120));
            let _ = busctl_call(&[
                "--user", "call", "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting",
                "unloadScript", "s", "coucou-close-window",
            ]);
        }
        #[cfg(target_os = "linux")]
        if pid > 0 {
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
        }
    });

    true
}

#[tauri::command]
pub fn focus_tracker_activate_window(
    state: State<'_, Arc<FocusTrackerState>>,
    id: Option<String>,
    pid: Option<i32>,
    resource_class: Option<String>,
) -> bool {
    activate_kwin_window(
        state.inner(),
        id.as_deref().unwrap_or(""),
        pid.unwrap_or(0),
        resource_class.as_deref().unwrap_or(""),
    )
}

#[tauri::command]
pub fn focus_tracker_close_window(
    state: State<'_, Arc<FocusTrackerState>>,
    id: Option<String>,
    pid: Option<i32>,
    resource_class: Option<String>,
) -> bool {
    close_kwin_window(
        state.inner(),
        id.as_deref().unwrap_or(""),
        pid.unwrap_or(0),
        resource_class.as_deref().unwrap_or(""),
    )
}

fn valid_kwin_package(directory: &Path) -> bool {
    let Ok(metadata) = std::fs::read_to_string(directory.join("metadata.json")) else {
        return false;
    };
    let Ok(metadata) = serde_json::from_str::<serde_json::Value>(&metadata) else {
        return false;
    };
    metadata.get("KPackageStructure").and_then(serde_json::Value::as_str) == Some("KWin/Script")
        && metadata.get("X-Plasma-API").and_then(serde_json::Value::as_str) == Some("javascript")
        && metadata.get("X-Plasma-MainScript").and_then(serde_json::Value::as_str) == Some("code/main.js")
}

fn kwin_package_source(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(resources) = app.path().resource_dir() {
        let bundled = resources.join("kwin/coucou-focus");
        if valid_kwin_package(&bundled) {
            return Some(bundled);
        }
    }

    let source_tree = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("kwin/coucou-focus");
    valid_kwin_package(&source_tree).then_some(source_tree)
}

fn busctl_call(arguments: &[&str]) -> Option<String> {
    let output = Command::new("busctl").args(arguments).output().ok()?;
    if !output.status.success() {
        eprintln!("[coucou] busctl {:?} failed: {}", arguments, String::from_utf8_lossy(&output.stderr).trim());
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn kwin_script_loaded() -> bool {
    busctl_call(&[
        "--user", "call", "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting",
        "isScriptLoaded", "s", "coucou-focus",
    ])
    .as_deref() == Some("b true")
}

/// Install and load the bundled KWin script after Coucou owns its DBus name.
pub fn install_kwin_script(app: &AppHandle) -> bool {
    let Some(source) = kwin_package_source(app) else {
        eprintln!("[coucou] bundled KWin script package is missing or has invalid metadata");
        return false;
    };
    let Some(data_dir) = dirs::data_dir() else {
        eprintln!("[coucou] cannot find the KDE user data directory");
        return false;
    };
    let installed = data_dir.join("kwin/scripts/coucou-focus");
    if installed.exists() && !valid_kwin_package(&installed) {
        eprintln!(
            "[coucou] existing KWin package at {} has malformed metadata; leaving it untouched",
            installed.display()
        );
        return false;
    }

    let upgrade = Command::new("kpackagetool6")
        .args(["--type", "KWin/Script", "--upgrade"])
        .arg(&source)
        .status()
        .is_ok_and(|status| status.success());
    if !upgrade {
        let installed_result = Command::new("kpackagetool6")
            .args(["--type", "KWin/Script", "--install"])
            .arg(&source)
            .status()
            .is_ok_and(|status| status.success());
        if !installed_result {
            eprintln!("[coucou] KPackage upgrade/install failed; no files were removed");
            return false;
        }
    }

    let enabled = Command::new("kwriteconfig6")
        .args(["--file", "kwinrc", "--group", "Plugins", "--key", "coucou-focusEnabled", "true"])
        .status()
        .is_ok_and(|status| status.success());
    if !enabled {
        eprintln!("[coucou] could not enable the KWin focus tracker plugin");
        return false;
    }

    if busctl_call(&[
        "--user", "call", "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting",
        "unloadScript", "s", "coucou-focus",
    ]).is_none() {
        return false;
    }

    if busctl_call(&["--user", "call", "org.kde.KWin", "/KWin", "org.kde.KWin", "reconfigure", ""]).is_none() {
        return false;
    }

    if !kwin_script_loaded() {
        let script_path = installed.join("contents/code/main.js");
        let path = script_path.to_string_lossy();
        let Some(result) = busctl_call(&[
            "--user", "call", "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting",
            "loadScript", "ss", &path, "coucou-focus",
        ]) else {
            return false;
        };
        if result == "i -1" {
            eprintln!("[coucou] KWin rejected the Coucou focus script");
            return false;
        }
        if busctl_call(&["--user", "call", "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting", "start", ""]).is_none() {
            return false;
        }
    }

    if !kwin_script_loaded() {
        eprintln!("[coucou] KWin focus script did not load; inspect the kwin_scripting journal");
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{
        active_window_info, FocusTrackerObject, FocusTrackerState, WindowEntry, WindowInfo,
    };
    use std::sync::Arc;

    fn tracker_with_empty_state() -> (FocusTrackerObject, Arc<FocusTrackerState>) {
        let state = Arc::new(FocusTrackerState::default());
        (FocusTrackerObject { state: state.clone() }, state)
    }

    #[test]
    fn accepts_legacy_string_pid_window_changed_calls() {
        let (tracker, state) = tracker_with_empty_state();
        tracker.window_changed("brave", "Browser", "13322");

        let windows = state.windows.lock().unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].pid, 13322);
        assert!(windows[0].active);
    }

    #[test]
    fn accepts_negative_kwin_pid_sentinel_in_snapshot() {
        let (tracker, state) = tracker_with_empty_state();
        tracker.windows_snapshot(
            r#"[{"id":"kwin-special","resourceClass":"plasmashell","caption":"","pid":-1,"active":true,"minimized":false}]"#,
        );

        let windows = state.windows.lock().unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].pid, -1);
    }

    #[test]
    fn does_not_mark_empty_caption_windows_active() {
        let (tracker, state) = tracker_with_empty_state();
        tracker.windows_snapshot(
            r#"[
                {"id":"desktop","resourceClass":"plasmashell","caption":"","pid":-1,"active":false,"minimized":false},
                {"id":"terminal","resourceClass":"org.kde.konsole","caption":"Konsole","pid":42,"active":true,"minimized":false}
            ]"#,
        );

        let windows = state.windows.lock().unwrap();
        assert!(!windows[0].active);
        assert!(windows[1].active);
        assert_eq!(windows.iter().filter(|window| window.active).count(), 1);
    }

    #[test]
    fn parses_camel_case_snapshot_with_unicode_caption() {
        let (tracker, state) = tracker_with_empty_state();
        tracker.windows_snapshot(
            r#"[{"id":"kwin-17","resourceClass":"org.kde.konsole","caption":"Terminal – 日本語","pid":42,"active":true,"minimized":false}]"#,
        );

        let windows = state.windows.lock().unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].id, "kwin-17");
        assert_eq!(windows[0].resource_class, "org.kde.konsole");
        assert_eq!(windows[0].caption, "Terminal – 日本語");
    }

    #[test]
    fn defaults_missing_optional_snapshot_fields() {
        let (tracker, state) = tracker_with_empty_state();
        tracker.windows_snapshot(r#"[{"resourceClass":"org.kde.konsole","caption":"Konsole","pid":42}]"#);

        let windows = state.windows.lock().unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].id, "");
        assert_eq!(windows[0].caption, "Konsole");
        assert!(!windows[0].active);
        assert!(!windows[0].minimized);
    }

    #[test]
    fn preserves_distinct_windows_with_same_pid_and_class() {
        let (tracker, state) = tracker_with_empty_state();
        tracker.windows_snapshot(
            r#"[
                {"id":"kwin-17","resourceClass":"org.example.App","caption":"First","pid":42,"active":true,"minimized":false},
                {"id":"kwin-18","resourceClass":"org.example.App","caption":"Second","pid":42,"active":false,"minimized":false}
            ]"#,
        );

        let windows = state.windows.lock().unwrap();
        assert_eq!(windows.len(), 2);
        assert_ne!(windows[0].id, windows[1].id);
    }

    #[test]
    fn malformed_snapshot_keeps_the_previous_list() {
        let (tracker, state) = tracker_with_empty_state();
        let previous = vec![WindowEntry {
            id: "kwin-17".into(),
            resource_class: "org.kde.konsole".into(),
            caption: "Terminal".into(),
            pid: 42,
            active: true,
            minimized: false,
        }];
        *state.windows.lock().unwrap() = previous.clone();

        tracker.windows_snapshot("[{broken json payload");

        assert_eq!(*state.windows.lock().unwrap(), previous);
    }

    #[test]
    fn serializes_window_entries_with_typescript_field_names() {
        let window = WindowEntry {
            id: "kwin-17".into(),
            resource_class: "org.kde.konsole".into(),
            caption: "Terminal".into(),
            pid: 42,
            active: true,
            minimized: false,
        };
        let serialized = serde_json::to_value(window).unwrap();

        assert_eq!(serialized["id"], "kwin-17");
        assert_eq!(serialized["resourceClass"], "org.kde.konsole");
        assert!(serialized.get("resource_class").is_none());
    }

    fn window(resource_class: &str, caption: &str, pid: i32, active: bool) -> WindowEntry {
        WindowEntry {
            id: format!("kwin-{pid}"),
            resource_class: resource_class.into(),
            caption: caption.into(),
            pid,
            active,
            minimized: false,
        }
    }

    #[test]
    fn window_info_names_the_active_window_by_resource_class() {
        let windows = vec![
            window("org.kde.plasmashell", "Desktop", -1, false),
            window("code_code", "main.rs — coucou", 4711, true),
        ];
        let info = active_window_info(&windows);

        // `name` is the class, not the title — workflow.ts classifies on it.
        assert_eq!(info.name, "code_code");
        assert_eq!(info.title, "main.rs — coucou");
        assert_eq!(info.pid, 4711);
    }

    #[test]
    fn window_info_is_empty_without_an_active_window() {
        let windows = vec![window("org.kde.plasmashell", "Desktop", -1, false)];
        let info = active_window_info(&windows);

        assert_eq!(info, WindowInfo { pid: 0, name: String::new(), title: String::new() });
    }

    #[test]
    fn window_info_serializes_with_the_fields_the_island_reads() {
        let info = active_window_info(&[window("org.kde.konsole", "Konsole", 42, true)]);
        let serialized = serde_json::to_value(&info).unwrap();

        assert_eq!(serialized["pid"], 42);
        assert_eq!(serialized["name"], "org.kde.konsole");
        assert_eq!(serialized["title"], "Konsole");
    }
}
