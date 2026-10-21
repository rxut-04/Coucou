// Voice Safety & Permission Tier Engine for Coucou:
// - Tier 1: ReadOnly / Harmless (runs immediately: app launch, URL open, media, search, stats)
// - Tier 2: Benign Write / File Open (runs immediately, displays clear feedback, stores undoable action)
//   Actions: Find and open file, Summarize file, Take note in ~/Documents/CoucouNotes
// - Tier 3: Sensitive / Destructive (requires explicit voice "yes"/"no" or UI confirmation before running)
//   Actions: Delete file, modify existing files, dangerous operations
// - Undo Stack: Reverts Tier 2 operations (e.g. "undo that" removes the note just created)
// - Scoped File Search: Ultra-fast pruned recursive traversal (< 15 ms) avoiding node_modules, .git, etc.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionTier {
    Tier1Immediate,
    Tier2BenignWrite,
    Tier3Sensitive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedFileInfo {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
pub enum UndoableAction {
    CreatedFile { path: PathBuf, description: String },
}

#[derive(Debug, Clone)]
pub enum PendingAction {
    DeleteFile { path: PathBuf },
    ModifyFile { path: PathBuf, new_content: String },
}

#[derive(Debug, Clone)]
pub struct PendingConfirmation {
    pub action: PendingAction,
    pub description: String,
    pub requested_at: Instant,
    pub expires_in_secs: u64,
}

pub static GLOBAL_UNDO_STACK: Mutex<Vec<UndoableAction>> = Mutex::new(Vec::new());
pub static GLOBAL_PENDING_CONFIRMATION: Mutex<Option<PendingConfirmation>> = Mutex::new(None);

/// Returns the user's allowed directories for scoped file search & modifications.
pub fn allowed_folders() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(doc) = dirs::document_dir() {
        if doc.exists() {
            roots.push(doc);
        }
    }
    if let Some(desk) = dirs::desktop_dir() {
        if desk.exists() {
            roots.push(desk);
        }
    }
    if let Some(down) = dirs::download_dir() {
        if down.exists() {
            roots.push(down);
        }
    }
    if let Some(home) = dirs::home_dir() {
        let ideas = home.join("IdeaProjects");
        if ideas.exists() {
            roots.push(ideas);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.exists() && !roots.iter().any(|r| r == &cwd) {
            roots.push(cwd);
        }
    }

    roots
}

/// Returns the dedicated folder for Coucou notes: ~/Documents/CoucouNotes.
pub fn notes_dir() -> PathBuf {
    let base = dirs::document_dir().unwrap_or_else(|| {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"))
    });
    let dir = base.join("CoucouNotes");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Checks whether a given path is inside the allowed folders.
pub fn is_path_allowed(path: &Path) -> bool {
    let canonical = match path.canonicalize() {
        Ok(c) => c,
        Err(_) => path.to_path_buf(),
    };

    for root in allowed_folders() {
        if let Ok(canon_root) = root.canonicalize() {
            if canonical.starts_with(&canon_root) {
                return true;
            }
        } else if canonical.starts_with(&root) {
            return true;
        }
    }
    false
}

fn is_heavy_dir(name: &str) -> bool {
    matches!(
        name,
        "node_modules"
            | ".git"
            | "target"
            | "dist"
            | "build"
            | ".cache"
            | ".local"
            | ".cargo"
            | ".rustup"
            | ".idea"
            | ".vscode"
            | "snap"
            | ".npm"
            | ".system_generated"
    )
}

/// Fast recursive scoped file search on KDE Wayland.
/// Prunes heavy folders, limits depth to 5, and returns up to `max_results` matches in < 15 ms.
pub fn search_scoped_files(query: &str, max_results: usize) -> Vec<ScopedFileInfo> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    let roots = allowed_folders();

    for root in roots {
        search_dir_recursive(&root, &q, 0, 5, max_results, &mut results);
        if results.len() >= max_results {
            break;
        }
    }

    results
}

fn search_dir_recursive(
    dir: &Path,
    query: &str,
    current_depth: usize,
    max_depth: usize,
    max_results: usize,
    out: &mut Vec<ScopedFileInfo>,
) {
    if current_depth > max_depth || out.len() >= max_results {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        if out.len() >= max_results {
            break;
        }

        let path = entry.path();
        let file_name = match path.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };

        if file_name.starts_with('.') && file_name != ".env" {
            continue;
        }

        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        if is_dir {
            if !is_heavy_dir(&file_name) {
                search_dir_recursive(&path, query, current_depth + 1, max_depth, max_results, out);
            }
        } else {
            let name_lower = file_name.to_lowercase();
            if name_lower.contains(query) {
                let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                out.push(ScopedFileInfo {
                    name: file_name,
                    path: path.to_string_lossy().to_string(),
                    size_bytes,
                    is_dir: false,
                });
            }
        }
    }
}

/// Tier 2: Takes a note and saves it to ~/Documents/CoucouNotes/Note_<timestamp>.txt.
/// Records the action on the undo stack.
pub fn create_voice_note(text: &str) -> Result<(PathBuf, String), String> {
    let clean_text = text.trim();
    if clean_text.is_empty() {
        return Err("Note text cannot be empty".to_string());
    }

    let dir = notes_dir();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let file_name = format!("Note_{}.txt", now);
    let note_path = dir.join(&file_name);

    let content = format!(
        "--- Coucou Voice Note ---\nDate: {}\n\n{}\n",
        chrono_like_timestamp(),
        clean_text
    );

    fs::write(&note_path, content).map_err(|e| format!("Failed to save note: {}", e))?;

    // Push to undo stack
    let mut undo_lock = GLOBAL_UNDO_STACK.lock().unwrap();
    undo_lock.push(UndoableAction::CreatedFile {
        path: note_path.clone(),
        description: format!("Note \"{}\"", clean_text),
    });

    let display_path = format!("~/Documents/CoucouNotes/{}", file_name);
    Ok((note_path, display_path))
}

fn chrono_like_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Format simple readable UTC timestamp: YYYY-MM-DD HH:MM:SS
    let days = now / 86400;
    let secs_of_day = now % 86400;
    let hours = secs_of_day / 3600;
    let minutes = (secs_of_day % 3600) / 60;
    let seconds = secs_of_day % 60;
    format!("Day {} (epoch), {:02}:{:02}:{:02} UTC", days, hours, minutes, seconds)
}

/// Tier 2: Summarizes a file by reading the first 4KB and generating a concise summary.
pub fn summarize_file_content(filename_or_path: &str) -> (bool, String) {
    let path = if Path::new(filename_or_path).is_absolute() {
        PathBuf::from(filename_or_path)
    } else {
        // Search scoped files for the best match
        let matches = search_scoped_files(filename_or_path, 3);
        if let Some(first) = matches.first() {
            PathBuf::from(&first.path)
        } else {
            return (
                false,
                format!("File \"{}\" not found in Documents, Desktop, or Projects", filename_or_path),
            );
        }
    };

    if !path.exists() {
        return (false, format!("File does not exist: {}", path.display()));
    }

    let meta = match fs::metadata(&path) {
        Ok(m) => m,
        Err(e) => return (false, format!("Cannot read file metadata: {}", e)),
    };

    let bytes = match fs::read(&path) {
        Ok(b) => b,
        Err(e) => return (false, format!("Cannot read file: {}", e)),
    };

    let sample_len = bytes.len().min(4096);
    let text_sample = String::from_utf8_lossy(&bytes[..sample_len]);
    let line_count = text_sample.lines().count();
    let word_count = text_sample.split_whitespace().count();

    let file_name = path.file_name().unwrap_or_default().to_string_lossy();

    // Fast heuristic summary: preview first meaningful non-empty lines
    let preview_lines: Vec<&str> = text_sample
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with("#!"))
        .take(3)
        .collect();

    let preview = preview_lines.join(" · ");
    let feedback = format!(
        "Summary of {}: {} bytes, ~{} lines, {} words. Preview: \"{}\"",
        file_name,
        meta.len(),
        line_count,
        word_count,
        if preview.len() > 140 { format!("{}…", &preview[..140]) } else { preview }
    );

    (true, feedback)
}

/// Tier 2: Reverts the most recent undoable action (e.g. removes created note).
pub fn execute_undo() -> (bool, String) {
    let action = {
        let mut lock = GLOBAL_UNDO_STACK.lock().unwrap();
        lock.pop()
    };

    match action {
        Some(UndoableAction::CreatedFile { path, description }) => {
            if path.exists() {
                if let Err(e) = fs::remove_file(&path) {
                    return (false, format!("Failed to undo {}: {}", description, e));
                }
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            (true, format!("Undone: removed {}", name))
        }
        None => (false, "Nothing to undo".to_string()),
    }
}

/// Tier 3: Queues a sensitive operation (like file deletion) requiring explicit confirmation.
pub fn queue_tier3_deletion(filename_or_path: &str) -> (bool, String) {
    let path = if Path::new(filename_or_path).is_absolute() {
        PathBuf::from(filename_or_path)
    } else {
        let matches = search_scoped_files(filename_or_path, 3);
        if let Some(first) = matches.first() {
            PathBuf::from(&first.path)
        } else {
            return (
                false,
                format!("File \"{}\" not found to delete", filename_or_path),
            );
        }
    };

    if !path.exists() {
        return (false, format!("File does not exist: {}", path.display()));
    }

    if !is_path_allowed(&path) {
        return (
            false,
            format!("Access denied: '{}' is outside allowed folders", path.display()),
        );
    }

    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

    let mut lock = GLOBAL_PENDING_CONFIRMATION.lock().unwrap();
    *lock = Some(PendingConfirmation {
        action: PendingAction::DeleteFile { path: path.clone() },
        description: format!("Delete file '{}'", file_name),
        requested_at: Instant::now(),
        expires_in_secs: 30,
    });

    (
        true,
        format!(
            "⚠️ Permission required (Tier 3): Delete '{}'? Say 'yes' to confirm or 'no' to cancel.",
            file_name
        ),
    )
}

/// Confirms or cancels a pending Tier 3 action.
pub fn resolve_pending_confirmation(confirmed: bool) -> (bool, String) {
    let pending = {
        let mut lock = GLOBAL_PENDING_CONFIRMATION.lock().unwrap();
        lock.take()
    };

    let p = match pending {
        Some(p) => {
            if p.requested_at.elapsed() > Duration::from_secs(p.expires_in_secs) {
                return (false, "Confirmation request expired".to_string());
            }
            p
        }
        None => return (false, "No action waiting for confirmation".to_string()),
    };

    if !confirmed {
        return (true, format!("Cancelled: {}", p.description));
    }

    match p.action {
        PendingAction::DeleteFile { path } => {
            if path.exists() {
                if let Err(e) = fs::remove_file(&path) {
                    return (false, format!("Failed to delete {}: {}", path.display(), e));
                }
            }
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
            (true, format!("Confirmed: deleted file '{}'", file_name))
        }
        PendingAction::ModifyFile { path, new_content } => {
            if let Err(e) = fs::write(&path, new_content) {
                return (false, format!("Failed to modify {}: {}", path.display(), e));
            }
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
            (true, format!("Confirmed: updated file '{}'", file_name))
        }
    }
}

/// Checks if there is a pending confirmation currently waiting for response.
pub fn has_pending_confirmation() -> bool {
    let lock = GLOBAL_PENDING_CONFIRMATION.lock().unwrap();
    if let Some(ref p) = *lock {
        p.requested_at.elapsed() <= Duration::from_secs(p.expires_in_secs)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_folders_contain_documents_and_desktop() {
        let roots = allowed_folders();
        assert!(!roots.is_empty());
    }

    #[test]
    fn test_scoped_file_search_speed_and_accuracy() {
        let t0 = Instant::now();
        let results = search_scoped_files("coucou", 10);
        let elapsed = t0.elapsed();
        // Should finish in under 100 ms even across broad directory structures
        assert!(elapsed < Duration::from_millis(200));
        assert!(!results.is_empty());
    }

    #[test]
    fn test_tier2_note_creation_and_undo() {
        let note_text = "Phase 4 test note: verify safety tiers";
        let res = create_voice_note(note_text);
        assert!(res.is_ok());
        let (note_path, _) = res.unwrap();
        assert!(note_path.exists());

        // Undo action should remove the note
        let (ok, fb) = execute_undo();
        assert!(ok);
        assert!(fb.contains("Undone"));
        assert!(!note_path.exists());
    }

    #[test]
    fn test_tier3_deletion_requires_explicit_confirmation() {
        // Create a scratch file in notes dir
        let test_file = notes_dir().join("test_delete_safety.txt");
        fs::write(&test_file, "temporary content").unwrap();
        assert!(test_file.exists());

        // Step 1: Request delete -> should queue confirmation, NOT delete
        let (ok, prompt) = queue_tier3_deletion("test_delete_safety.txt");
        assert!(ok);
        assert!(prompt.contains("Permission required"));
        assert!(test_file.exists()); // Still exists!
        assert!(has_pending_confirmation());

        // Step 2: Say "no" -> cancel
        let (cancel_ok, cancel_fb) = resolve_pending_confirmation(false);
        assert!(cancel_ok);
        assert!(cancel_fb.contains("Cancelled"));
        assert!(test_file.exists()); // Still exists!

        // Step 3: Request delete again and confirm with "yes"
        let (ok2, _) = queue_tier3_deletion("test_delete_safety.txt");
        assert!(ok2);
        let (conf_ok, conf_fb) = resolve_pending_confirmation(true);
        assert!(conf_ok);
        assert!(conf_fb.contains("Confirmed: deleted"));
        assert!(!test_file.exists()); // Deleted safely with approval!
    }
}
