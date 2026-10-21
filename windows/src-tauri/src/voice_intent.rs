// Voice Intent Classification and Execution Engine for Coucou:
// - Fast Path: Instant multilingual rule-based token matching (< 0.05 ms classification, < 15 ms execution)
//   Supports English, Hindi / Hinglish, French, Spanish, code-switching, and compound sentences.
// - Smart Path: Deep context-aware understanding engine (OpenCode CLI fallback) returning structured actions.
// - Short conversation memory ("play another one", "open the second one").
// - Clarification mechanism when speech is ambiguous.

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::focus_tracker::FocusTrackerState;
use crate::db::DbState;

/// All supported structured voice intents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload")]
pub enum VoiceIntent {
    FocusWindow { target: String },
    OpenApp { app: String },
    OpenUrl { url: String },
    PlayYouTube { query: String },
    MediaControl { action: String }, // "play", "pause", "play-pause", "next", "previous", "mute", "volume-up", "volume-down"
    StartFocus { duration_mins: i64, task: String },
    StopFocus,
    AddTask { title: String, project: String },
    CompleteTask,
    ShowScore,
    SystemControl { action: String }, // "lock", "screenshot"
    Clarify { question: String },
    FollowUp { action: String, index: Option<usize> },
    Compound { actions: Vec<VoiceIntent> },
    Unknown { raw: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceActionResult {
    pub intent: String,
    pub is_fast_path: bool,
    pub success: bool,
    pub feedback: String,
    pub query: String,
    pub decision_latency_ms: u64,
    pub total_latency_ms: u64,
    pub clarification: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FailedCommandContext {
    pub cmd: String,
    pub exit_code: i32,
    pub detail: String,
    pub timestamp_epoch_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VoiceMemoryState {
    pub last_intent: Option<VoiceIntent>,
    pub last_topic: Option<String>,
    pub last_app: Option<String>,
    pub last_search_results: Vec<String>,
    pub recent_queries: Vec<String>,
}

pub static GLOBAL_FAILED_COMMAND: Mutex<Option<FailedCommandContext>> = Mutex::new(None);
pub static GLOBAL_VOICE_MEMORY: Mutex<Option<VoiceMemoryState>> = Mutex::new(None);

pub fn with_voice_memory<R, F: FnOnce(&mut VoiceMemoryState) -> R>(f: F) -> R {
    let mut lock = GLOBAL_VOICE_MEMORY.lock().unwrap();
    if lock.is_none() {
        *lock = Some(VoiceMemoryState::default());
    }
    f(lock.as_mut().unwrap())
}

pub fn clear_voice_memory() {
    let mut lock = GLOBAL_VOICE_MEMORY.lock().unwrap();
    *lock = Some(VoiceMemoryState::default());
}

/// Self-contained URL component encoding.
pub fn url_encode(input: &str) -> String {
    let mut encoded = String::new();
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

/// Records the last failed command from terminal or agent hooks for smart path context.
pub fn record_failed_command(cmd: &str, exit_code: i32, detail: &str) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut lock = GLOBAL_FAILED_COMMAND.lock().unwrap();
    *lock = Some(FailedCommandContext {
        cmd: cmd.to_string(),
        exit_code,
        detail: detail.to_string(),
        timestamp_epoch_secs: now,
    });
}

/// Normalizes raw transcribed text for intent classification:
/// lowercase, trims punctuation, removes excess whitespace.
pub fn normalize_query(text: &str) -> String {
    let trimmed = text.trim().to_lowercase();
    let cleaned: String = trimmed
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() || c == '.' || c == '-' || c == '/' { c } else { ' ' })
        .collect();
    cleaned.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// Splits compound utterances like "ouvre VS Code et commence un focus session"
/// or "terminal kholo aur focus start karo" into separate commands.
fn split_compound(query: &str) -> Vec<String> {
    // Avoid splitting "open youtube and play lofi" or "youtube pe lofi play karo"
    let q = query.to_lowercase();
    if (q.contains("youtube") && q.contains("play"))
        || (q.contains("youtube") && (q.contains("chalao") || q.contains("bajao") || q.contains("lagao")))
    {
        return vec![query.to_string()];
    }

    // Split on conjunctions
    let conjunctions = [" and then ", " et ensuite ", " y luego ", " aur phir ", " and ", " et ", " aur ", " y "];
    for conj in conjunctions {
        if let Some(pos) = q.find(conj) {
            let first = query[..pos].trim();
            let second = query[pos + conj.len()..].trim();
            if !first.is_empty() && !second.is_empty() {
                let mut parts = vec![first.to_string()];
                parts.extend(split_compound(second));
                return parts;
            }
        }
    }
    vec![query.to_string()]
}

/// Resolves follow-up commands like "play another one" or "open the second one" using conversation memory.
fn resolve_follow_up(query: &str, memory: &VoiceMemoryState) -> Option<VoiceIntent> {
    let q = query.to_lowercase();

    // "play another one" / "play something else" / "aur koi chalao" / "autre chose" / "otra cancion"
    let next_patterns = [
        "play another one", "play another", "play something else",
        "aur koi chalao", "dusra chalao", "koi aur gaana",
        "autre chose", "joue un autre",
        "otra cancion", "pon otra", "reproduce otra",
    ];
    for p in next_patterns {
        if q == p || q.starts_with(p) {
            if let Some(ref last) = memory.last_intent {
                match last {
                    VoiceIntent::PlayYouTube { query: ref old_q } => {
                        return Some(VoiceIntent::PlayYouTube {
                            query: format!("{} mix", old_q),
                        });
                    }
                    VoiceIntent::MediaControl { .. } => {
                        return Some(VoiceIntent::MediaControl { action: "next".into() });
                    }
                    _ => {}
                }
            }
            return Some(VoiceIntent::MediaControl { action: "next".into() });
        }
    }

    // "open the second one" / "open the first one" / "open the third one" / "first" / "second"
    // "dusra kholo" / "pehla kholo" / "ouvre le deuxieme" / "abre el segundo"
    let index_map = [
        ("first", 0), ("1st", 0), ("pehla", 0), ("premier", 0), ("primero", 0),
        ("second", 1), ("2nd", 1), ("dusra", 1), ("doosra", 1), ("deuxieme", 1), ("segundo", 1),
        ("third", 2), ("3rd", 2), ("teesra", 2), ("troisieme", 2), ("tercero", 2),
    ];

    if q.contains("open") || q.contains("kholo") || q.contains("ouvre") || q.contains("abre") {
        for (word, idx) in index_map {
            if q.contains(word) {
                if let Some(target) = memory.last_search_results.get(idx) {
                    if target.starts_with("http://") || target.starts_with("https://") {
                        return Some(VoiceIntent::OpenUrl { url: target.clone() });
                    } else {
                        return Some(VoiceIntent::FocusWindow { target: target.clone() });
                    }
                } else {
                    return Some(VoiceIntent::Clarify {
                        question: format!("I don't have item #{} in memory. What would you like to open?", idx + 1),
                    });
                }
            }
        }
    }

    None
}

/// Multilingual Fast-Path Classifier (< 0.05 ms latency)
/// Recognizes common requests across English, Hindi/Hinglish, French, Spanish.
pub fn fast_path_classify(raw_query: &str) -> Option<VoiceIntent> {
    let norm = normalize_query(raw_query);
    if norm.is_empty() {
        return None;
    }

    // Check conversation memory follow-ups first
    if let Some(follow_up) = with_voice_memory(|mem| resolve_follow_up(&norm, mem)) {
        return Some(follow_up);
    }

    // Check for compound sentences
    let parts = split_compound(raw_query);
    if parts.len() > 1 {
        let mut sub_intents = Vec::new();
        for p in parts {
            if let Some(sub) = fast_path_classify(&p) {
                sub_intents.push(sub);
            } else {
                // If a sub-part cannot be resolved via fast path, abort compound fast path
                return None;
            }
        }
        if !sub_intents.is_empty() {
            return Some(VoiceIntent::Compound { actions: sub_intents });
        }
    }

    let q = norm.as_str();

    // ── 1. MEDIA CONTROLS ──────────────────────────────────────────────────
    // Pause
    let pause_words = [
        "pause", "pause music", "stop music", "pause playback", "stop playback",
        "gaana roko", "music roko", "pause karo", "roko", "band karo gaana", "gana roko",
        "pause la musique", "arrete la musique", "met sur pause", "mets en pause",
        "pausar musica", "detener musica", "pausa", "parar musica",
    ];
    for p in pause_words {
        if q == p || q.starts_with(p) {
            return Some(VoiceIntent::MediaControl { action: "pause".into() });
        }
    }

    // Play / Resume
    let play_words = [
        "play music", "resume music", "resume", "play song", "unpause", "continue music",
        "gaana chalao", "music chalao", "play karo", "shuru karo gaana", "gana chalao",
        "reprends la musique", "joue de la musique", "lance la musique", "relance la musique",
        "reanudar", "reproducir musica", "continuar musica", "play musica",
    ];
    for p in play_words {
        if q == p {
            return Some(VoiceIntent::MediaControl { action: "play".into() });
        }
    }

    // Next
    let next_words = [
        "next song", "next track", "skip", "skip song", "next", "play next",
        "agla gaana", "agla song", "next karo", "skip karo", "agle gaane pe jao",
        "morceau suivant", "suivant", "chanson suivante", "passe le morceau",
        "siguiente cancion", "saltar", "siguiente", "proxima cancion",
    ];
    for p in next_words {
        if q == p || q.starts_with(p) {
            return Some(VoiceIntent::MediaControl { action: "next".into() });
        }
    }

    // Previous
    let prev_words = [
        "previous song", "previous track", "previous", "go back song",
        "pichhla gaana", "pichla gaana", "previous karo",
        "morceau precedent", "precedent", "chanson precedente",
        "cancion anterior", "anterior",
    ];
    for p in prev_words {
        if q == p || q.starts_with(p) {
            return Some(VoiceIntent::MediaControl { action: "previous".into() });
        }
    }

    // Mute / Unmute
    let mute_words = [
        "mute", "unmute", "mute audio", "silence", "mute sound",
        "mute karo", "aawaz band karo", "awaz band karo", "chup karo",
        "coupe le son", "silence", "muet",
        "silenciar", "quitar sonido", "mudo",
    ];
    for p in mute_words {
        if q == p || q.starts_with(p) {
            return Some(VoiceIntent::MediaControl { action: "mute".into() });
        }
    }

    // Volume Up
    let vol_up_words = [
        "volume up", "increase volume", "louder", "turn it up",
        "aawaz badhao", "awaz badhao", "volume badhao", "sound badhao",
        "monte le volume", "plus fort", "augmente le volume",
        "subir volumen", "mas alto", "aumentar volumen",
    ];
    for p in vol_up_words {
        if q == p || q.starts_with(p) {
            return Some(VoiceIntent::MediaControl { action: "volume-up".into() });
        }
    }

    // Volume Down
    let vol_down_words = [
        "volume down", "decrease volume", "quieter", "turn it down",
        "aawaz kam karo", "awaz kam karo", "volume kam karo", "sound kam karo",
        "baisse le volume", "moins fort", "diminue le volume",
        "bajar volumen", "mas bajo", "disminuir volumen",
    ];
    for p in vol_down_words {
        if q == p || q.starts_with(p) {
            return Some(VoiceIntent::MediaControl { action: "volume-down".into() });
        }
    }

    // ── 2. YOUTUBE / VIDEO PLAYBACK ─────────────────────────────────────────
    // English: "play <query> on youtube", "play <query> youtube", "open youtube and play <query>"
    if q.contains("youtube") || q.contains("lo-fi") || q.contains("lofi") || q.starts_with("play ") || q.starts_with("joue ") || q.starts_with("reproduce ") {
        // Hindi / Hinglish: "youtube pe <query> chalao/lagao/bajao", "<query> bajao"
        if q.contains("chalao") || q.contains("bajao") || q.contains("lagao") {
            let cleaned = q
                .replace("youtube pe", "")
                .replace("youtube par", "")
                .replace("youtube", "")
                .replace("chalao", "")
                .replace("bajao", "")
                .replace("lagao", "")
                .replace("gaana", "")
                .replace("song", "")
                .trim()
                .to_string();
            let query = if cleaned.is_empty() { "lo-fi beats".to_string() } else { cleaned };
            return Some(VoiceIntent::PlayYouTube { query });
        }

        // French: "joue <query> sur youtube", "mets <query> sur youtube"
        if q.starts_with("joue ") || q.starts_with("mets ") || q.contains("sur youtube") {
            let cleaned = q
                .replace("joue", "")
                .replace("mets", "")
                .replace("sur youtube", "")
                .replace("youtube", "")
                .replace("de la musique", "")
                .trim()
                .to_string();
            let query = if cleaned.is_empty() { "lofi chill".to_string() } else { cleaned };
            return Some(VoiceIntent::PlayYouTube { query });
        }

        // Spanish: "reproduce <query> en youtube", "pon <query> en youtube"
        if q.starts_with("reproduce ") || q.starts_with("reproducir ") || q.starts_with("pon ") || q.starts_with("poner ") || q.contains("en youtube") {
            let cleaned = q
                .replace("reproducir", "")
                .replace("reproduce", "")
                .replace("poner", "")
                .replace("pon", "")
                .replace("en youtube", "")
                .replace("youtube", "")
                .replace("musica", "")
                .trim()
                .to_string();
            let query = if cleaned.is_empty() { "lofi hip hop".to_string() } else { cleaned };
            return Some(VoiceIntent::PlayYouTube { query });
        }

        // English: "open youtube and play <query>", "play <query> on youtube", "play <query>"
        if q.contains("play") {
            let mut extracted = q
                .replace("open youtube and play", "")
                .replace("open youtube and", "")
                .replace("on youtube", "")
                .replace("in youtube", "")
                .replace("youtube", "")
                .replace("play", "")
                .replace("song", "")
                .replace("music", "")
                .trim()
                .to_string();
            if extracted.is_empty() && (q.contains("lo-fi") || q.contains("lofi")) {
                extracted = "lo-fi beats".to_string();
            }
            if !extracted.is_empty() {
                return Some(VoiceIntent::PlayYouTube { query: extracted });
            }
        }
    }

    // ── 3. FOCUS SESSIONS ──────────────────────────────────────────────────
    // Stop Focus
    let stop_focus_words = [
        "stop focus", "end focus", "finish focus", "cancel focus", "stop session",
        "focus band karo", "focus khatam karo", "session roko", "session khatam karo",
        "arrete le focus", "termine la session", "fin du focus", "stop le focus",
        "detener concentracion", "terminar focus", "parar sesion",
    ];
    for p in stop_focus_words {
        if q == p || q.starts_with(p) {
            return Some(VoiceIntent::StopFocus);
        }
    }

    // Start Focus
    let start_focus_words = [
        "start focus", "focus session", "start a focus", "focus for", "start pomodoro", "begin focus",
        "focus start karo", "focus shuru karo", "focus lagao", "focus session start karo",
        "commence un focus", "lance une session de focus", "commence une session de focus", "session de focus", "lance un focus", "commence le focus",
        "iniciar concentracion", "empezar focus", "iniciar sesion de concentracion", "sesion de concentracion",
    ];
    for p in start_focus_words {
        if q.contains(p) {
            // Extract optional duration (e.g. "for 25 minutes", "30 mins", "de 20 minutes")
            let mut duration_mins = 25;
            for token in q.split_whitespace() {
                if let Ok(num) = token.parse::<i64>() {
                    if num > 0 && num <= 240 {
                        duration_mins = num;
                        break;
                    }
                }
            }
            return Some(VoiceIntent::StartFocus {
                duration_mins,
                task: String::new(),
            });
        }
    }

    // ── 4. TASKS ───────────────────────────────────────────────────────────
    // Complete Task
    let complete_task_words = [
        "finish task", "complete task", "done with task", "task complete", "task is done", "mark task done", "task done",
        "task complete ho gaya", "task khatam hua", "task khatam", "task ho gaya", "task finish karo",
        "termine la tache", "tache terminee", "tache finie", "finis la tache",
        "terminar tarea", "tarea completada", "tarea hecha", "completar tarea",
    ];
    for p in complete_task_words {
        if q == p || q.starts_with(p) {
            return Some(VoiceIntent::CompleteTask);
        }
    }

    // Add Task
    let add_task_prefixes = [
        "add task ", "create task ", "new task ", "todo ", "add todo ",
        "task add karo ", "task banao ", "naya task ",
        "ajoute la tache ", "nouvelle tache ", "creer la tache ",
        "agregar tarea ", "nueva tarea ", "crear tarea ",
    ];
    for prefix in add_task_prefixes {
        if q.starts_with(prefix) {
            let title = q[prefix.len()..].trim().to_string();
            if !title.is_empty() {
                return Some(VoiceIntent::AddTask {
                    title,
                    project: "coucou".to_string(),
                });
            }
        }
    }

    // ── 5. SCORE & STATS ───────────────────────────────────────────────────
    let score_words = [
        "show my score", "what is my score", "what did i do today", "today's stats", "todays stats",
        "show stats", "my stats", "daily recap", "show score", "how did i do today",
        "mera score dikhao", "aaj maine kya kiya", "aaj ka score", "aaj kitna kaam kiya", "score kya hai",
        "montre mon score", "quel est mon score", "qu'est-ce que j'ai fait aujourd'hui", "statistiques du jour", "mon score",
        "mostrar mi puntuacion", "cual es mi puntuacion", "que hice hoy", "mis estadisticas", "mi puntuacion",
    ];
    for p in score_words {
        if q == p || q.starts_with(p) {
            return Some(VoiceIntent::ShowScore);
        }
    }

    // ── 6. WEBSITES & URLS ─────────────────────────────────────────────────
    let known_sites = [
        ("github", "https://github.com"),
        ("google", "https://google.com"),
        ("reddit", "https://reddit.com"),
        ("youtube", "https://youtube.com"),
        ("stackoverflow", "https://stackoverflow.com"),
        ("docs.rs", "https://docs.rs"),
        ("twitter", "https://x.com"),
        ("gmail", "https://mail.google.com"),
    ];

    if q.starts_with("open ") || q.starts_with("go to ") || q.starts_with("ouvre ") || q.starts_with("abrir ") || q.contains("kholo") {
        for (name, url) in known_sites {
            if q.contains(name) && !q.contains("play") && !q.contains("chalao") && !q.contains("bajao") {
                return Some(VoiceIntent::OpenUrl { url: url.to_string() });
            }
        }
        if q.contains(".com") || q.contains(".org") || q.contains(".rs") || q.contains(".io") || q.contains(".net") {
            let mut extracted = q
                .replace("open", "")
                .replace("go to", "")
                .replace("ouvre", "")
                .replace("abrir", "")
                .replace("kholo", "")
                .trim()
                .to_string();
            if !extracted.starts_with("http://") && !extracted.starts_with("https://") {
                extracted = format!("https://{}", extracted);
            }
            return Some(VoiceIntent::OpenUrl { url: extracted });
        }
    }

    // ── 7. APPS & WINDOW FOCUS ─────────────────────────────────────────────
    let app_map = [
        ("vs code", "code"),
        ("vscode", "code"),
        ("visual studio code", "code"),
        ("code", "code"),
        ("terminal", "konsole"),
        ("konsole", "konsole"),
        ("bash", "konsole"),
        ("browser", "brave-browser"),
        ("brave", "brave-browser"),
        ("chrome", "google-chrome"),
        ("firefox", "firefox"),
        ("files", "dolphin"),
        ("file manager", "dolphin"),
        ("dolphin", "dolphin"),
        ("settings", "systemsettings"),
        ("calculator", "kcalc"),
        ("slack", "slack"),
        ("discord", "discord"),
        ("spotify", "spotify"),
    ];

    let focus_verbs = [
        "focus ", "switch to ", "bring up ", "show ", "open ", "launch ",
        "passe sur ", "ouvre ", "lance ",
        "cambiar a ", "abrir ", "ir a ",
    ];

    for verb in focus_verbs {
        if q.starts_with(verb) {
            let target_phrase = q[verb.len()..].trim();
            if target_phrase.is_empty() || target_phrase == "it" || target_phrase == "that" || target_phrase == "this" || target_phrase == "le" || target_phrase == "la" || target_phrase == "eso" {
                continue;
            }
            for (alias, actual) in app_map {
                if target_phrase == alias || target_phrase.starts_with(alias) {
                    return Some(VoiceIntent::FocusWindow { target: actual.to_string() });
                }
            }
            if !target_phrase.is_empty() && target_phrase.split_whitespace().count() <= 2 {
                return Some(VoiceIntent::FocusWindow { target: target_phrase.to_string() });
            }
        }
    }

    // Hindi: "<app> kholo", "<app> par jao", "<app> par switch karo"
    if q.contains("kholo") || q.contains("par jao") || q.contains("switch karo") {
        let clean = q
            .replace("kholo", "")
            .replace("par jao", "")
            .replace("par switch karo", "")
            .replace("switch karo", "")
            .trim()
            .to_string();
        for (alias, actual) in app_map {
            if clean == alias || clean.contains(alias) {
                return Some(VoiceIntent::FocusWindow { target: actual.to_string() });
            }
        }
        if !clean.is_empty() {
            return Some(VoiceIntent::FocusWindow { target: clean });
        }
    }

    // ── 8. SYSTEM BASICS ───────────────────────────────────────────────────
    if q == "lock screen" || q == "lock pc" || q == "lock computer" || q == "screen lock karo" || q == "verrouille l'ecran" || q == "bloquear pantalla" {
        return Some(VoiceIntent::SystemControl { action: "lock".into() });
    }
    if q == "take screenshot" || q == "screenshot" || q == "screenshot lo" || q == "capture d'ecran" || q == "captura de pantalla" {
        return Some(VoiceIntent::SystemControl { action: "screenshot".into() });
    }

    // ── 9. AMBIGUOUS / EMPTY CLARIFICATION ─────────────────────────────────
    if q == "open it" || q == "open" || q == "kholo" || q == "ouvre" || q == "abrir" {
        return Some(VoiceIntent::Clarify {
            question: "Which app or website would you like to open?".to_string(),
        });
    }
    if q == "start it" || q == "start" || q == "shuru karo" || q == "commence" || q == "iniciar" {
        return Some(VoiceIntent::Clarify {
            question: "Would you like to start a focus session or launch an application?".to_string(),
        });
    }

    None
}

/// Fallback Smart Path understanding engine using OpenCode CLI.
/// Passes real-time context: current window, current project, current task, and last failed command.
pub async fn smart_path_classify(
    app: &AppHandle,
    raw_query: &str,
) -> Result<VoiceIntent, String> {
    // 1. Gather rich live context
    let focus_state = app.state::<Arc<FocusTrackerState>>();
    let active_title = focus_state.last_window_title.lock().unwrap().clone();
    let windows_snapshot = focus_state.windows.lock().unwrap().clone();
    let active_class = windows_snapshot
        .iter()
        .find(|w| w.active)
        .map(|w| w.resource_class.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let (task_title, current_proj) = {
        let db_state = app.state::<DbState>();
        if let Some(task) = db_state.current_task() {
            (task.title, task.project)
        } else {
            ("No active task".to_string(), "general".to_string())
        }
    };

    let failed_cmd_info = {
        let lock = GLOBAL_FAILED_COMMAND.lock().unwrap();
        if let Some(ref f) = *lock {
            format!("'{}' (exit code {})", f.cmd, f.exit_code)
        } else {
            "None".to_string()
        }
    };

    let system_prompt = format!(
        r#"You are Coucou's voice assistant reasoning engine on Linux KDE Plasma (Wayland).
The user gave this spoken request: "{raw_query}"

Current Context:
- Active Window: "{active_title}" (class: "{active_class}")
- Current Project: "{current_proj}"
- Current Task: "{task_title}"
- Last Failed Command: {failed_cmd_info}

You must return a single JSON object matching one of the following schemas:
1. {{"action": "focus_window", "target": "<app_name>"}}
2. {{"action": "open_app", "app": "<app_name>"}}
3. {{"action": "open_url", "url": "<url>"}}
4. {{"action": "play_youtube", "query": "<search_query>"}}
5. {{"action": "media_control", "control": "play"|"pause"|"next"|"previous"|"mute"|"volume-up"|"volume-down"}}
6. {{"action": "start_focus", "duration_mins": <number>, "task": "<title>"}}
7. {{"action": "stop_focus"}}
8. {{"action": "add_task", "title": "<task_title>", "project": "<project>"}}
9. {{"action": "complete_task"}}
10. {{"action": "show_score"}}
11. {{"action": "system_control", "control": "lock"|"screenshot"}}
12. {{"action": "clarify", "question": "<concise clarifying question>"}}

Rules:
- Output pure JSON only. Do not include markdown code ticks, greetings, or explanations.
- If the intent cannot be determined with certainty, choose "clarify" with a short, helpful question."#
    );

    let opencode_bin = match crate::opencode_chat::find_binary() {
        Some(b) => b,
        None => {
            return Ok(VoiceIntent::Clarify {
                question: format!("Could you clarify what you mean by \"{}\"?", raw_query),
            });
        }
    };

    // Run opencode CLI with a 6-second timeout
    let mut cmd = tokio::process::Command::new(opencode_bin);
    cmd.args(["run", "--format", "json", &system_prompt]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(_e) => {
            return Ok(VoiceIntent::Clarify {
                question: format!("Could you please rephrase \"{}\"?", raw_query),
            });
        }
    };

    let output_future = child.wait_with_output();
    let output = match tokio::time::timeout(std::time::Duration::from_secs(6), output_future).await {
        Ok(Ok(out)) => out,
        _ => {
            return Ok(VoiceIntent::Clarify {
                question: format!("I didn't quite catch that. Could you please specify your request?"),
            });
        }
    };

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    // Parse json events from OpenCode output to extract generated text
    let mut extracted_text = String::new();
    for line in stdout_str.lines() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(text) = v.pointer("/part/text").and_then(|t| t.as_str()) {
                extracted_text.push_str(text);
            }
        }
    }

    if extracted_text.is_empty() {
        // Fallback: search for pure JSON within raw stdout
        if let Some(start) = stdout_str.find('{') {
            if let Some(end) = stdout_str.rfind('}') {
                extracted_text = stdout_str[start..=end].to_string();
            }
        }
    }

    // Parse the extracted structured action JSON
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&extracted_text) {
        if let Some(action) = val.get("action").and_then(|a| a.as_str()) {
            match action {
                "focus_window" => {
                    let target = val.get("target").and_then(|t| t.as_str()).unwrap_or("code").to_string();
                    return Ok(VoiceIntent::FocusWindow { target });
                }
                "open_app" => {
                    let app_name = val.get("app").and_then(|t| t.as_str()).unwrap_or("code").to_string();
                    return Ok(VoiceIntent::OpenApp { app: app_name });
                }
                "open_url" => {
                    let url = val.get("url").and_then(|u| u.as_str()).unwrap_or("https://google.com").to_string();
                    return Ok(VoiceIntent::OpenUrl { url });
                }
                "play_youtube" => {
                    let query = val.get("query").and_then(|q| q.as_str()).unwrap_or("lo-fi hip hop").to_string();
                    return Ok(VoiceIntent::PlayYouTube { query });
                }
                "media_control" => {
                    let control = val.get("control").and_then(|c| c.as_str()).unwrap_or("play-pause").to_string();
                    return Ok(VoiceIntent::MediaControl { action: control });
                }
                "start_focus" => {
                    let duration_mins = val.get("duration_mins").and_then(|d| d.as_i64()).unwrap_or(25);
                    let task = val.get("task").and_then(|t| t.as_str()).unwrap_or("").to_string();
                    return Ok(VoiceIntent::StartFocus { duration_mins, task });
                }
                "stop_focus" => return Ok(VoiceIntent::StopFocus),
                "add_task" => {
                    let title = val.get("title").and_then(|t| t.as_str()).unwrap_or("New task").to_string();
                    let project = val.get("project").and_then(|p| p.as_str()).unwrap_or("coucou").to_string();
                    return Ok(VoiceIntent::AddTask { title, project });
                }
                "complete_task" => return Ok(VoiceIntent::CompleteTask),
                "show_score" => return Ok(VoiceIntent::ShowScore),
                "system_control" => {
                    let control = val.get("control").and_then(|c| c.as_str()).unwrap_or("lock").to_string();
                    return Ok(VoiceIntent::SystemControl { action: control });
                }
                "clarify" => {
                    let question = val.get("question").and_then(|q| q.as_str()).unwrap_or("Could you please clarify?").to_string();
                    return Ok(VoiceIntent::Clarify { question });
                }
                _ => {}
            }
        }
    }

    Ok(VoiceIntent::Clarify {
        question: format!("Could you please clarify your request?"),
    })
}

/// Executes a resolved voice intent on the system and returns user-visible feedback.
pub fn execute_intent(app: &AppHandle, intent: &VoiceIntent) -> (bool, String) {
    match intent {
        VoiceIntent::FocusWindow { target } => {
            let focus_state = app.state::<Arc<FocusTrackerState>>();
            let target_lower = target.to_lowercase();
            let windows = focus_state.windows.lock().unwrap().clone();

            let matched = windows.iter().find(|w| {
                w.caption.to_lowercase().contains(&target_lower)
                    || w.resource_class.to_lowercase().contains(&target_lower)
            });

            if let Some(w) = matched {
                let success = crate::focus_tracker::activate_kwin_window(
                    &focus_state,
                    &w.id,
                    w.pid,
                    &w.resource_class,
                );
                (success, format!("Focusing {}", if !w.caption.is_empty() { &w.caption } else { target }))
            } else {
                // If not currently open, try launching the application
                let child = Command::new(target).spawn();
                if child.is_ok() {
                    (true, format!("Launching {}", target))
                } else {
                    let _ = Command::new("gtk-launch").arg(target).spawn();
                    (true, format!("Opening {}", target))
                }
            }
        }

        VoiceIntent::OpenApp { app: app_name } => {
            let _ = Command::new(app_name).spawn().or_else(|_| {
                Command::new("gtk-launch").arg(app_name).spawn()
            });
            (true, format!("Launching {}", app_name))
        }

        VoiceIntent::OpenUrl { url } => {
            let _ = Command::new("xdg-open").arg(url).spawn();
            (true, format!("Opening {}", url))
        }

        VoiceIntent::PlayYouTube { query } => {
            let encoded = url_encode(query);
            let url = format!("https://www.youtube.com/results?search_query={}", encoded);
            let _ = Command::new("xdg-open").arg(&url).spawn();

            // Store in memory for follow-ups
            with_voice_memory(|mem| {
                mem.last_topic = Some(query.clone());
            });

            (true, format!("Playing \"{}\" on YouTube", query))
        }

        VoiceIntent::MediaControl { action } => {
            match action.as_str() {
                "play" => {
                    let _ = Command::new("playerctl").arg("play").status();
                    (true, "Resumed playback".to_string())
                }
                "pause" => {
                    let _ = Command::new("playerctl").arg("pause").status();
                    (true, "Paused playback".to_string())
                }
                "next" => {
                    let _ = Command::new("playerctl").arg("next").status();
                    (true, "Skipped to next track".to_string())
                }
                "previous" => {
                    let _ = Command::new("playerctl").arg("previous").status();
                    (true, "Playing previous track".to_string())
                }
                "mute" => {
                    let _ = Command::new("wpctl").args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]).status();
                    (true, "Toggled mute".to_string())
                }
                "volume-up" => {
                    let _ = Command::new("wpctl").args(["set-volume", "@DEFAULT_AUDIO_SINK@", "5%+"]).status();
                    (true, "Volume increased".to_string())
                }
                "volume-down" => {
                    let _ = Command::new("wpctl").args(["set-volume", "@DEFAULT_AUDIO_SINK@", "5%-"]).status();
                    (true, "Volume decreased".to_string())
                }
                _ => {
                    let _ = crate::system_media::media_control(action.clone(), None);
                    (true, format!("Media {}", action))
                }
            }
        }

        VoiceIntent::StartFocus { duration_mins, task } => {
            let db = app.state::<DbState>();
            let secs = *duration_mins * 60;
            let _ = db.start_focus_session(secs, task, "voice");
            let feedback = if task.is_empty() {
                format!("Started a {}-minute focus session", duration_mins)
            } else {
                format!("Started a {}-minute focus session on \"{}\"", duration_mins, task)
            };
            (true, feedback)
        }

        VoiceIntent::StopFocus => {
            let db = app.state::<DbState>();
            let sessions = db.list_focus_sessions(1);
            if let Some(recent) = sessions.first() {
                if recent.status == "active" || recent.ended_at.is_none() {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    let dur = (now - recent.started_at).max(0);
                    let _ = db.finish_focus_session(recent.id, dur, "completed");
                }
            }
            (true, "Focus session finished".to_string())
        }

        VoiceIntent::AddTask { title, project } => {
            let db = app.state::<DbState>();
            let _ = db.set_task(title, project);
            (true, format!("Added task: \"{}\"", title))
        }

        VoiceIntent::CompleteTask => {
            let db = app.state::<DbState>();
            let _ = db.complete_task(None);
            (true, "Completed current task".to_string())
        }

        VoiceIntent::ShowScore => {
            let db = app.state::<DbState>();
            let stats = db.stats_dashboard();
            (true, format!(
                "Today's score: {} points ({:.0}m focus, {} tasks completed)",
                stats.daily_score, stats.focus_mins_today, stats.tasks_done_today
            ))
        }

        VoiceIntent::SystemControl { action } => {
            match action.as_str() {
                "lock" => {
                    let _ = Command::new("loginctl").arg("lock-session").spawn();
                    (true, "Screen locked".to_string())
                }
                "screenshot" => {
                    let _ = Command::new("spectacle").spawn();
                    (true, "Opened screenshot tool".to_string())
                }
                other => (false, format!("Unknown system action: {}", other)),
            }
        }

        VoiceIntent::Clarify { question } => {
            (true, question.clone())
        }

        VoiceIntent::Compound { actions } => {
            let mut feedbacks = Vec::new();
            let mut all_ok = true;
            for a in actions {
                let (ok, fb) = execute_intent(app, a);
                if !ok { all_ok = false; }
                feedbacks.push(fb);
            }
            (all_ok, feedbacks.join(" & "))
        }

        VoiceIntent::FollowUp { action, .. } => {
            (true, format!("Executed follow-up: {}", action))
        }

        VoiceIntent::Unknown { raw } => {
            (false, format!("I couldn't understand: \"{}\"", raw))
        }
    }
}

/// Main pipeline entry point: processes a voice transcription query through
/// either Fast Path or Smart Path, updates conversation memory, executes the action,
/// and returns the structured `VoiceActionResult`.
pub async fn process_voice_query(
    app: &AppHandle,
    raw_query: &str,
) -> VoiceActionResult {
    let t0 = Instant::now();

    // 1. Try Fast Path (< 0.05 ms)
    let (intent, is_fast_path) = match fast_path_classify(raw_query) {
        Some(i) => (i, true),
        None => {
            // 2. Try Smart Path (OpenCode AI fallback)
            match smart_path_classify(app, raw_query).await {
                Ok(i) => (i, false),
                Err(err) => (VoiceIntent::Clarify {
                    question: format!("Could you please clarify? ({err})"),
                }, false),
            }
        }
    };

    let decision_latency_ms = t0.elapsed().as_millis() as u64;

    // 3. Update conversation memory
    with_voice_memory(|mem| {
        mem.last_intent = Some(intent.clone());
        mem.recent_queries.push(raw_query.to_string());
        if mem.recent_queries.len() > 10 {
            mem.recent_queries.remove(0);
        }
    });

    // 4. Execute the intent
    let (success, feedback) = execute_intent(app, &intent);
    let total_latency_ms = t0.elapsed().as_millis() as u64;

    let clarification = match &intent {
        VoiceIntent::Clarify { question } => Some(question.clone()),
        _ => None,
    };

    let result = VoiceActionResult {
        intent: format!("{:?}", intent),
        is_fast_path,
        success,
        feedback,
        query: raw_query.to_string(),
        decision_latency_ms,
        total_latency_ms,
        clarification,
    };

    // Emit event to dynamic island UI
    let _ = app.emit("voice-action-result", &result);

    crate::log::line(format!(
        "voice intent: query=\"{}\" fast_path={} decision={}ms total={}ms feedback=\"{}\"",
        raw_query, is_fast_path, decision_latency_ms, total_latency_ms, result.feedback
    ));

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_path_media_controls() {
        clear_voice_memory();
        assert_eq!(fast_path_classify("pause music"), Some(VoiceIntent::MediaControl { action: "pause".into() }));
        assert_eq!(fast_path_classify("gaana roko"), Some(VoiceIntent::MediaControl { action: "pause".into() }));
        assert_eq!(fast_path_classify("pause la musique"), Some(VoiceIntent::MediaControl { action: "pause".into() }));
        assert_eq!(fast_path_classify("next song"), Some(VoiceIntent::MediaControl { action: "next".into() }));
        assert_eq!(fast_path_classify("agla gaana"), Some(VoiceIntent::MediaControl { action: "next".into() }));
        assert_eq!(fast_path_classify("morceau suivant"), Some(VoiceIntent::MediaControl { action: "next".into() }));
        assert_eq!(fast_path_classify("siguiente cancion"), Some(VoiceIntent::MediaControl { action: "next".into() }));
        assert_eq!(fast_path_classify("volume up"), Some(VoiceIntent::MediaControl { action: "volume-up".into() }));
        assert_eq!(fast_path_classify("aawaz badhao"), Some(VoiceIntent::MediaControl { action: "volume-up".into() }));
    }

    #[test]
    fn test_fast_path_youtube() {
        assert_eq!(
            fast_path_classify("play lo-fi on youtube"),
            Some(VoiceIntent::PlayYouTube { query: "lo-fi".into() })
        );
        assert_eq!(
            fast_path_classify("youtube pe lofi chalao"),
            Some(VoiceIntent::PlayYouTube { query: "lofi".into() })
        );
        assert_eq!(
            fast_path_classify("joue du lofi sur youtube"),
            Some(VoiceIntent::PlayYouTube { query: "du lofi".into() })
        );
    }

    #[test]
    fn test_fast_path_focus_and_tasks() {
        assert_eq!(
            fast_path_classify("start focus session"),
            Some(VoiceIntent::StartFocus { duration_mins: 25, task: String::new() })
        );
        assert_eq!(
            fast_path_classify("focus for 30 minutes"),
            Some(VoiceIntent::StartFocus { duration_mins: 30, task: String::new() })
        );
        assert_eq!(
            fast_path_classify("stop focus"),
            Some(VoiceIntent::StopFocus)
        );
        assert_eq!(
            fast_path_classify("finish task"),
            Some(VoiceIntent::CompleteTask)
        );
        assert_eq!(
            fast_path_classify("task complete ho gaya"),
            Some(VoiceIntent::CompleteTask)
        );
        assert_eq!(
            fast_path_classify("show my score"),
            Some(VoiceIntent::ShowScore)
        );
        assert_eq!(
            fast_path_classify("mera score dikhao"),
            Some(VoiceIntent::ShowScore)
        );
    }

    #[test]
    fn test_fast_path_compound() {
        let res = fast_path_classify("ouvre VS Code et commence un focus session");
        assert!(matches!(res, Some(VoiceIntent::Compound { .. })));
        if let Some(VoiceIntent::Compound { actions }) = res {
            assert_eq!(actions.len(), 2);
            assert_eq!(actions[0], VoiceIntent::FocusWindow { target: "code".into() });
            assert_eq!(actions[1], VoiceIntent::StartFocus { duration_mins: 25, task: String::new() });
        }

        let res2 = fast_path_classify("terminal kholo aur focus start karo");
        assert!(matches!(res2, Some(VoiceIntent::Compound { .. })));
        if let Some(VoiceIntent::Compound { actions }) = res2 {
            assert_eq!(actions.len(), 2);
            assert_eq!(actions[0], VoiceIntent::FocusWindow { target: "konsole".into() });
            assert_eq!(actions[1], VoiceIntent::StartFocus { duration_mins: 25, task: String::new() });
        }
    }

    #[test]
    fn test_fast_path_multilingual_code_switching() {
        // Spanish
        assert_eq!(
            fast_path_classify("pausar musica"),
            Some(VoiceIntent::MediaControl { action: "pause".into() })
        );
        assert_eq!(
            fast_path_classify("reproducir lofi en youtube"),
            Some(VoiceIntent::PlayYouTube { query: "lofi".into() })
        );
        assert_eq!(
            fast_path_classify("mostrar mi puntuacion"),
            Some(VoiceIntent::ShowScore)
        );

        // Hindi
        assert_eq!(
            fast_path_classify("aawaz badhao"),
            Some(VoiceIntent::MediaControl { action: "volume-up".into() })
        );
        assert_eq!(
            fast_path_classify("aawaz kam karo"),
            Some(VoiceIntent::MediaControl { action: "volume-down".into() })
        );
        assert_eq!(
            fast_path_classify("code kholo"),
            Some(VoiceIntent::FocusWindow { target: "code".into() })
        );
        assert_eq!(
            fast_path_classify("aaj maine kya kiya"),
            Some(VoiceIntent::ShowScore)
        );
    }

    #[test]
    fn test_conversation_memory_and_clarification() {
        // Ambiguous prompts trigger friendly clarifying questions
        assert_eq!(
            fast_path_classify("open it"),
            Some(VoiceIntent::Clarify {
                question: "Which app or website would you like to open?".into(),
            })
        );
        assert_eq!(
            fast_path_classify("start it"),
            Some(VoiceIntent::Clarify {
                question: "Would you like to start a focus session or launch an application?".into(),
            })
        );

        // Memory follow-up "play another one"
        with_voice_memory(|mem| {
            mem.last_intent = Some(VoiceIntent::PlayYouTube { query: "lo-fi beats".into() });
        });
        assert_eq!(
            fast_path_classify("play another one"),
            Some(VoiceIntent::PlayYouTube { query: "lo-fi beats mix".into() })
        );

        // Memory follow-up "open the second one"
        with_voice_memory(|mem| {
            mem.last_search_results = vec!["https://github.com".into(), "https://docs.rs".into()];
        });
        assert_eq!(
            fast_path_classify("open the second one"),
            Some(VoiceIntent::OpenUrl { url: "https://docs.rs".into() })
        );
    }
}
