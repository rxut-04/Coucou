// Battery charging status (/sys/class/power_supply) and MPRIS / browser media
// playback controls (playerctl + wpctl) for the Coucou Dynamic Island.

use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatteryStatus {
    pub has_battery: bool,
    pub charging: bool,
    pub percentage: u8,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaStatus {
    pub active: bool,
    pub playing: bool,
    pub player_name: String,
    pub title: String,
    pub artist: String,
    pub url: String,
    pub art_url: String,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub muted: bool,
}

impl Default for MediaStatus {
    fn default() -> Self {
        Self {
            active: false,
            playing: false,
            player_name: String::new(),
            title: String::new(),
            artist: String::new(),
            url: String::new(),
            art_url: String::new(),
            position_secs: 0.0,
            duration_secs: 0.0,
            muted: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluetoothDeviceStatus {
    pub address: String,
    pub name: String,
    pub icon: String,
    pub battery: u8,
    pub has_battery_info: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BluetoothStatus {
    pub connected: bool,
    pub primary_name: String,
    pub primary_address: String,
    pub primary_icon: String,
    pub primary_battery: u8,
    pub has_battery_info: bool,
    pub devices: Vec<BluetoothDeviceStatus>,
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn fallback_peripheral_battery() -> Option<u8> {
    let base = Path::new("/sys/class/power_supply");
    let entries = fs::read_dir(base).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("BAT")
            || name.starts_with("AC")
            || name.starts_with("ADP")
            || name.starts_with("Mains")
            || name.contains("ucsi")
        {
            continue;
        }
        let path = entry.path();
        let scope = read_trimmed(&path.join("scope")).unwrap_or_default();
        if scope.eq_ignore_ascii_case("Device") || name.contains("hid") || name.contains("blue") {
            if let Some(cap_str) = read_trimmed(&path.join("capacity")) {
                if let Ok(cap) = cap_str.parse::<u16>() {
                    return Some(cap.min(100) as u8);
                }
            }
        }
    }
    None
}

#[tauri::command]
pub fn bluetooth_status_get() -> BluetoothStatus {
    let Ok(out) = Command::new("busctl")
        .args([
            "--json=short",
            "call",
            "--system",
            "org.bluez",
            "/",
            "org.freedesktop.DBus.ObjectManager",
            "GetManagedObjects",
        ])
        .output()
    else {
        return BluetoothStatus::default();
    };

    if !out.status.success() {
        return BluetoothStatus::default();
    }

    let Ok(root) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return BluetoothStatus::default();
    };

    let Some(objects) = root
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|arr| arr.first())
        .and_then(|o| o.as_object())
    else {
        return BluetoothStatus::default();
    };

    let peripheral_bat = fallback_peripheral_battery();
    let mut connected_devices: Vec<BluetoothDeviceStatus> = Vec::new();

    for (obj_path, ifaces_val) in objects {
        if !obj_path.starts_with("/org/bluez/") {
            continue;
        }
        let Some(ifaces) = ifaces_val.as_object() else {
            continue;
        };
        let Some(dev1) = ifaces.get("org.bluez.Device1").and_then(|v| v.as_object()) else {
            continue;
        };

        let is_connected = dev1
            .get("Connected")
            .and_then(|v| v.get("data"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !is_connected {
            continue;
        }

        let address = dev1
            .get("Address")
            .and_then(|v| v.get("data"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let alias = dev1
            .get("Alias")
            .and_then(|v| v.get("data"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let raw_name = dev1
            .get("Name")
            .and_then(|v| v.get("data"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let name = if !alias.trim().is_empty() {
            alias.trim().to_string()
        } else if !raw_name.trim().is_empty() {
            raw_name.trim().to_string()
        } else {
            "Bluetooth Device".to_string()
        };
        let icon = dev1
            .get("Icon")
            .and_then(|v| v.get("data"))
            .and_then(|v| v.as_str())
            .unwrap_or("audio-headset")
            .to_string();

        let bluez_bat = ifaces
            .get("org.bluez.Battery1")
            .and_then(|v| v.as_object())
            .and_then(|b| b.get("Percentage"))
            .and_then(|v| v.get("data"))
            .and_then(|v| v.as_u64())
            .map(|n| n.min(100) as u8);

        let resolved_bat = bluez_bat.or(peripheral_bat);
        connected_devices.push(BluetoothDeviceStatus {
            address,
            name,
            icon,
            battery: resolved_bat.unwrap_or(100),
            has_battery_info: resolved_bat.is_some(),
            connected: true,
        });
    }

    // Prioritize audio headsets / earbuds first
    connected_devices.sort_by_key(|d| {
        let ic = d.icon.to_lowercase();
        if ic.contains("headset") || ic.contains("headphone") || ic.contains("audio") {
            0
        } else {
            1
        }
    });

    if let Some(primary) = connected_devices.first().cloned() {
        BluetoothStatus {
            connected: true,
            primary_name: primary.name,
            primary_address: primary.address,
            primary_icon: primary.icon,
            primary_battery: primary.battery,
            has_battery_info: primary.has_battery_info,
            devices: connected_devices,
        }
    } else {
        BluetoothStatus::default()
    }
}

#[tauri::command]
pub fn battery_status_get() -> BatteryStatus {
    let base = Path::new("/sys/class/power_supply");
    if !base.exists() {
        return BatteryStatus {
            has_battery: false,
            charging: false,
            percentage: 100,
            status: "Unknown".to_string(),
        };
    }

    let mut has_battery = false;
    let mut percentage: u8 = 100;
    let mut status = String::from("Unknown");
    let mut ac_online = false;

    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();
            if name.starts_with("BAT") {
                has_battery = true;
                if let Some(cap_str) = read_trimmed(&path.join("capacity")) {
                    if let Ok(cap) = cap_str.parse::<u16>() {
                        percentage = cap.min(100) as u8;
                    }
                }
                if let Some(st) = read_trimmed(&path.join("status")) {
                    status = st;
                }
            } else if name.starts_with("AC")
                || name.starts_with("ADP")
                || name.starts_with("Mains")
                || name.contains("ucsi")
            {
                if let Some(online_str) = read_trimmed(&path.join("online")) {
                    if online_str == "1" {
                        ac_online = true;
                    }
                }
            }
        }
    }

    let status_lower = status.to_lowercase();
    let charging = status_lower == "charging"
        || (ac_online && status_lower != "discharging");

    BatteryStatus {
        has_battery,
        charging,
        percentage,
        status: if charging && status_lower != "full" {
            "Charging".to_string()
        } else {
            status
        },
    }
}

fn pick_best_player() -> Option<String> {
    let out = Command::new("playerctl").arg("-l").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let players: Vec<String> = text
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if players.is_empty() {
        return None;
    }

    // Prefer a player that is currently Playing, prioritizing browser instances
    for p in &players {
        if let Ok(st) = Command::new("playerctl")
            .args(["-p", p, "status"])
            .output()
        {
            let s = String::from_utf8_lossy(&st.stdout).trim().to_string();
            if s.eq_ignore_ascii_case("Playing") {
                return Some(p.clone());
            }
        }
    }

    // Next, prefer a browser player that is Paused and has a title
    for p in &players {
        let lower = p.to_lowercase();
        if lower.contains("brave")
            || lower.contains("chrom")
            || lower.contains("firefox")
            || lower.contains("edge")
        {
            if let Ok(st) = Command::new("playerctl")
                .args(["-p", p, "status"])
                .output()
            {
                let s = String::from_utf8_lossy(&st.stdout).trim().to_string();
                if s.eq_ignore_ascii_case("Paused") {
                    return Some(p.clone());
                }
            }
        }
    }

    // Fallback to any Paused player
    for p in &players {
        if let Ok(st) = Command::new("playerctl")
            .args(["-p", p, "status"])
            .output()
        {
            let s = String::from_utf8_lossy(&st.stdout).trim().to_string();
            if s.eq_ignore_ascii_case("Paused") {
                return Some(p.clone());
            }
        }
    }

    players.into_iter().next()
}

fn check_sink_muted() -> bool {
    if let Ok(out) = Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            return s.contains("[MUTED]");
        }
    }
    false
}

#[tauri::command]
pub fn media_status_get() -> MediaStatus {
    let Some(player) = pick_best_player() else {
        return MediaStatus::default();
    };

    let format_arg = "{{playerName}}\t{{status}}\t{{xesam:title}}\t{{xesam:artist}}\t{{xesam:url}}\t{{mpris:artUrl}}\t{{position}}\t{{mpris:length}}";
    let Ok(out) = Command::new("playerctl")
        .args(["-p", &player, "metadata", "--format", format_arg])
        .output()
    else {
        return MediaStatus::default();
    };

    if !out.status.success() {
        return MediaStatus::default();
    }

    let raw = String::from_utf8_lossy(&out.stdout);
    let line = raw.lines().next().unwrap_or("").trim_end_matches('\r');
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() < 3 {
        return MediaStatus::default();
    }

    let player_name = parts.first().copied().unwrap_or(&player).to_string();
    let status = parts.get(1).copied().unwrap_or("").trim();
    let title = parts.get(2).copied().unwrap_or("").trim().to_string();
    let artist = parts.get(3).copied().unwrap_or("").trim().to_string();
    let url = parts.get(4).copied().unwrap_or("").trim().to_string();
    let art_url = parts.get(5).copied().unwrap_or("").trim().to_string();
    let pos_us: f64 = parts
        .get(6)
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(0.0);
    let len_us: f64 = parts
        .get(7)
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(0.0);

    if title.is_empty() && !status.eq_ignore_ascii_case("Playing") {
        return MediaStatus::default();
    }

    MediaStatus {
        active: true,
        playing: status.eq_ignore_ascii_case("Playing"),
        player_name,
        title,
        artist,
        url,
        art_url,
        position_secs: (pos_us / 1_000_000.0).max(0.0),
        duration_secs: (len_us / 1_000_000.0).max(0.0),
        muted: check_sink_muted(),
    }
}

#[tauri::command]
pub fn media_control(action: String, value: Option<f64>) -> Result<MediaStatus, String> {
    let player = pick_best_player();
    match action.as_str() {
        "play-pause" => {
            if let Some(ref p) = player {
                let _ = Command::new("playerctl").args(["-p", p, "play-pause"]).status();
            } else {
                let _ = Command::new("playerctl").arg("play-pause").status();
            }
        }
        "next" => {
            if let Some(ref p) = player {
                let _ = Command::new("playerctl").args(["-p", p, "next"]).status();
            } else {
                let _ = Command::new("playerctl").arg("next").status();
            }
        }
        "previous" => {
            if let Some(ref p) = player {
                let _ = Command::new("playerctl").args(["-p", p, "previous"]).status();
            } else {
                let _ = Command::new("playerctl").arg("previous").status();
            }
        }
        "seek-relative" => {
            let delta = value.unwrap_or(10.0);
            let arg = if delta >= 0.0 {
                format!("{}+", delta.abs() as u64)
            } else {
                format!("{}-", delta.abs() as u64)
            };
            if let Some(ref p) = player {
                let _ = Command::new("playerctl")
                    .args(["-p", p, "position", &arg])
                    .status();
            } else {
                let _ = Command::new("playerctl").args(["position", &arg]).status();
            }
        }
        "seek-to" => {
            let pos = value.unwrap_or(0.0).max(0.0);
            let arg = format!("{pos:.1}");
            if let Some(ref p) = player {
                let _ = Command::new("playerctl")
                    .args(["-p", p, "position", &arg])
                    .status();
            } else {
                let _ = Command::new("playerctl").args(["position", &arg]).status();
            }
        }
        "mute" => {
            let ok = Command::new("wpctl")
                .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !ok {
                let _ = Command::new("pactl")
                    .args(["set-sink-mute", "@DEFAULT_SINK@", "toggle"])
                    .status();
            }
        }
        other => return Err(format!("Unsupported media action: {other}")),
    }

    Ok(media_status_get())
}
