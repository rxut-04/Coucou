// Voice pipeline for Coucou:
// - Local speech-to-text with resident whisper-server in RAM (privacy: audio never leaves machine or touches disk)
// - Audio capture via PipeWire pw-record (fallback to parec / arecord)
// - Real-time Voice Activity Detection (VAD) with adaptive threshold & auto-stop on silence
// - Live partial transcription streaming while speaking
// - Low-latency final transcription with accurate duration measurement
// - DBus interface `fr.louisraille.coucou.Voice` for KDE Wayland global shortcuts

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use zbus::{connection, interface};

pub const VOICE_SERVER_PORT: u16 = 8976;
pub const VOICE_SERVER_HOST: &str = "127.0.0.1";
pub const SAMPLE_RATE: u32 = 16000;
pub const CHANNELS: u16 = 1;
pub const BITS_PER_SAMPLE: u16 = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceMeterPayload {
    pub level: f32,
    pub speaking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatePayload {
    pub state: String, // "idle", "listening", "speaking", "transcribing"
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceTranscriptPayload {
    pub text: String,
    pub is_final: bool,
    pub latency_ms: Option<u64>,
}

pub struct VoiceServerProcess {
    pub child: Option<Child>,
    pub port: u16,
}

pub struct ActiveRecording {
    pub stop_signal: Arc<AtomicBool>,
    pub cancel_signal: Arc<AtomicBool>,
}

pub struct VoiceManagerState {
    pub app_handle: Arc<Mutex<Option<AppHandle>>>,
    pub server_process: Arc<Mutex<VoiceServerProcess>>,
    pub active_recording: Arc<Mutex<Option<ActiveRecording>>>,
    pub is_listening: Arc<AtomicBool>,
    pub is_speaking: Arc<AtomicBool>,
    pub last_final_text: Arc<Mutex<String>>,
    pub last_latency_ms: Arc<Mutex<u64>>,
}

impl Default for VoiceManagerState {
    fn default() -> Self {
        Self {
            app_handle: Arc::new(Mutex::new(None)),
            server_process: Arc::new(Mutex::new(VoiceServerProcess {
                child: None,
                port: VOICE_SERVER_PORT,
            })),
            active_recording: Arc::new(Mutex::new(None)),
            is_listening: Arc::new(AtomicBool::new(false)),
            is_speaking: Arc::new(AtomicBool::new(false)),
            last_final_text: Arc::new(Mutex::new(String::new())),
            last_latency_ms: Arc::new(Mutex::new(0)),
        }
    }
}

/// Creates a standard in-memory 16kHz 16-bit mono RIFF WAV header for raw PCM samples.
/// This avoids ever writing audio to disk, upholding strict privacy.
pub fn pcm_to_wav(pcm_bytes: &[u8]) -> Vec<u8> {
    let num_channels = CHANNELS;
    let sample_rate = SAMPLE_RATE;
    let bits_per_sample = BITS_PER_SAMPLE;
    let byte_rate = sample_rate * (num_channels as u32) * (bits_per_sample as u32) / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = pcm_bytes.len() as u32;
    let chunk_size = 36 + data_size;

    let mut wav = Vec::with_capacity(44 + pcm_bytes.len());
    // RIFF chunk descriptor
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&chunk_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // "fmt " sub-chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (16 for PCM)
    wav.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat (1 for PCM)
    wav.extend_from_slice(&num_channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());

    // "data" sub-chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.extend_from_slice(pcm_bytes);

    wav
}

/// Locates the whisper-server binary and models in ~/.local/share/coucou
pub fn get_whisper_paths() -> (PathBuf, PathBuf) {
    let local_dir = crate::platform::local_dir();
    let bin_dir = local_dir.join("bin");
    let server_bin = bin_dir.join("whisper-server");

    let models_dir = local_dir.join("models");
    let base_model = models_dir.join("ggml-base.bin");
    let tiny_model = models_dir.join("ggml-tiny.bin");

    let model = if base_model.exists() {
        base_model
    } else if tiny_model.exists() {
        tiny_model
    } else {
        base_model
    };

    (server_bin, model)
}

/// Ensures the local whisper-server is active on 127.0.0.1:8976.
pub async fn ensure_whisper_server(state: Arc<VoiceManagerState>) -> Result<(), String> {
    let port = {
        state.server_process.lock().unwrap().port
    };

    // Check if server is already responding on localhost:8976
    let ping_url = format!("http://{}:{}/inference", VOICE_SERVER_HOST, port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(400))
        .build()
        .map_err(|e| e.to_string())?;

    if let Ok(res) = client.get(&ping_url).send().await {
        if res.status().is_success() || res.status().as_u16() == 400 || res.status().as_u16() == 405 {
            return Ok(());
        }
    }

    let should_spawn = {
        let mut server = state.server_process.lock().unwrap();
        // If existing child is dead, clean it up
        if let Some(ref mut child) = server.child {
            if let Ok(Some(_)) = child.try_wait() {
                server.child = None;
            }
        }
        server.child.is_none()
    };

    if !should_spawn {
        return Ok(());
    }

    let (bin_path, model_path) = get_whisper_paths();
    if !bin_path.exists() {
        return Err(format!("whisper-server not found at {}", bin_path.display()));
    }
    if !model_path.exists() {
        return Err(format!("whisper model not found at {}", model_path.display()));
    }

    let bin_dir = bin_path.parent().unwrap_or(Path::new("")).to_path_buf();
    let ld_path = match std::env::var("LD_LIBRARY_PATH") {
        Ok(existing) => format!("{}:{}", bin_dir.display(), existing),
        Err(_) => bin_dir.display().to_string(),
    };

    crate::log::line(format!(
        "voice: starting whisper-server on port {} with model {}",
        port,
        model_path.display()
    ));

    let child = Command::new(&bin_path)
        .arg("-m")
        .arg(&model_path)
        .arg("--host")
        .arg(VOICE_SERVER_HOST)
        .arg("--port")
        .arg(port.to_string())
        .arg("-t")
        .arg("6")
        .arg("-l")
        .arg("auto")
        .env("LD_LIBRARY_PATH", ld_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn whisper-server: {e}"))?;

    {
        let mut server = state.server_process.lock().unwrap();
        server.child = Some(child);
    }

    // Wait up to 3 seconds for server port to listen
    let start_wait = Instant::now();
    while start_wait.elapsed() < Duration::from_secs(3) {
        tokio::time::sleep(Duration::from_millis(80)).await;
        if let Ok(res) = client.get(&ping_url).send().await {
            if res.status().is_success() || res.status().as_u16() == 400 || res.status().as_u16() == 405 {
                crate::log::line(format!(
                    "voice: whisper-server ready in {:.1} ms",
                    start_wait.elapsed().as_secs_f64() * 1000.0
                ));
                return Ok(());
            }
        }
    }

    Ok(())
}

/// Sends an in-memory WAV buffer to the whisper server and extracts clean text.
pub async fn transcribe_wav(
    port: u16,
    wav_bytes: &[u8],
) -> Result<String, String> {
    if wav_bytes.len() <= 44 {
        return Ok(String::new());
    }

    let boundary = "----CoucouVoiceBoundary42981";
    let mut body = Vec::new();

    // Multipart header for "file"
    let header = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"voice.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
    );
    body.extend_from_slice(header.as_bytes());
    body.extend_from_slice(wav_bytes);
    let footer = format!("\r\n--{boundary}--\r\n");
    body.extend_from_slice(footer.as_bytes());

    let url = format!("http://{}:{}/inference", VOICE_SERVER_HOST, port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let res = client
        .post(&url)
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .body(body)
        .send()
        .await
        .map_err(|e| format!("HTTP request to whisper-server failed: {e}"))?;

    if !res.status().is_success() {
        return Err(format!("whisper-server returned HTTP {}", res.status()));
    }

    let resp_text = res.text().await.map_err(|e| e.to_string())?;

    // Parse JSON response: `{"text": "..."}`
    #[derive(Deserialize)]
    struct InferenceResult {
        text: Option<String>,
    }

    let parsed: InferenceResult = serde_json::from_str(&resp_text)
        .map_err(|e| format!("failed to parse whisper response '{resp_text}': {e}"))?;

    let clean = parsed
        .text
        .unwrap_or_default()
        .trim()
        .trim_start_matches(|c: char| c == '[' || c == '(')
        .trim_end_matches(|c: char| c == ']' || c == ')')
        .trim()
        .to_string();

    Ok(clean)
}

/// Spawns the audio recorder and VAD loop.
pub fn start_audio_capture_and_vad(
    app: AppHandle,
    state: Arc<VoiceManagerState>,
) -> Result<(), String> {
    let stop_signal = Arc::new(AtomicBool::new(false));
    let cancel_signal = Arc::new(AtomicBool::new(false));

    {
        let mut rec_guard = state.active_recording.lock().unwrap();
        // If an existing recording is running, stop it first
        if let Some(prev) = rec_guard.take() {
            prev.stop_signal.store(true, Ordering::SeqCst);
        }
        *rec_guard = Some(ActiveRecording {
            stop_signal: stop_signal.clone(),
            cancel_signal: cancel_signal.clone(),
        });
    }

    state.is_listening.store(true, Ordering::SeqCst);
    state.is_speaking.store(false, Ordering::SeqCst);

    let _ = app.emit("voice-state", VoiceStatePayload {
        state: "listening".to_string(),
        error: None,
    });

    let state_clone = state.clone();
    let app_clone = app.clone();

    std::thread::spawn(move || {
        // Find best audio recorder tool: pw-record > parec > arecord
        let child_proc = Command::new("pw-record")
            .args(["--rate", "16000", "--channels", "1", "--format", "s16", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .or_else(|_| {
                Command::new("parec")
                    .args(["--rate", "16000", "--channels", "1", "--format", "s16le"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
            })
            .or_else(|_| {
                Command::new("arecord")
                    .args(["-r", "16000", "-c", "1", "-f", "S16_LE", "-t", "raw", "-"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
            });

        let mut child = match child_proc {
            Ok(c) => c,
            Err(e) => {
                state_clone.is_listening.store(false, Ordering::SeqCst);
                let _ = app_clone.emit("voice-state", VoiceStatePayload {
                    state: "idle".to_string(),
                    error: Some(format!("Could not access microphone: {e}")),
                });
                return;
            }
        };

        let mut stdout = match child.stdout.take() {
            Some(s) => s,
            None => return,
        };

        let mut pcm_buffer: Vec<u8> = Vec::with_capacity(16000 * 2 * 10); // up to 10s audio in RAM
        let mut frame_buf = [0u8; 1280]; // 640 samples = 40 ms at 16kHz
        let mut noise_floor: f32 = 400.0;
        let mut speech_frames = 0;
        let mut silence_frames = 0;
        let mut has_spoken = false;
        let mut last_partial_time = Instant::now();
        let mut speech_start_time: Option<Instant> = None;
        let mut silence_stop_detected_at: Option<Instant> = None;

        while !stop_signal.load(Ordering::SeqCst) && !cancel_signal.load(Ordering::SeqCst) {
            match stdout.read_exact(&mut frame_buf) {
                Ok(_) => {
                    pcm_buffer.extend_from_slice(&frame_buf);

                    // Compute RMS energy of this 40ms frame
                    let mut sum_sq: f64 = 0.0;
                    for chunk in frame_buf.chunks_exact(2) {
                        let sample = i16::from_le_bytes([chunk[0], chunk[1]]) as f64;
                        sum_sq += sample * sample;
                    }
                    let num_samples = frame_buf.len() / 2;
                    let rms = ((sum_sq / (num_samples as f64)).sqrt()) as f32;

                    // Update adaptive noise floor slowly during silence
                    if !has_spoken && rms < noise_floor * 1.5 {
                        noise_floor = noise_floor * 0.95 + rms * 0.05;
                    }

                    // VAD threshold: speech when RMS > noise_floor * 2.2 or absolute > 1100
                    let speech_threshold = (noise_floor * 2.2).max(1100.0);
                    let is_speech_now = rms > speech_threshold;

                    let normalized_level = (rms / 120.0).clamp(0.0, 100.0);
                    let _ = app_clone.emit("voice-meter", VoiceMeterPayload {
                        level: normalized_level,
                        speaking: is_speech_now,
                    });

                    if is_speech_now {
                        speech_frames += 1;
                        silence_frames = 0;
                        if speech_frames >= 3 {
                            if !has_spoken {
                                has_spoken = true;
                                speech_start_time = Some(Instant::now());
                                state_clone.is_speaking.store(true, Ordering::SeqCst);
                                let _ = app_clone.emit("voice-state", VoiceStatePayload {
                                    state: "speaking".to_string(),
                                    error: None,
                                });
                            }
                        }
                    } else {
                        speech_frames = 0;
                        if has_spoken {
                            silence_frames += 1;
                            // 16 frames * 40 ms = 640 ms silence -> speech complete
                            if silence_frames >= 16 {
                                silence_stop_detected_at = Some(Instant::now());
                                break;
                            }
                        }
                    }

                    // Live streaming partial transcription every ~750ms while speaking
                    if has_spoken && last_partial_time.elapsed() > Duration::from_millis(750) {
                        last_partial_time = Instant::now();
                        let buffer_snapshot = pcm_buffer.clone();
                        let app_for_partial = app_clone.clone();

                        tauri::async_runtime::spawn(async move {
                            let wav = pcm_to_wav(&buffer_snapshot);
                            if let Ok(partial) = transcribe_wav(VOICE_SERVER_PORT, &wav).await {
                                if !partial.is_empty() {
                                    let _ = app_for_partial.emit("voice-partial-transcript", VoiceTranscriptPayload {
                                        text: partial,
                                        is_final: false,
                                        latency_ms: None,
                                    });
                                }
                            }
                        });
                    }

                    // Max utterance safeguard: auto-finish if user speaks continuously for 15s
                    if let Some(t0) = speech_start_time {
                        if t0.elapsed() > Duration::from_secs(15) {
                            silence_stop_detected_at = Some(Instant::now());
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        }

        // Kill audio capture child immediately
        let _ = child.kill();
        let _ = child.wait();

        let cancelled = cancel_signal.load(Ordering::SeqCst);
        if cancelled {
            pcm_buffer.clear();
            state_clone.is_listening.store(false, Ordering::SeqCst);
            state_clone.is_speaking.store(false, Ordering::SeqCst);
            let _ = app_clone.emit("voice-state", VoiceStatePayload {
                state: "idle".to_string(),
                error: None,
            });
            return;
        }

        // Check if user spoke anything meaningful (at least 0.4s of audio)
        if !has_spoken || pcm_buffer.len() < (SAMPLE_RATE as usize * 2 / 2) {
            pcm_buffer.clear();
            state_clone.is_listening.store(false, Ordering::SeqCst);
            state_clone.is_speaking.store(false, Ordering::SeqCst);
            let _ = app_clone.emit("voice-state", VoiceStatePayload {
                state: "idle".to_string(),
                error: None,
            });
            return;
        }

        // Final transcription path
        let silence_t0 = silence_stop_detected_at.unwrap_or_else(Instant::now);
        state_clone.is_listening.store(false, Ordering::SeqCst);
        state_clone.is_speaking.store(false, Ordering::SeqCst);

        let _ = app_clone.emit("voice-state", VoiceStatePayload {
            state: "transcribing".to_string(),
            error: None,
        });

        let final_wav = pcm_to_wav(&pcm_buffer);
        // Clear raw PCM buffer immediately (audio lives purely in RAM temporarily, then wiped)
        pcm_buffer.clear();

        let app_final = app_clone.clone();
        let state_final = state_clone.clone();

        tauri::async_runtime::spawn(async move {
            match transcribe_wav(VOICE_SERVER_PORT, &final_wav).await {
                Ok(final_text) => {
                    let latency_ms = silence_t0.elapsed().as_millis() as u64;
                    crate::log::line(format!(
                        "voice: final transcript in {} ms: \"{}\"",
                        latency_ms, final_text
                    ));

                    *state_final.last_final_text.lock().unwrap() = final_text.clone();
                    *state_final.last_latency_ms.lock().unwrap() = latency_ms;

                    let _ = app_final.emit("voice-final-transcript", VoiceTranscriptPayload {
                        text: final_text.clone(),
                        is_final: true,
                        latency_ms: Some(latency_ms),
                    });

                    let _ = app_final.emit("voice-state", VoiceStatePayload {
                        state: "idle".to_string(),
                        error: None,
                    });

                    // Trigger Voice Intent Engine immediately
                    if !final_text.trim().is_empty() {
                        let app_intent = app_final.clone();
                        let text_intent = final_text.clone();
                        tauri::async_runtime::spawn(async move {
                            crate::voice_intent::process_voice_query(&app_intent, &text_intent).await;
                        });
                    }
                }
                Err(err) => {
                    crate::log::line(format!("voice transcription error: {err}"));
                    let _ = app_final.emit("voice-state", VoiceStatePayload {
                        state: "idle".to_string(),
                        error: Some(err),
                    });
                }
            }
        });
    });

    Ok(())
}

// ── DBus Interface for KDE Wayland ──────────────────────────────────────────

pub struct VoiceDbusObject {
    pub app: AppHandle,
    pub state: Arc<VoiceManagerState>,
}

#[interface(name = "fr.louisraille.coucou.Voice")]
impl VoiceDbusObject {
    fn toggle(&self) -> bool {
        let is_listening = self.state.is_listening.load(Ordering::SeqCst);
        if is_listening {
            stop_voice_listening(&self.state);
            false
        } else {
            let app_handle = self.app.clone();
            let state_clone = self.state.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = start_voice_listening(app_handle, state_clone).await {
                    eprintln!("[coucou] failed to start voice from DBus: {e}");
                }
            });
            true
        }
    }

    fn start(&self) -> bool {
        let app_handle = self.app.clone();
        let state_clone = self.state.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = start_voice_listening(app_handle, state_clone).await {
                eprintln!("[coucou] failed to start voice from DBus: {e}");
            }
        });
        true
    }

    fn stop(&self) -> bool {
        stop_voice_listening(&self.state);
        true
    }

    fn cancel(&self) -> bool {
        cancel_voice_listening(&self.state);
        true
    }

    fn status(&self) -> String {
        let listening = self.state.is_listening.load(Ordering::SeqCst);
        let speaking = self.state.is_speaking.load(Ordering::SeqCst);
        let last_text = self.state.last_final_text.lock().unwrap().clone();
        let latency = *self.state.last_latency_ms.lock().unwrap();

        serde_json::json!({
            "listening": listening,
            "speaking": speaking,
            "lastTranscript": last_text,
            "latencyMs": latency,
        })
        .to_string()
    }

    async fn process_query(&self, query: String) -> String {
        let res = crate::voice_intent::process_voice_query(&self.app, &query).await;
        serde_json::to_string(&res).unwrap_or_default()
    }
}

pub async fn serve_voice_dbus(app: AppHandle, state: Arc<VoiceManagerState>) -> zbus::Result<()> {
    let conn = connection::Builder::session()?
        .name("fr.louisraille.coucou.Voice")?
        .serve_at("/fr/louisraille/coucou/Voice", VoiceDbusObject { app, state })?
        .build()
        .await?;
    let _conn = conn;
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

// ── Public Helper Functions ─────────────────────────────────────────────────

pub async fn start_voice_listening(
    app: AppHandle,
    state: Arc<VoiceManagerState>,
) -> Result<(), String> {
    // 1. Expand Island UI to "voice" view immediately (< 50 ms target)
    let _ = app.emit_to(crate::island::WINDOW_LABEL, "show-voice-view", ());

    // 2. Ensure whisper-server is running
    ensure_whisper_server(state.clone()).await?;

    // 3. Start audio capture and VAD
    start_audio_capture_and_vad(app, state)?;

    Ok(())
}

pub fn stop_voice_listening(state: &Arc<VoiceManagerState>) {
    let rec_guard = state.active_recording.lock().unwrap();
    if let Some(ref rec) = *rec_guard {
        rec.stop_signal.store(true, Ordering::SeqCst);
    }
}

pub fn cancel_voice_listening(state: &Arc<VoiceManagerState>) {
    let rec_guard = state.active_recording.lock().unwrap();
    if let Some(ref rec) = *rec_guard {
        rec.cancel_signal.store(true, Ordering::SeqCst);
    }
    state.is_listening.store(false, Ordering::SeqCst);
    state.is_speaking.store(false, Ordering::SeqCst);
}

// ── Tauri Commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn voice_start(
    app: AppHandle,
    state: State<'_, Arc<VoiceManagerState>>,
) -> Result<(), String> {
    start_voice_listening(app, state.inner().clone()).await
}

#[tauri::command]
pub fn voice_stop(state: State<'_, Arc<VoiceManagerState>>) {
    stop_voice_listening(state.inner());
}

#[tauri::command]
pub fn voice_cancel(state: State<'_, Arc<VoiceManagerState>>) {
    cancel_voice_listening(state.inner());
}

#[tauri::command]
pub async fn voice_toggle(
    app: AppHandle,
    state: State<'_, Arc<VoiceManagerState>>,
) -> Result<bool, String> {
    let is_listening = state.is_listening.load(Ordering::SeqCst);
    if is_listening {
        stop_voice_listening(state.inner());
        Ok(false)
    } else {
        start_voice_listening(app, state.inner().clone()).await?;
        Ok(true)
    }
}

#[tauri::command]
pub fn voice_status(state: State<'_, Arc<VoiceManagerState>>) -> serde_json::Value {
    let listening = state.is_listening.load(Ordering::SeqCst);
    let speaking = state.is_speaking.load(Ordering::SeqCst);
    let last_text = state.last_final_text.lock().unwrap().clone();
    let latency = *state.last_latency_ms.lock().unwrap();

    serde_json::json!({
        "listening": listening,
        "speaking": speaking,
        "lastTranscript": last_text,
        "latencyMs": latency,
    })
}

#[tauri::command]
pub async fn voice_process_query(
    app: AppHandle,
    query: String,
) -> Result<crate::voice_intent::VoiceActionResult, String> {
    Ok(crate::voice_intent::process_voice_query(&app, &query).await)
}

/// Initializes voice service and registers DBus on app startup.
pub fn start(app: AppHandle) {
    let state = app.state::<Arc<VoiceManagerState>>().inner().clone();
    {
        let mut handle = state.app_handle.lock().unwrap();
        *handle = Some(app.clone());
    }

    let state_for_dbus = state.clone();
    let app_for_dbus = app.clone();

    #[cfg(target_os = "linux")]
    tauri::async_runtime::spawn(async move {
        if let Err(err) = serve_voice_dbus(app_for_dbus, state_for_dbus).await {
            eprintln!("[coucou] voice DBus error: {err}");
        }
    });

    // Warm up whisper-server in background on app start so first voice trigger is instant
    let state_for_warmup = state.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = ensure_whisper_server(state_for_warmup).await {
            eprintln!("[coucou] whisper-server background startup: {e}");
        }
    });
}
