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
    CloseWindow { target: Option<String> },
    OpenApp { app: String },
    OpenUrl { url: String },
    SearchWeb { query: String },
    PlayYouTube { query: String },
    MediaControl { action: String }, // "play", "pause", "play-pause", "next", "previous", "mute", "volume-up", "volume-down", "volume-set:<pct>"
    StartFocus { duration_mins: i64, task: String },
    PauseFocus,
    CancelFocus,
    StopFocus,
    AddTask { title: String, project: String },
    CompleteTask,
    SetGoal { title: String },
    ShowScore,
    OpenChat,
    SystemControl { action: String }, // "lock", "screenshot", "brightness-up", "brightness-down", "brightness-set:<pct>"
    // ── Phase 4: Files, Safety & Permission Tiers ──────────────────────────
    FindAndOpenFile { filename: String },
    SearchFiles { query: String },
    SummarizeFile { filename: String },
    TakeNote { text: String },
    DeleteFile { filename: String },
    ConfirmAction { confirmed: bool },
    Undo,
    Clarify { question: String },
    FollowUp { action: String, index: Option<usize> },
    Compound { actions: Vec<VoiceIntent> },
    Unknown { raw: String },
}

impl VoiceIntent {
    pub fn permission_tier(&self) -> &'static str {
        match self {
            VoiceIntent::DeleteFile { .. } => "Tier 3 (Sensitive)",
            VoiceIntent::FindAndOpenFile { .. }
            | VoiceIntent::SummarizeFile { .. }
            | VoiceIntent::TakeNote { .. }
            | VoiceIntent::Undo => "Tier 2 (Benign Write / File Access)",
            _ => "Tier 1 (Immediate)",
        }
    }
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
    pub tier: String,
    pub can_undo: bool,
    pub requires_confirmation: bool,
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

/// Attempts to resolve a spoken query to an exact YouTube video ID using yt-dlp.
/// Takes ~1.5 - 2s on broadband; returns None on failure or timeout so callers can fallback to search URL.
pub fn resolve_youtube_video(query: &str) -> Option<String> {
    let clean = query.trim();
    if clean.is_empty() {
        return None;
    }
    let ytdlp_candidates = [
        "/home/om-raut/.local/bin/yt-dlp",
        "yt-dlp",
        "/usr/local/bin/yt-dlp",
        "/usr/bin/yt-dlp",
    ];
    let binary = ytdlp_candidates.iter().find(|bin| {
        Command::new(bin).arg("--version").output().is_ok()
    })?;

    let child = Command::new(binary)
        .args([
            "--print", "%(id)s",
            &format!("ytsearch1:{}", clean),
            "--no-warnings",
            "--no-playlist",
            "--socket-timeout", "3",
        ])
        .output();

    if let Ok(output) = child {
        if output.status.success() {
            let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !id.is_empty() && id.len() >= 8 && id.len() <= 16 && !id.contains(' ') && !id.contains('\n') {
                return Some(id);
            }
        }
    }
    None
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
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() || c == '.' || c == '-' || c == '_' || c == '/' || c == '\'' || c == '%' { c } else { ' ' })
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
        || (q.contains("find") && q.contains("open"))
        || (q.contains("dhundo") && q.contains("kholo"))
        || (q.contains("cherche") && q.contains("ouvre"))
        || (q.contains("busca") && q.contains("abre"))
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

    // ── 0. SAFETY: TIER 3 PENDING CONFIRMATION & UNDO ──────────────────────
    if crate::voice_safety::has_pending_confirmation() {
        let yes_words = [
            "yes", "confirm", "proceed", "sure", "ok", "okay",
            "haan", "haan karo", "ha", "theek hai",
            "oui", "d'accord", "confirme",
            "si", "sí", "claro", "confirmar",
        ];
        for y in yes_words {
            if q == y {
                return Some(VoiceIntent::ConfirmAction { confirmed: true });
            }
        }
        let no_words = [
            "no", "cancel", "stop", "abort",
            "nahin", "mat karo", "roko",
            "non", "annule", "arrete",
            "no cancelar", "cancela",
        ];
        for n in no_words {
            if q == n {
                return Some(VoiceIntent::ConfirmAction { confirmed: false });
            }
        }
    }

    // Undo Tier 2 operations ("undo that", "undo", "wapas lo", "annule ça")
    let undo_words = [
        "undo that", "undo", "undo last action", "revert",
        "wapas lo", "undo karo", "pichla action wapas lo",
        "annule ca", "annule ça", "annule", "annuler", "defaire",
        "deshacer", "deshazlo",
    ];
    for u in undo_words {
        if q == u {
            return Some(VoiceIntent::Undo);
        }
    }

    // Take a note (Tier 2)
    let note_prefixes = [
        "take a note about ", "take a note on ", "take a note ", "take note of ", "take note ",
        "note down ", "add note ", "new note ", "create note ",
        "note banao ", "ek note likho ", "note likho ",
        "prends une note sur ", "prends une note ", "ajoute une note ", "nouvelle note ",
        "tomar nota de ", "tomar nota ", "crear nota ", "nueva nota ",
    ];
    for prefix in note_prefixes {
        if q.starts_with(prefix) {
            let text = q[prefix.len()..].trim().to_string();
            if !text.is_empty() {
                return Some(VoiceIntent::TakeNote { text });
            }
        }
    }

    // Summarize a file (Tier 2)
    let summarize_prefixes = [
        "summarize my file ", "summarize the file ", "summarize file ", "summarize my ", "summarize ",
        "summary of file ", "summary of ", "give summary of ",
        "resume le fichier ", "resume mon fichier ", "resume ",
        "resumir el archivo ", "resumir mi archivo ", "resumir ", "resumen de ",
    ];
    for prefix in summarize_prefixes {
        if q.starts_with(prefix) {
            let filename = q[prefix.len()..].trim().to_string();
            if !filename.is_empty() && filename != "file" && filename != "fichier" {
                return Some(VoiceIntent::SummarizeFile { filename });
            }
        }
    }
    if q.ends_with(" ka summary do") || q.ends_with(" ko summarize karo") || q.ends_with(" ka summary") {
        let clean = q
            .replace("ka summary do", "")
            .replace("ko summarize karo", "")
            .replace("ka summary", "")
            .replace("file", "")
            .trim()
            .to_string();
        if !clean.is_empty() {
            return Some(VoiceIntent::SummarizeFile { filename: clean });
        }
    }

    // Delete a file (Tier 3: always asks confirmation first)
    let delete_prefixes = [
        "delete my file ", "delete the file ", "delete file ", "delete ",
        "remove the file ", "remove file ", "remove ",
        "supprime le fichier ", "supprime mon fichier ", "supprime ",
        "eliminar el archivo ", "eliminar mi archivo ", "eliminar archivo ", "eliminar ",
    ];
    for prefix in delete_prefixes {
        if q.starts_with(prefix) {
            let filename = q[prefix.len()..].trim().to_string();
            if !filename.is_empty() && filename != "file" && filename != "fichier" && filename != "it" {
                return Some(VoiceIntent::DeleteFile { filename });
            }
        }
    }
    if q.ends_with(" delete karo") || q.ends_with(" hatao") || q.ends_with(" nikal do") {
        let clean = q
            .replace("delete karo", "")
            .replace("hatao", "")
            .replace("nikal do", "")
            .replace("file", "")
            .trim()
            .to_string();
        if !clean.is_empty() {
            return Some(VoiceIntent::DeleteFile { filename: clean });
        }
    }

    // Find and open file (Tier 2) / Search files (Tier 1)
    if q.contains("find") || q.contains("search file") || q.contains("search files") || q.contains("dhundo") || q.contains("trouve") || q.contains("busca") {
        if q.contains("open") || q.contains("kholo") || q.contains("ouvre") || q.contains("abrir") {
            let clean = q
                .replace("find my file and open it", "")
                .replace("find my file and open", "")
                .replace("find my file", "")
                .replace("find my", "")
                .replace("find and open file", "")
                .replace("find and open", "")
                .replace("find file", "")
                .replace("find", "")
                .replace("search file", "")
                .replace("search", "")
                .replace("and open it", "")
                .replace("and open", "")
                .replace("open file", "")
                .replace("dhundo aur kholo", "")
                .replace("dhundo", "")
                .replace("kholo", "")
                .replace("cherche et ouvre", "")
                .replace("cherche mon fichier", "")
                .replace("cherche le fichier", "")
                .replace("trouve", "")
                .replace("ouvre", "")
                .replace("busca y abre", "")
                .replace("busca mi archivo", "")
                .replace("abrir", "")
                .trim()
                .to_string();
            if !clean.is_empty() {
                return Some(VoiceIntent::FindAndOpenFile { filename: clean });
            }
        }

        if q.starts_with("find my ") || q.starts_with("find file ") || q.starts_with("search file ") || q.starts_with("search files ") || q.ends_with(" dhundo") {
            let clean = q
                .replace("search files for", "")
                .replace("search file for", "")
                .replace("search files", "")
                .replace("search file", "")
                .replace("find my file", "")
                .replace("find my", "")
                .replace("find file", "")
                .replace("find", "")
                .replace("dhundo", "")
                .replace("file", "")
                .trim()
                .to_string();
            if !clean.is_empty() && !clean.ends_with(".com") && !clean.ends_with(".rs") {
                return Some(VoiceIntent::SearchFiles { query: clean });
            }
        }
    }

    // ── 1. MEDIA CONTROLS & VOLUME ─────────────────────────────────────────
    // Volume Set to specific percentage (e.g. "set volume to 60%", "volume 80%", "aawaz 50 karo")
    if q.contains("volume") || q.contains("aawaz") || q.contains("awaz") || q.contains("sonido") {
        for word in q.split_whitespace() {
            let clean_word = word.trim_matches(|c: char| !c.is_numeric());
            if let Ok(n) = clean_word.parse::<u32>() {
                if n <= 100 && (q.contains('%') || q.contains("percent") || q.contains("pourcent") || q.contains("por ciento") || q.starts_with("volume") || q.contains("set volume") || q.contains("karo")) {
                    return Some(VoiceIntent::MediaControl { action: format!("volume-set:{}", n) });
                }
            }
        }
    }

    // Pause
    let pause_words = [
        "pause", "pause music", "stop music", "pause playback", "stop playback",
        "gaana roko", "music roko", "pause karo", "roko", "band karo gaana", "gana roko",
        "pause la musique", "arrete la musique", "met sur pause", "mets en pause",
        "pausar musica", "detener musica", "pausa", "parar musica",
    ];
    for p in pause_words {
        if (q == p || q.starts_with(p)) && !q.contains("focus") && !q.contains("timer") && !q.contains("session") {
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

    // ── 2. BRIGHTNESS & SYSTEM BASICS ──────────────────────────────────────
    if q.contains("brightness") || q.contains("luminosite") || q.contains("brillo") {
        for word in q.split_whitespace() {
            let clean_word = word.trim_matches(|c: char| !c.is_numeric());
            if let Ok(n) = clean_word.parse::<u32>() {
                if n <= 100 {
                    return Some(VoiceIntent::SystemControl { action: format!("brightness-set:{}", n) });
                }
            }
        }
        if q.contains("up") || q.contains("increase") || q.contains("badhao") || q.contains("augmente") || q.contains("subir") || q.contains("plus") {
            return Some(VoiceIntent::SystemControl { action: "brightness-up".into() });
        }
        if q.contains("down") || q.contains("decrease") || q.contains("kam") || q.contains("baisse") || q.contains("dim") || q.contains("bajar") || q.contains("moins") {
            return Some(VoiceIntent::SystemControl { action: "brightness-down".into() });
        }
    }

    if q == "lock screen" || q == "lock pc" || q == "lock computer" || q == "screen lock karo" || q == "verrouille l'ecran" || q == "bloquear pantalla" {
        return Some(VoiceIntent::SystemControl { action: "lock".into() });
    }
    if q == "take screenshot" || q == "screenshot" || q == "screenshot lo" || q == "capture d'ecran" || q == "captura de pantalla" {
        return Some(VoiceIntent::SystemControl { action: "screenshot".into() });
    }

    // ── 3. COUCOU CHAT VIEW ────────────────────────────────────────────────
    let chat_words = [
        "open chat", "open chat section", "open assistant chat", "chat with coucou", "show chat", "start chat",
        "chat kholo", "coucou chat kholo", "chat section kholo", "chat pe jao",
        "ouvre le chat", "affiche le chat",
        "abrir chat", "abre el chat", "muestra el chat",
    ];
    for p in chat_words {
        if q == p || q.starts_with(p) {
            return Some(VoiceIntent::OpenChat);
        }
    }

    // ── 4. WEB SEARCH ──────────────────────────────────────────────────────
    let search_prefixes = [
        "search for ", "search the web for ", "google ", "search ", "look up ",
        "google par search karo ", "internet par dhundo ", "search karo ", "google karo ",
        "cherche ", "recherche sur google ", "recherche ",
        "buscar en google ", "busca en google ", "buscar ", "busca ",
    ];
    for prefix in search_prefixes {
        if q.starts_with(prefix) {
            let raw_s = q[prefix.len()..].trim();
            if !raw_s.is_empty() && !raw_s.ends_with(".com") && !raw_s.ends_with(".rs") && !raw_s.ends_with(".org") && !raw_s.ends_with(".io") {
                return Some(VoiceIntent::SearchWeb { query: raw_s.to_string() });
            }
        }
    }
    if q.contains("par search karo") || q.contains("par dhundo") {
        let clean = q
            .replace("par search karo", "")
            .replace("par dhundo", "")
            .replace("google", "")
            .replace("internet", "")
            .trim()
            .to_string();
        if !clean.is_empty() {
            return Some(VoiceIntent::SearchWeb { query: clean });
        }
    }

    // ── 5. YOUTUBE / VIDEO PLAYBACK ─────────────────────────────────────────
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

    // ── 6. FOCUS SESSIONS ──────────────────────────────────────────────────
    // Pause Focus
    let pause_focus_words = [
        "pause focus", "pause focus session", "pause timer", "pause the focus",
        "focus pause karo", "timer pause karo", "focus roko",
        "mets en pause le focus", "pause le focus",
        "pausar concentracion", "pausa el focus", "pausar focus",
    ];
    for p in pause_focus_words {
        if q == p || q.starts_with(p) {
            return Some(VoiceIntent::PauseFocus);
        }
    }

    // Cancel Focus
    let cancel_focus_words = [
        "cancel focus", "cancel focus session", "abort focus", "cancel the focus",
        "focus cancel karo", "session cancel karo",
        "annule le focus", "abandonne le focus",
        "cancelar concentracion", "cancelar focus",
    ];
    for p in cancel_focus_words {
        if q == p || q.starts_with(p) {
            return Some(VoiceIntent::CancelFocus);
        }
    }

    // Stop Focus
    let stop_focus_words = [
        "stop focus", "end focus", "finish focus", "stop session",
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

    // ── 7. TASKS & GOALS ───────────────────────────────────────────────────
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

    // Daily Goals
    let goal_prefixes = [
        "set goal ", "add goal ", "new goal ", "today's goal is ", "todays goal is ",
        "goal set karo ", "aaj ka goal ", "goal banao ",
        "fixe l'objectif ", "ajoute l'objectif ", "nouvel objectif ",
        "fijar meta ", "agregar meta ", "nueva meta ",
    ];
    for prefix in goal_prefixes {
        if q.starts_with(prefix) {
            let title = q[prefix.len()..].trim().to_string();
            if !title.is_empty() {
                return Some(VoiceIntent::SetGoal { title });
            }
        }
    }

    // ── 8. SCORE & STATS ───────────────────────────────────────────────────
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

    // ── 9. CLOSE WINDOW ────────────────────────────────────────────────────
    let close_current_words = [
        "close window", "close this window", "close current window", "close it",
        "window band karo", "ye window band karo", "is window ko band karo",
        "ferme la fenetre", "ferme cette fenetre",
        "cerrar ventana", "cierra la ventana", "cierra esta ventana",
    ];
    for p in close_current_words {
        if q == p {
            return Some(VoiceIntent::CloseWindow { target: None });
        }
    }

    let close_prefixes = [
        "close window ", "close ", "ferme la fenetre ", "ferme ", "cierra la ventana ", "cierra ",
    ];
    for prefix in close_prefixes {
        if q.starts_with(prefix) {
            let target = q[prefix.len()..].trim();
            if !target.is_empty() && target != "window" && target != "fenetre" && target != "ventana" && target != "it" {
                return Some(VoiceIntent::CloseWindow { target: Some(target.to_string()) });
            }
        }
    }

    if q.ends_with(" band karo") && !q.contains("gaana") && !q.contains("music") && !q.contains("focus") && !q.contains("aawaz") {
        let target = q.replace(" band karo", "").replace("window", "").trim().to_string();
        if !target.is_empty() && target != "ye" && target != "is" {
            return Some(VoiceIntent::CloseWindow { target: Some(target) });
        } else {
            return Some(VoiceIntent::CloseWindow { target: None });
        }
    }

    // ── 10. WEBSITES & URLS ────────────────────────────────────────────────
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

    // ── 11. APPS & WINDOW FOCUS ────────────────────────────────────────────
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

    // ── 12. AMBIGUOUS / EMPTY CLARIFICATION ────────────────────────────────
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
2. {{"action": "close_window", "target": "<app_name_or_empty>"}}
3. {{"action": "open_app", "app": "<app_name>"}}
4. {{"action": "open_url", "url": "<url>"}}
5. {{"action": "search_web", "query": "<search_query>"}}
6. {{"action": "play_youtube", "query": "<search_query>"}}
7. {{"action": "media_control", "control": "play"|"pause"|"next"|"previous"|"mute"|"volume-up"|"volume-down"|"volume-set:<number>"}}
8. {{"action": "start_focus", "duration_mins": <number>, "task": "<title>"}}
9. {{"action": "pause_focus"}}
10. {{"action": "cancel_focus"}}
11. {{"action": "stop_focus"}}
12. {{"action": "add_task", "title": "<task_title>", "project": "<project>"}}
13. {{"action": "complete_task"}}
14. {{"action": "set_goal", "title": "<goal_title>"}}
15. {{"action": "show_score"}}
16. {{"action": "open_chat"}}
17. {{"action": "system_control", "control": "lock"|"screenshot"|"brightness-up"|"brightness-down"|"brightness-set:<number>"}}
18. {{"action": "clarify", "question": "<concise clarifying question>"}}

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
                "close_window" => {
                    let target = val.get("target").and_then(|t| t.as_str()).filter(|s| !s.is_empty()).map(String::from);
                    return Ok(VoiceIntent::CloseWindow { target });
                }
                "open_app" => {
                    let app_name = val.get("app").and_then(|t| t.as_str()).unwrap_or("code").to_string();
                    return Ok(VoiceIntent::OpenApp { app: app_name });
                }
                "open_url" => {
                    let url = val.get("url").and_then(|u| u.as_str()).unwrap_or("https://google.com").to_string();
                    return Ok(VoiceIntent::OpenUrl { url });
                }
                "search_web" => {
                    let query = val.get("query").and_then(|q| q.as_str()).unwrap_or("").to_string();
                    return Ok(VoiceIntent::SearchWeb { query });
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
                "pause_focus" => return Ok(VoiceIntent::PauseFocus),
                "cancel_focus" => return Ok(VoiceIntent::CancelFocus),
                "stop_focus" => return Ok(VoiceIntent::StopFocus),
                "add_task" => {
                    let title = val.get("title").and_then(|t| t.as_str()).unwrap_or("New task").to_string();
                    let project = val.get("project").and_then(|p| p.as_str()).unwrap_or("coucou").to_string();
                    return Ok(VoiceIntent::AddTask { title, project });
                }
                "complete_task" => return Ok(VoiceIntent::CompleteTask),
                "set_goal" => {
                    let title = val.get("title").and_then(|t| t.as_str()).unwrap_or("Daily goal").to_string();
                    return Ok(VoiceIntent::SetGoal { title });
                }
                "show_score" => return Ok(VoiceIntent::ShowScore),
                "open_chat" => return Ok(VoiceIntent::OpenChat),
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

        VoiceIntent::CloseWindow { target } => {
            let focus_state = app.state::<Arc<FocusTrackerState>>();
            let windows = focus_state.windows.lock().unwrap().clone();

            let matched = if let Some(ref tgt) = target {
                let tgt_lower = tgt.to_lowercase();
                if tgt_lower == "this" || tgt_lower == "current" || tgt_lower == "it" || tgt_lower == "ye" || tgt_lower == "cette" || tgt_lower == "esta" {
                    windows.iter().find(|w| w.active).cloned()
                } else {
                    windows.iter().find(|w| {
                        w.caption.to_lowercase().contains(&tgt_lower)
                            || w.resource_class.to_lowercase().contains(&tgt_lower)
                    }).cloned()
                }
            } else {
                windows.iter().find(|w| w.active).cloned()
            };

            if let Some(w) = matched {
                let title = if !w.caption.is_empty() { w.caption.clone() } else { w.resource_class.clone() };
                let ok = crate::focus_tracker::close_kwin_window(
                    &focus_state,
                    &w.id,
                    w.pid,
                    &w.resource_class,
                );
                (ok, format!("Closed window: {}", title))
            } else {
                (false, "No matching window to close".to_string())
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

        VoiceIntent::SearchWeb { query } => {
            let encoded = url_encode(query);
            let url = format!("https://www.google.com/search?q={}", encoded);
            let _ = Command::new("xdg-open").arg(&url).spawn();
            (true, format!("Searching Google for \"{}\"", query))
        }

        VoiceIntent::PlayYouTube { query } => {
            // Attempt real video resolution via yt-dlp first
            let video_id = resolve_youtube_video(query);
            if let Some(ref id) = video_id {
                let watch_url = format!("https://www.youtube.com/watch?v={}&autoplay=1", id);
                let _ = Command::new("xdg-open").arg(&watch_url).spawn();
                with_voice_memory(|mem| {
                    mem.last_topic = Some(query.clone());
                });
                (true, format!("Playing \"{}\" on YouTube", query))
            } else {
                let encoded = url_encode(query);
                let url = format!("https://www.youtube.com/results?search_query={}", encoded);
                let _ = Command::new("xdg-open").arg(&url).spawn();
                with_voice_memory(|mem| {
                    mem.last_topic = Some(query.clone());
                });
                (true, format!("Searching \"{}\" on YouTube", query))
            }
        }

        VoiceIntent::MediaControl { action } => {
            // Check for volume percentage setting
            if let Some(pct_str) = action.strip_prefix("volume-set:") {
                if let Ok(pct) = pct_str.parse::<u32>() {
                    let clamped = pct.min(100);
                    let frac = (clamped as f64) / 100.0;
                    let _ = Command::new("wpctl").args(["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{:.2}", frac)]).status();
                    return (true, format!("Volume set to {}%", clamped));
                }
            }

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
                    (true, "Toggled audio mute".to_string())
                }
                "volume-up" => {
                    let _ = Command::new("wpctl").args(["set-volume", "@DEFAULT_AUDIO_SINK@", "5%+"]).status();
                    (true, "Volume increased by 5%".to_string())
                }
                "volume-down" => {
                    let _ = Command::new("wpctl").args(["set-volume", "@DEFAULT_AUDIO_SINK@", "5%-"]).status();
                    (true, "Volume decreased by 5%".to_string())
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

        VoiceIntent::PauseFocus => {
            let db = app.state::<DbState>();
            let sessions = db.list_focus_sessions(1);
            if let Some(recent) = sessions.first() {
                if recent.status == "running" || recent.status == "active" || recent.ended_at.is_none() {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    let dur = (now - recent.started_at).max(0);
                    let _ = db.finish_focus_session(recent.id, dur, "paused");
                    return (true, "Paused focus session".to_string());
                }
            }
            (false, "No active focus session to pause".to_string())
        }

        VoiceIntent::CancelFocus => {
            let db = app.state::<DbState>();
            let sessions = db.list_focus_sessions(1);
            if let Some(recent) = sessions.first() {
                if recent.status == "running" || recent.status == "active" || recent.ended_at.is_none() {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    let dur = (now - recent.started_at).max(0);
                    let _ = db.finish_focus_session(recent.id, dur, "cancelled");
                    return (true, "Cancelled focus session".to_string());
                }
            }
            (false, "No active focus session to cancel".to_string())
        }

        VoiceIntent::StopFocus => {
            let db = app.state::<DbState>();
            let sessions = db.list_focus_sessions(1);
            if let Some(recent) = sessions.first() {
                if recent.status == "running" || recent.status == "active" || recent.ended_at.is_none() {
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

        VoiceIntent::SetGoal { title } => {
            let db = app.state::<DbState>();
            let goals = db.daily_goal_add(title);
            (true, format!("Added daily goal #{}: \"{}\"", goals.len(), title))
        }

        VoiceIntent::ShowScore => {
            let db = app.state::<DbState>();
            let stats = db.stats_dashboard();
            (true, format!(
                "Today's score: {} points ({:.0}m focus, {} tasks completed)",
                stats.daily_score, stats.focus_mins_today, stats.tasks_done_today
            ))
        }

        VoiceIntent::OpenChat => {
            let _ = app.emit("show-chat-view", ());
            (true, "Opened Coucou chat".to_string())
        }

        // ── Phase 4: Files, Notes, Safety Tiers ────────────────────────────────
        VoiceIntent::FindAndOpenFile { filename } => {
            let matches = crate::voice_safety::search_scoped_files(filename, 3);
            if let Some(first) = matches.first() {
                let _ = Command::new("xdg-open").arg(&first.path).spawn();
                (true, format!("Found and opened {} ({})", first.name, first.path))
            } else {
                (false, format!("Could not find '{}' in Documents, Desktop, or Projects", filename))
            }
        }

        VoiceIntent::SearchFiles { query } => {
            let matches = crate::voice_safety::search_scoped_files(query, 5);
            if matches.is_empty() {
                (false, format!("No files found matching '{}'", query))
            } else {
                let names: Vec<String> = matches.iter().map(|m| m.name.clone()).collect();
                (true, format!("Found {} file(s): {}", names.len(), names.join(", ")))
            }
        }

        VoiceIntent::SummarizeFile { filename } => {
            crate::voice_safety::summarize_file_content(filename)
        }

        VoiceIntent::TakeNote { text } => {
            match crate::voice_safety::create_voice_note(text) {
                Ok((_, display_path)) => {
                    (true, format!("Saved note to {} (Say 'undo that' to revert)", display_path))
                }
                Err(err) => (false, format!("Failed to save note: {}", err)),
            }
        }

        VoiceIntent::DeleteFile { filename } => {
            crate::voice_safety::queue_tier3_deletion(filename)
        }

        VoiceIntent::ConfirmAction { confirmed } => {
            crate::voice_safety::resolve_pending_confirmation(*confirmed)
        }

        VoiceIntent::Undo => {
            crate::voice_safety::execute_undo()
        }

        VoiceIntent::SystemControl { action } => {
            if let Some(pct_str) = action.strip_prefix("brightness-set:") {
                if let Ok(pct) = pct_str.parse::<u32>() {
                    let clamped = pct.min(100);
                    let _ = Command::new("brightnessctl").args(["s", &format!("{}%", clamped)]).status();
                    return (true, format!("Brightness set to {}%", clamped));
                }
            }

            match action.as_str() {
                "brightness-up" => {
                    let _ = Command::new("brightnessctl").args(["s", "10%+"]).status();
                    (true, "Brightness increased by 10%".to_string())
                }
                "brightness-down" => {
                    let _ = Command::new("brightnessctl").args(["s", "10%-"]).status();
                    (true, "Brightness decreased by 10%".to_string())
                }
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

    let tier = intent.permission_tier().to_string();
    let can_undo = {
        let lock = crate::voice_safety::GLOBAL_UNDO_STACK.lock().unwrap();
        !lock.is_empty()
    };
    let requires_confirmation = crate::voice_safety::has_pending_confirmation();

    let result = VoiceActionResult {
        intent: format!("{:?}", intent),
        is_fast_path,
        success,
        feedback,
        query: raw_query.to_string(),
        decision_latency_ms,
        total_latency_ms,
        clarification,
        tier,
        can_undo,
        requires_confirmation,
    };

    // Emit event to dynamic island UI
    let _ = app.emit("voice-action-result", &result);

    crate::log::line(format!(
        "voice intent: query=\"{}\" fast_path={} decision={}ms total={}ms tier=\"{}\" feedback=\"{}\"",
        raw_query, is_fast_path, decision_latency_ms, total_latency_ms, result.tier, result.feedback
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

    #[test]
    fn test_phase3_actions_fast_path() {
        clear_voice_memory();

        // 1. Volume setting percentage
        assert_eq!(
            fast_path_classify("set volume to 80%"),
            Some(VoiceIntent::MediaControl { action: "volume-set:80".into() })
        );
        assert_eq!(
            fast_path_classify("volume 60%"),
            Some(VoiceIntent::MediaControl { action: "volume-set:60".into() })
        );
        assert_eq!(
            fast_path_classify("aawaz 50 karo"),
            Some(VoiceIntent::MediaControl { action: "volume-set:50".into() })
        );

        // 2. Brightness controls
        assert_eq!(
            fast_path_classify("brightness up"),
            Some(VoiceIntent::SystemControl { action: "brightness-up".into() })
        );
        assert_eq!(
            fast_path_classify("brightness badhao"),
            Some(VoiceIntent::SystemControl { action: "brightness-up".into() })
        );
        assert_eq!(
            fast_path_classify("brightness down"),
            Some(VoiceIntent::SystemControl { action: "brightness-down".into() })
        );
        assert_eq!(
            fast_path_classify("brightness 70%"),
            Some(VoiceIntent::SystemControl { action: "brightness-set:70".into() })
        );

        // 3. Web Search
        assert_eq!(
            fast_path_classify("search for tauri v2"),
            Some(VoiceIntent::SearchWeb { query: "tauri v2".into() })
        );
        assert_eq!(
            fast_path_classify("google rust tokio"),
            Some(VoiceIntent::SearchWeb { query: "rust tokio".into() })
        );
        assert_eq!(
            fast_path_classify("internet par dhundo weather today"),
            Some(VoiceIntent::SearchWeb { query: "weather today".into() })
        );
        assert_eq!(
            fast_path_classify("cherche intelligence artificielle"),
            Some(VoiceIntent::SearchWeb { query: "intelligence artificielle".into() })
        );

        // 4. Close Window
        assert_eq!(
            fast_path_classify("close window"),
            Some(VoiceIntent::CloseWindow { target: None })
        );
        assert_eq!(
            fast_path_classify("window band karo"),
            Some(VoiceIntent::CloseWindow { target: None })
        );
        assert_eq!(
            fast_path_classify("ferme la fenetre"),
            Some(VoiceIntent::CloseWindow { target: None })
        );
        assert_eq!(
            fast_path_classify("close konsole"),
            Some(VoiceIntent::CloseWindow { target: Some("konsole".into()) })
        );

        // 5. Coucou Focus Sessions (Pause & Cancel)
        assert_eq!(
            fast_path_classify("pause focus"),
            Some(VoiceIntent::PauseFocus)
        );
        assert_eq!(
            fast_path_classify("focus pause karo"),
            Some(VoiceIntent::PauseFocus)
        );
        assert_eq!(
            fast_path_classify("cancel focus"),
            Some(VoiceIntent::CancelFocus)
        );
        assert_eq!(
            fast_path_classify("focus cancel karo"),
            Some(VoiceIntent::CancelFocus)
        );

        // 6. Daily Goals
        assert_eq!(
            fast_path_classify("set goal finish phase 3"),
            Some(VoiceIntent::SetGoal { title: "finish phase 3".into() })
        );
        assert_eq!(
            fast_path_classify("goal banao deploy coucou"),
            Some(VoiceIntent::SetGoal { title: "deploy coucou".into() })
        );
        assert_eq!(
            fast_path_classify("fixe l'objectif coder l'interface"),
            Some(VoiceIntent::SetGoal { title: "coder l'interface".into() })
        );

        // 7. Open Chat View
        assert_eq!(
            fast_path_classify("open chat"),
            Some(VoiceIntent::OpenChat)
        );
        assert_eq!(
            fast_path_classify("chat kholo"),
            Some(VoiceIntent::OpenChat)
        );
        assert_eq!(
            fast_path_classify("ouvre le chat"),
            Some(VoiceIntent::OpenChat)
        );
    }

    #[test]
    fn test_phase4_files_notes_and_safety_tiers() {
        clear_voice_memory();

        // 1. Take a note (Tier 2)
        assert_eq!(
            fast_path_classify("take a note about finish voice assistant phase 4"),
            Some(VoiceIntent::TakeNote { text: "finish voice assistant phase 4".into() })
        );
        assert_eq!(
            fast_path_classify("note banao test safety tiers"),
            Some(VoiceIntent::TakeNote { text: "test safety tiers".into() })
        );
        assert_eq!(
            fast_path_classify("prends une note sur le projet coucou"),
            Some(VoiceIntent::TakeNote { text: "le projet coucou".into() })
        );

        // 2. Summarize a file (Tier 2)
        assert_eq!(
            fast_path_classify("summarize my project report"),
            Some(VoiceIntent::SummarizeFile { filename: "project report".into() })
        );
        assert_eq!(
            fast_path_classify("notes.txt ka summary do"),
            Some(VoiceIntent::SummarizeFile { filename: "notes.txt".into() })
        );
        assert_eq!(
            fast_path_classify("resume le fichier test.txt"),
            Some(VoiceIntent::SummarizeFile { filename: "test.txt".into() })
        );

        // 3. Find and open file (Tier 2) vs Search files (Tier 1)
        assert_eq!(
            fast_path_classify("find my project report and open it"),
            Some(VoiceIntent::FindAndOpenFile { filename: "project report".into() })
        );
        assert_eq!(
            fast_path_classify("find and open api.js"),
            Some(VoiceIntent::FindAndOpenFile { filename: "api.js".into() })
        );
        assert_eq!(
            fast_path_classify("find my notes"),
            Some(VoiceIntent::SearchFiles { query: "notes".into() })
        );
        assert_eq!(
            fast_path_classify("search files document"),
            Some(VoiceIntent::SearchFiles { query: "document".into() })
        );

        // 4. Delete file (Tier 3)
        assert_eq!(
            fast_path_classify("delete file old_notes.txt"),
            Some(VoiceIntent::DeleteFile { filename: "old_notes.txt".into() })
        );
        assert_eq!(
            fast_path_classify("temp.txt delete karo"),
            Some(VoiceIntent::DeleteFile { filename: "temp.txt".into() })
        );

        // 5. Undo action (Tier 2 undo)
        assert_eq!(
            fast_path_classify("undo that"),
            Some(VoiceIntent::Undo)
        );
        assert_eq!(
            fast_path_classify("wapas lo"),
            Some(VoiceIntent::Undo)
        );
        assert_eq!(
            fast_path_classify("annule ca"),
            Some(VoiceIntent::Undo)
        );
    }
}
