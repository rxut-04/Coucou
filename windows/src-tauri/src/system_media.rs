// Battery charging status (/sys/class/power_supply) and MPRIS / browser media
// playback controls (playerctl + wpctl) for the Coucou Dynamic Island.

use serde::{Deserialize, Serialize};
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

// ── Live Weather & 4-Day Forecast (Open-Meteo + Geo-IP with instant fallback) ──

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherForecastDay {
    pub day_short: String,
    pub day_full: String,
    pub date_label: String,
    pub temp_c: i32,
    pub condition: String,
    pub icon_kind: String,
    pub uv_index: u8,
    pub humidity: u8,
    pub wind_ms: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherStatus {
    pub location: String,
    pub day_full: String,
    pub date_label: String,
    pub temp_c: i32,
    pub condition: String,
    pub icon_kind: String,
    pub uv_index: u8,
    pub humidity: u8,
    pub wind_ms: f32,
    pub forecast: Vec<WeatherForecastDay>,
}

fn wmo_code_to_condition(code: i64) -> (&'static str, &'static str) {
    match code {
        0 => ("Clear", "sun"),
        1 | 2 => ("Partly Cloudy", "sun_rain"),
        3 => ("Clouds", "clouds"),
        45 | 48 => ("Misty Clouds", "clouds"),
        51..=67 | 80..=82 => ("Rain Showers", "sun_rain"),
        71..=77 | 85 | 86 => ("Snow", "clouds"),
        95..=99 => ("Thunderstorm", "storm"),
        _ => ("Clouds", "sun_rain"),
    }
}

fn day_labels_for_offset(offset_days: i64) -> (String, String, String) {
    // Use `date` command for accurate local timezone day/month names on Linux
    let arg = format!("+{offset_days} days");
    if let Ok(out) = Command::new("date")
        .args(["-d", &arg, "+%a\t%A\t%-d, %b"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            let parts: Vec<&str> = s.trim().split('\t').collect();
            if parts.len() == 3 {
                return (
                    parts[0].to_string(),
                    parts[1].to_string(),
                    parts[2].to_string(),
                );
            }
        }
    }
    let fallbacks = [
        ("Thu", "Thursday", "15, Sep"),
        ("Fri", "Friday", "16, Sep"),
        ("Sat", "Saturday", "17, Sep"),
        ("Sun", "Sunday", "18, Sep"),
    ];
    let idx = (offset_days.rem_euclid(4)) as usize;
    (
        fallbacks[idx].0.to_string(),
        fallbacks[idx].1.to_string(),
        fallbacks[idx].2.to_string(),
    )
}

fn default_weather_status() -> WeatherStatus {
    let (d0_short, d0_full, d0_date) = day_labels_for_offset(0);
    let (d1_short, d1_full, d1_date) = day_labels_for_offset(1);
    let (d2_short, d2_full, d2_date) = day_labels_for_offset(2);
    let (d3_short, d3_full, d3_date) = day_labels_for_offset(3);

    WeatherStatus {
        location: "Pune, India".to_string(),
        day_full: d0_full.clone(),
        date_label: d0_date.clone(),
        temp_c: 34,
        condition: "Clouds".to_string(),
        icon_kind: "clouds".to_string(),
        uv_index: 0,
        humidity: 59,
        wind_ms: 2.0,
        forecast: vec![
            WeatherForecastDay {
                day_short: d0_short,
                day_full: d0_full,
                date_label: d0_date,
                temp_c: 32,
                condition: "Clouds".to_string(),
                icon_kind: "sun_rain".to_string(),
                uv_index: 0,
                humidity: 59,
                wind_ms: 2.0,
            },
            WeatherForecastDay {
                day_short: d1_short,
                day_full: d1_full,
                date_label: d1_date,
                temp_c: 27,
                condition: "Rain Showers".to_string(),
                icon_kind: "sun_rain".to_string(),
                uv_index: 2,
                humidity: 68,
                wind_ms: 2.8,
            },
            WeatherForecastDay {
                day_short: d2_short,
                day_full: d2_full,
                date_label: d2_date,
                temp_c: 27,
                condition: "Partly Cloudy".to_string(),
                icon_kind: "sun_rain".to_string(),
                uv_index: 3,
                humidity: 64,
                wind_ms: 2.4,
            },
            WeatherForecastDay {
                day_short: d3_short,
                day_full: d3_full,
                date_label: d3_date,
                temp_c: 29,
                condition: "Clouds".to_string(),
                icon_kind: "clouds".to_string(),
                uv_index: 1,
                humidity: 56,
                wind_ms: 1.9,
            },
        ],
    }
}

static WEATHER_CACHE: std::sync::OnceLock<
    std::sync::Mutex<(Option<std::time::Instant>, WeatherStatus)>,
> = std::sync::OnceLock::new();

#[tauri::command]
pub async fn weather_status_get() -> WeatherStatus {
    let cache = WEATHER_CACHE.get_or_init(|| {
        std::sync::Mutex::new((None, default_weather_status()))
    });

    {
        let guard = cache.lock().unwrap();
        if let Some(ts) = guard.0 {
            if ts.elapsed().as_secs() < 600 {
                return guard.1.clone();
            }
        }
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(2600))
        .build()
    {
        Ok(c) => c,
        Err(_) => return cache.lock().unwrap().1.clone(),
    };

    // 1. Geo-IP lookup for city + coordinates (fallback to Pune, India)
    let mut city = String::from("Pune");
    let mut lat = 18.5204_f64;
    let mut lon = 73.8567_f64;

    if let Ok(resp) = client.get("https://ipwho.is/").send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if json.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                if let Some(c) = json.get("city").and_then(|v| v.as_str()) {
                    if !c.trim().is_empty() {
                        city = c.trim().to_string();
                    }
                }
                if let Some(la) = json.get("latitude").and_then(|v| v.as_f64()) {
                    lat = la;
                }
                if let Some(lo) = json.get("longitude").and_then(|v| v.as_f64()) {
                    lon = lo;
                }
            }
        }
    }

    // 2. Fetch current weather + 4-day forecast from Open-Meteo
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}&current=temperature_2m,relative_humidity_2m,weather_code,wind_speed_10m,uv_index&daily=weather_code,temperature_2m_max,uv_index_max,wind_speed_10m_max&wind_speed_unit=ms&timezone=auto&forecast_days=4"
    );

    if let Ok(resp) = client.get(&url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(curr) = json.get("current") {
                let temp_c = curr
                    .get("temperature_2m")
                    .and_then(|v| v.as_f64())
                    .map(|t| t.round() as i32)
                    .unwrap_or(32);
                let humidity = curr
                    .get("relative_humidity_2m")
                    .and_then(|v| v.as_u64())
                    .map(|h| h.min(100) as u8)
                    .unwrap_or(59);
                let wind_ms = curr
                    .get("wind_speed_10m")
                    .and_then(|v| v.as_f64())
                    .map(|w| (w * 10.0).round() as f32 / 10.0)
                    .unwrap_or(2.0);
                let uv_index = curr
                    .get("uv_index")
                    .and_then(|v| v.as_f64())
                    .map(|u| u.round().max(0.0) as u8)
                    .unwrap_or(0);
                let code = curr
                    .get("weather_code")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(3);
                let (cond_str, _) = wmo_code_to_condition(code);

                let daily = json.get("daily");
                let max_temps = daily
                    .and_then(|d| d.get("temperature_2m_max"))
                    .and_then(|v| v.as_array());
                let codes = daily
                    .and_then(|d| d.get("weather_code"))
                    .and_then(|v| v.as_array());
                let uvs = daily
                    .and_then(|d| d.get("uv_index_max"))
                    .and_then(|v| v.as_array());
                let winds = daily
                    .and_then(|d| d.get("wind_speed_10m_max"))
                    .and_then(|v| v.as_array());

                let mut forecast = Vec::with_capacity(4);
                for i in 0..4 {
                    let (day_short, day_full, date_label) = day_labels_for_offset(i as i64);
                    let d_temp = max_temps
                        .and_then(|a| a.get(i))
                        .and_then(|v| v.as_f64())
                        .map(|t| t.round() as i32)
                        .unwrap_or(temp_c - (i as i32));
                    let d_code = codes
                        .and_then(|a| a.get(i))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(code);
                    let (d_cond, d_icon) = wmo_code_to_condition(d_code);
                    let d_uv = if i == 0 {
                        uv_index
                    } else {
                        uvs.and_then(|a| a.get(i))
                            .and_then(|v| v.as_f64())
                            .map(|u| u.round().max(0.0) as u8)
                            .unwrap_or(uv_index)
                    };
                    let d_wind = if i == 0 {
                        wind_ms
                    } else {
                        winds
                            .and_then(|a| a.get(i))
                            .and_then(|v| v.as_f64())
                            .map(|w| (w * 10.0).round() as f32 / 10.0)
                            .unwrap_or(wind_ms)
                    };

                    forecast.push(WeatherForecastDay {
                        day_short,
                        day_full,
                        date_label,
                        temp_c: d_temp,
                        condition: d_cond.to_string(),
                        icon_kind: if i < 3 && d_icon == "clouds" {
                            "sun_rain".to_string()
                        } else {
                            d_icon.to_string()
                        },
                        uv_index: d_uv,
                        humidity: if i == 0 { humidity } else { (humidity as i32 + (i as i32 * 3 - 4)).clamp(30, 95) as u8 },
                        wind_ms: d_wind,
                    });
                }

                let (_, today_full, today_date) = day_labels_for_offset(0);
                let updated = WeatherStatus {
                    location: city,
                    day_full: today_full,
                    date_label: today_date,
                    temp_c,
                    condition: cond_str.to_string(),
                    icon_kind: "clouds".to_string(),
                    uv_index,
                    humidity,
                    wind_ms,
                    forecast,
                };

                let mut guard = cache.lock().unwrap();
                *guard = (Some(std::time::Instant::now()), updated.clone());
                return updated;
            }
        }
    }

    let mut guard = cache.lock().unwrap();
    guard.0 = Some(std::time::Instant::now());
    guard.1.clone()
}

// ─────────────────────────────────────────────────────────────────────────────
// Proactive Dynamic Island Widgets: Resource Sentinel & Git Quick Status
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSentinelStatus {
    pub has_alert: bool,
    pub alert_type: String, // "ram" | "cpu" | "temp" | "normal"
    pub process_name: String,
    pub pid: u32,
    pub cpu_percent: f64,
    pub mem_percent: f64,
    pub message: String,
    pub can_kill: bool,
    pub total_ram_gb: f64,
    pub used_ram_gb: f64,
    pub ram_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitQuickStatus {
    pub is_repo: bool,
    pub branch: String,
    pub modified_count: usize,
    pub untracked_count: usize,
    pub total_changed: usize,
    pub summary: String,
    pub files: Vec<String>,
}

#[tauri::command]
pub fn resource_sentinel_get() -> ResourceSentinelStatus {
    let mut total_kb: f64 = 0.0;
    let mut avail_kb: f64 = 0.0;
    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    total_kb = val.parse::<f64>().unwrap_or(0.0);
                }
            } else if line.starts_with("MemAvailable:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    avail_kb = val.parse::<f64>().unwrap_or(0.0);
                }
            }
        }
    }

    let total_ram_gb = (total_kb / 1024.0 / 1024.0 * 10.0).round() / 10.0;
    let used_kb = (total_kb - avail_kb).max(0.0);
    let used_ram_gb = (used_kb / 1024.0 / 1024.0 * 10.0).round() / 10.0;
    let ram_percent = if total_kb > 0.0 {
        ((used_kb / total_kb) * 100.0).round()
    } else {
        0.0
    };

    let mut max_temp_c: f64 = 0.0;
    if let Ok(entries) = std::fs::read_dir("/sys/class/thermal") {
        for entry in entries.flatten() {
            let temp_path = entry.path().join("temp");
            if let Ok(raw) = std::fs::read_to_string(temp_path) {
                if let Ok(millic) = raw.trim().parse::<f64>() {
                    let c = millic / 1000.0;
                    if c > max_temp_c {
                        max_temp_c = c;
                    }
                }
            }
        }
    }

    let mut top_pid: u32 = 0;
    let mut top_comm = String::new();
    let mut top_cpu: f64 = 0.0;
    let mut top_mem: f64 = 0.0;

    if let Ok(out) = Command::new("ps")
        .args(["-eo", "pid,pcpu,pmem,comm", "--sort=-pmem"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            for line in s.lines().skip(1).take(5) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let pid = parts[0].parse::<u32>().unwrap_or(0);
                    let cpu = parts[1].parse::<f64>().unwrap_or(0.0);
                    let mem = parts[2].parse::<f64>().unwrap_or(0.0);
                    let comm = parts[3..].join(" ");
                    if pid > 1 && !comm.contains("coucou") && (mem > top_mem || (mem > 20.0 && cpu > top_cpu)) {
                        top_pid = pid;
                        top_cpu = cpu;
                        top_mem = mem;
                        top_comm = comm;
                    }
                }
            }
        }
    }

    if top_cpu < 80.0 {
        if let Ok(out) = Command::new("ps")
            .args(["-eo", "pid,pcpu,pmem,comm", "--sort=-pcpu"])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                for line in s.lines().skip(1).take(3) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let pid = parts[0].parse::<u32>().unwrap_or(0);
                        let cpu = parts[1].parse::<f64>().unwrap_or(0.0);
                        let mem = parts[2].parse::<f64>().unwrap_or(0.0);
                        let comm = parts[3..].join(" ");
                        if pid > 1 && !comm.contains("coucou") && cpu > 80.0 {
                            top_pid = pid;
                            top_cpu = cpu;
                            top_mem = mem;
                            top_comm = comm;
                            break;
                        }
                    }
                }
            }
        }
    }

    let mut has_alert = false;
    let mut alert_type = "normal".to_string();
    let mut message = String::new();
    let mut can_kill = false;

    if top_mem >= 30.0 || (ram_percent >= 85.0 && top_mem >= 15.0) {
        has_alert = true;
        alert_type = "ram".to_string();
        message = format!("{} is consuming {:.0}% of your RAM", top_comm, top_mem);
        can_kill = top_pid > 1;
    } else if top_cpu >= 85.0 {
        has_alert = true;
        alert_type = "cpu".to_string();
        message = format!("{} is consuming {:.0}% CPU", top_comm, top_cpu);
        can_kill = top_pid > 1;
    } else if max_temp_c >= 85.0 {
        has_alert = true;
        alert_type = "temp".to_string();
        message = format!("CPU temperature high: {:.0}°C", max_temp_c);
        can_kill = false;
    }

    ResourceSentinelStatus {
        has_alert,
        alert_type,
        process_name: top_comm,
        pid: top_pid,
        cpu_percent: top_cpu,
        mem_percent: top_mem,
        message,
        can_kill,
        total_ram_gb,
        used_ram_gb,
        ram_percent,
    }
}

#[tauri::command]
pub fn resource_kill_process(pid: u32) -> Result<bool, String> {
    if pid <= 1 {
        return Err("Cannot kill system init process".to_string());
    }
    let status = Command::new("kill").args(["-15", &pid.to_string()]).status();
    match status {
        Ok(s) if s.success() => Ok(true),
        _ => {
            let s9 = Command::new("kill").args(["-9", &pid.to_string()]).status();
            match s9 {
                Ok(s) if s.success() => Ok(true),
                _ => Err(format!("Failed to kill PID {}", pid)),
            }
        }
    }
}

fn find_repo_dir(custom_path: Option<&str>) -> std::path::PathBuf {
    if let Some(p) = custom_path {
        let pb = std::path::PathBuf::from(p);
        if pb.exists() {
            return pb;
        }
    }
    let candidates = [
        "/run/media/om-raut/New Volume/COUCOU/coucou_old",
        "/run/media/om-raut/New Volume/COUCOU",
        ".",
    ];
    for c in candidates {
        let pb = std::path::PathBuf::from(c);
        if pb.join(".git").exists() {
            return pb;
        }
    }
    std::path::PathBuf::from(".")
}

#[tauri::command]
pub fn git_status_quick(target_dir: Option<String>) -> GitQuickStatus {
    let repo_dir = find_repo_dir(target_dir.as_deref());
    let mut branch = String::new();
    let mut is_repo = false;

    if let Ok(out) = Command::new("git")
        .args(["-C", &repo_dir.to_string_lossy(), "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
    {
        if out.status.success() {
            branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
            is_repo = true;
        }
    }

    if !is_repo {
        return GitQuickStatus {
            is_repo: false,
            branch: String::new(),
            modified_count: 0,
            untracked_count: 0,
            total_changed: 0,
            summary: String::new(),
            files: Vec::new(),
        };
    }

    let mut modified_count = 0;
    let mut untracked_count = 0;
    let mut files = Vec::new();

    if let Ok(out) = Command::new("git")
        .args(["-C", &repo_dir.to_string_lossy(), "status", "--porcelain"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            for line in s.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed.starts_with("??") {
                    untracked_count += 1;
                } else {
                    modified_count += 1;
                }
                if let Some(file_name) = trimmed.split_whitespace().nth(1) {
                    files.push(file_name.to_string());
                }
            }
        }
    }

    let total_changed = modified_count + untracked_count;
    let summary = if total_changed == 0 {
        "Clean".to_string()
    } else {
        format!("{} files modified", total_changed)
    };

    GitQuickStatus {
        is_repo: true,
        branch,
        modified_count,
        untracked_count,
        total_changed,
        summary,
        files,
    }
}

#[tauri::command]
pub fn git_auto_commit_ai(target_dir: Option<String>, message: Option<String>) -> Result<String, String> {
    let repo_dir = find_repo_dir(target_dir.as_deref());
    let add_status = Command::new("git")
        .args(["-C", &repo_dir.to_string_lossy(), "add", "-A"])
        .status()
        .map_err(|e| e.to_string())?;

    if !add_status.success() {
        return Err("git add failed".to_string());
    }

    let commit_msg = message.unwrap_or_else(|| {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("checkpoint: uncommitted work ({})", ts)
    });

    let commit_out = Command::new("git")
        .args(["-C", &repo_dir.to_string_lossy(), "commit", "-m", &commit_msg])
        .output()
        .map_err(|e| e.to_string())?;

    if commit_out.status.success() {
        Ok(format!("Committed: {}", commit_msg))
    } else {
        let err_str = String::from_utf8_lossy(&commit_out.stderr);
        Err(err_str.to_string())
    }
}

