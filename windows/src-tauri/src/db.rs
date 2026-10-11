// Local SQLite store for Coucou tasks, focus sessions, and typed events.
// Uses system libsqlite3 directly via C FFI so builds work offline without external crates.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::State;

#[repr(C)]
struct sqlite3 {
    _private: [u8; 0],
}

#[repr(C)]
struct sqlite3_stmt {
    _private: [u8; 0],
}

const SQLITE_OK: c_int = 0;
const SQLITE_ROW: c_int = 100;
const SQLITE_DONE: c_int = 101;

type SqliteDestructor = Option<unsafe extern "C" fn(*mut c_void)>;

fn sqlite_transient() -> SqliteDestructor {
    unsafe { std::mem::transmute::<isize, SqliteDestructor>(-1isize) }
}

#[cfg(target_os = "linux")]
#[link(name = "sqlite3")]
extern "C" {
    fn sqlite3_open(filename: *const c_char, pp_db: *mut *mut sqlite3) -> c_int;
    fn sqlite3_close(db: *mut sqlite3) -> c_int;
    fn sqlite3_exec(
        db: *mut sqlite3,
        sql: *const c_char,
        callback: Option<
            unsafe extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int,
        >,
        arg: *mut c_void,
        errmsg: *mut *mut c_char,
    ) -> c_int;
    fn sqlite3_free(ptr: *mut c_void);
    fn sqlite3_errmsg(db: *mut sqlite3) -> *const c_char;
    fn sqlite3_prepare_v2(
        db: *mut sqlite3,
        z_sql: *const c_char,
        n_byte: c_int,
        pp_stmt: *mut *mut sqlite3_stmt,
        pz_tail: *mut *const c_char,
    ) -> c_int;
    fn sqlite3_bind_int64(stmt: *mut sqlite3_stmt, idx: c_int, val: i64) -> c_int;
    fn sqlite3_bind_text(
        stmt: *mut sqlite3_stmt,
        idx: c_int,
        val: *const c_char,
        n: c_int,
        destructor: SqliteDestructor,
    ) -> c_int;
    fn sqlite3_step(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_column_int64(stmt: *mut sqlite3_stmt, i_col: c_int) -> i64;
    fn sqlite3_column_text(stmt: *mut sqlite3_stmt, i_col: c_int) -> *const c_char;
    fn sqlite3_finalize(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_last_insert_rowid(db: *mut sqlite3) -> i64;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: i64,
    pub title: String,
    pub project: String,
    pub status: String,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FocusSessionRecord {
    pub id: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub duration_secs: i64,
    pub target_secs: i64,
    pub status: String,
    pub task_title: String,
    pub app_class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BrowserSessionRecord {
    pub id: i64,
    pub domain: String,
    pub category: String,
    pub intent: String,
    pub started_at: i64,
    pub duration_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentWaitRecord {
    pub id: i64,
    pub agent: String,
    pub window_class: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub wait_secs: i64,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyAgentWaitStats {
    pub total_wait_secs: i64,
    pub total_sessions: i64,
    pub antigravity_secs: i64,
    pub kiro_secs: i64,
    pub opencode_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DailyGoalRecord {
    pub id: i64,
    pub day: String,
    pub slot: i64,
    pub title: String,
    pub done: bool,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MorningRecapData {
    pub day: String,
    pub should_show_today: bool,
    pub last_project: String,
    pub last_file: String,
    pub git_branch: String,
    pub unfinished_task: String,
    pub focus_minutes_recent: i64,
    pub tasks_completed_recent: i64,
    pub focus_sessions_recent: i64,
    pub agent_runs_recent: i64,
    pub goals_done: i64,
    pub goals_total: i64,
}

pub struct DbState {
    path: PathBuf,
    lock: Mutex<()>,
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn today_date_string() -> String {
    // Convert unix timestamp to YYYY-MM-DD (UTC/local day bucket)
    let secs = unix_now() + 19_800; // IST (+05:30)
    let days = secs.div_euclid(86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant algorithm)
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{year:04}-{m:02}-{d:02}")
}

pub fn default_db_path() -> PathBuf {
    let dir = crate::platform::local_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("coucou.db")
}

struct Connection {
    raw: *mut sqlite3,
}

unsafe impl Send for Connection {}

impl Connection {
    fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let c_path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|e| format!("invalid db path: {e}"))?;
        let mut raw: *mut sqlite3 = ptr::null_mut();
        let rc = unsafe { sqlite3_open(c_path.as_ptr(), &mut raw) };
        if rc != SQLITE_OK || raw.is_null() {
            if !raw.is_null() {
                unsafe {
                    sqlite3_close(raw);
                }
            }
            return Err(format!("sqlite3_open failed with code {rc}"));
        }
        Ok(Self { raw })
    }

    fn exec(&self, sql: &str) -> Result<(), String> {
        let c_sql = CString::new(sql).map_err(|e| format!("invalid sql string: {e}"))?;
        let mut errmsg: *mut c_char = ptr::null_mut();
        let rc = unsafe {
            sqlite3_exec(
                self.raw,
                c_sql.as_ptr(),
                None,
                ptr::null_mut(),
                &mut errmsg,
            )
        };
        if rc != SQLITE_OK {
            let msg = if !errmsg.is_null() {
                let s = unsafe { CStr::from_ptr(errmsg).to_string_lossy().into_owned() };
                unsafe {
                    sqlite3_free(errmsg as *mut c_void);
                }
                s
            } else {
                format!("sqlite3_exec error {rc}")
            };
            return Err(msg);
        }
        Ok(())
    }

    fn last_error(&self) -> String {
        unsafe {
            let ptr = sqlite3_errmsg(self.raw);
            if ptr.is_null() {
                "unknown sqlite error".to_string()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        }
    }

    fn last_insert_rowid(&self) -> i64 {
        unsafe { sqlite3_last_insert_rowid(self.raw) }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                sqlite3_close(self.raw);
            }
            self.raw = ptr::null_mut();
        }
    }
}

struct Statement<'a> {
    conn: &'a Connection,
    raw: *mut sqlite3_stmt,
}

impl<'a> Statement<'a> {
    fn prepare(conn: &'a Connection, sql: &str) -> Result<Self, String> {
        let c_sql = CString::new(sql).map_err(|e| format!("invalid sql: {e}"))?;
        let mut raw: *mut sqlite3_stmt = ptr::null_mut();
        let rc = unsafe {
            sqlite3_prepare_v2(
                conn.raw,
                c_sql.as_ptr(),
                -1,
                &mut raw,
                ptr::null_mut(),
            )
        };
        if rc != SQLITE_OK || raw.is_null() {
            return Err(format!("prepare failed: {}", conn.last_error()));
        }
        Ok(Self { conn, raw })
    }

    fn bind_i64(&self, idx: c_int, val: i64) -> Result<(), String> {
        let rc = unsafe { sqlite3_bind_int64(self.raw, idx, val) };
        if rc != SQLITE_OK {
            return Err(format!("bind_int64 failed: {}", self.conn.last_error()));
        }
        Ok(())
    }

    fn bind_text(&self, idx: c_int, val: &str) -> Result<(), String> {
        let c_val = CString::new(val).unwrap_or_else(|_| CString::new("").unwrap());
        let rc = unsafe {
            sqlite3_bind_text(
                self.raw,
                idx,
                c_val.as_ptr(),
                -1,
                sqlite_transient(),
            )
        };
        if rc != SQLITE_OK {
            return Err(format!("bind_text failed: {}", self.conn.last_error()));
        }
        Ok(())
    }

    fn step(&self) -> Result<bool, String> {
        let rc = unsafe { sqlite3_step(self.raw) };
        match rc {
            SQLITE_ROW => Ok(true),
            SQLITE_DONE => Ok(false),
            _ => Err(format!("step failed ({rc}): {}", self.conn.last_error())),
        }
    }

    fn col_i64(&self, col: c_int) -> i64 {
        unsafe { sqlite3_column_int64(self.raw, col) }
    }

    fn col_opt_i64(&self, col: c_int) -> Option<i64> {
        let v = self.col_i64(col);
        if v <= 0 { None } else { Some(v) }
    }

    fn col_text(&self, col: c_int) -> String {
        unsafe {
            let ptr = sqlite3_column_text(self.raw, col);
            if ptr.is_null() {
                String::new()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        }
    }
}

impl<'a> Drop for Statement<'a> {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                sqlite3_finalize(self.raw);
            }
            self.raw = ptr::null_mut();
        }
    }
}

impl DbState {
    pub fn new(path: PathBuf) -> Self {
        let state = Self {
            path,
            lock: Mutex::new(()),
        };
        if let Err(err) = state.init_schema() {
            eprintln!("[coucou-db] schema initialization warning: {err}");
        }
        state
    }

    fn init_schema(&self) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|_| "db lock poisoned".to_string())?;
        let conn = Connection::open(&self.path)?;
        conn.exec(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS tasks (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 title TEXT NOT NULL,
                 project TEXT NOT NULL DEFAULT '',
                 status TEXT NOT NULL DEFAULT 'active',
                 created_at INTEGER NOT NULL,
                 completed_at INTEGER
             );
             CREATE TABLE IF NOT EXISTS focus_sessions (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 started_at INTEGER NOT NULL,
                 ended_at INTEGER,
                 duration_secs INTEGER NOT NULL DEFAULT 0,
                 target_secs INTEGER NOT NULL DEFAULT 1500,
                 status TEXT NOT NULL,
                 task_title TEXT NOT NULL DEFAULT '',
                 app_class TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE IF NOT EXISTS browser_sessions (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 domain TEXT NOT NULL,
                 category TEXT NOT NULL DEFAULT 'neutral',
                 intent TEXT NOT NULL DEFAULT 'browse',
                 started_at INTEGER NOT NULL,
                 duration_secs INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS agent_wait_sessions (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 agent TEXT NOT NULL,
                 window_class TEXT NOT NULL DEFAULT '',
                 started_at INTEGER NOT NULL,
                 ended_at INTEGER NOT NULL,
                 wait_secs INTEGER NOT NULL DEFAULT 0,
                 status TEXT NOT NULL DEFAULT 'done',
                 detail TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE IF NOT EXISTS daily_goals (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 day TEXT NOT NULL,
                 slot INTEGER NOT NULL,
                 title TEXT NOT NULL,
                 done INTEGER NOT NULL DEFAULT 0,
                 created_at INTEGER NOT NULL,
                 completed_at INTEGER
             );
             CREATE TABLE IF NOT EXISTS workspace_context (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 last_project TEXT NOT NULL DEFAULT 'Coucou',
                 last_file TEXT NOT NULL DEFAULT 'windows/src-tauri/src/lib.rs',
                 git_branch TEXT NOT NULL DEFAULT 'main',
                 updated_at INTEGER NOT NULL DEFAULT 0,
                 last_recap_day TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE IF NOT EXISTS event_log (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 ts INTEGER NOT NULL,
                 kind TEXT NOT NULL,
                 payload TEXT NOT NULL
             );",
        )?;

        // Seed an initial task if the tasks table is completely empty
        let count_stmt = Statement::prepare(&conn, "SELECT COUNT(*) FROM tasks;")?;
        let total = if count_stmt.step()? { count_stmt.col_i64(0) } else { 0 };
        drop(count_stmt);

        if total == 0 {
            let now = unix_now();
            let ins = Statement::prepare(
                &conn,
                "INSERT INTO tasks (title, project, status, created_at) VALUES (?1, ?2, 'active', ?3);",
            )?;
            ins.bind_text(1, "Deep work session")?;
            ins.bind_text(2, "Coucou")?;
            ins.bind_i64(3, now)?;
            let _ = ins.step()?;
        }

        // Ensure workspace_context singleton row exists
        let ctx_stmt = Statement::prepare(&conn, "SELECT COUNT(*) FROM workspace_context WHERE id = 1;")?;
        let ctx_count = if ctx_stmt.step()? { ctx_stmt.col_i64(0) } else { 0 };
        drop(ctx_stmt);
        if ctx_count == 0 {
            let now = unix_now();
            let ins_ctx = Statement::prepare(
                &conn,
                "INSERT INTO workspace_context (id, last_project, last_file, git_branch, updated_at, last_recap_day)
                 VALUES (1, 'Coucou', 'windows/src-tauri/src/lib.rs', 'main', ?1, '');",
            )?;
            ins_ctx.bind_i64(1, now)?;
            let _ = ins_ctx.step()?;
        }

        // Seed 3 daily goals for today if none exist yet
        let today = today_date_string();
        let goal_count_stmt = Statement::prepare(
            &conn,
            "SELECT COUNT(*) FROM daily_goals WHERE day = ?1;",
        )?;
        goal_count_stmt.bind_text(1, &today)?;
        let goal_count = if goal_count_stmt.step()? {
            goal_count_stmt.col_i64(0)
        } else {
            0
        };
        drop(goal_count_stmt);

        if goal_count == 0 {
            let now = unix_now();
            let defaults = [
                (1_i64, "Ship core feature milestone", 1_i64),
                (2_i64, "Verify Wayland & SQLite tests", 0_i64),
                (3_i64, "Clean git commit & review", 0_i64),
            ];
            for (slot, title, done) in defaults {
                let ins = Statement::prepare(
                    &conn,
                    "INSERT INTO daily_goals (day, slot, title, done, created_at, completed_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
                )?;
                ins.bind_text(1, &today)?;
                ins.bind_i64(2, slot)?;
                ins.bind_text(3, title)?;
                ins.bind_i64(4, done)?;
                ins.bind_i64(5, now)?;
                ins.bind_i64(6, if done == 1 { now } else { 0 })?;
                let _ = ins.step()?;
            }
        }
        Ok(())
    }

    pub fn log_event(&self, kind: &str, payload: &str) {
        let Ok(_guard) = self.lock.lock() else { return };
        let Ok(conn) = Connection::open(&self.path) else { return };
        let Ok(stmt) = Statement::prepare(
            &conn,
            "INSERT INTO event_log (ts, kind, payload) VALUES (?1, ?2, ?3);",
        ) else {
            return;
        };
        let _ = stmt.bind_i64(1, unix_now());
        let _ = stmt.bind_text(2, kind);
        let _ = stmt.bind_text(3, payload);
        let _ = stmt.step();
    }

    pub fn current_task(&self) -> Option<TaskRecord> {
        let _guard = self.lock.lock().ok()?;
        let conn = Connection::open(&self.path).ok()?;
        let stmt = Statement::prepare(
            &conn,
            "SELECT id, title, project, status, created_at, COALESCE(completed_at, 0)
             FROM tasks
             WHERE status = 'active'
             ORDER BY id DESC
             LIMIT 1;",
        )
        .ok()?;
        if stmt.step().ok()? {
            Some(TaskRecord {
                id: stmt.col_i64(0),
                title: stmt.col_text(1),
                project: stmt.col_text(2),
                status: stmt.col_text(3),
                created_at: stmt.col_i64(4),
                completed_at: stmt.col_opt_i64(5),
            })
        } else {
            None
        }
    }

    pub fn set_task(&self, title: &str, project: &str) -> Option<TaskRecord> {
        let clean_title = title.trim();
        if clean_title.is_empty() {
            return self.current_task();
        }
        let clean_project = project.trim();
        let now = unix_now();
        {
            let _guard = self.lock.lock().ok()?;
            let conn = Connection::open(&self.path).ok()?;
            let stmt = Statement::prepare(
                &conn,
                "INSERT INTO tasks (title, project, status, created_at) VALUES (?1, ?2, 'active', ?3);",
            )
            .ok()?;
            stmt.bind_text(1, clean_title).ok()?;
            stmt.bind_text(2, clean_project).ok()?;
            stmt.bind_i64(3, now).ok()?;
            stmt.step().ok()?;
        }
        self.log_event(
            "task_created",
            &serde_json::json!({ "title": clean_title, "project": clean_project }).to_string(),
        );
        self.current_task()
    }

    pub fn complete_task(&self, id: Option<i64>) -> Option<TaskRecord> {
        let target = match id {
            Some(tid) => {
                let _guard = self.lock.lock().ok()?;
                let conn = Connection::open(&self.path).ok()?;
                let stmt = Statement::prepare(
                    &conn,
                    "SELECT id, title, project, status, created_at, COALESCE(completed_at, 0)
                     FROM tasks WHERE id = ?1 LIMIT 1;",
                )
                .ok()?;
                stmt.bind_i64(1, tid).ok()?;
                if stmt.step().ok()? {
                    Some(TaskRecord {
                        id: stmt.col_i64(0),
                        title: stmt.col_text(1),
                        project: stmt.col_text(2),
                        status: stmt.col_text(3),
                        created_at: stmt.col_i64(4),
                        completed_at: stmt.col_opt_i64(5),
                    })
                } else {
                    None
                }
            }
            None => self.current_task(),
        }?;

        let now = unix_now();
        {
            let _guard = self.lock.lock().ok()?;
            let conn = Connection::open(&self.path).ok()?;
            let stmt = Statement::prepare(
                &conn,
                "UPDATE tasks SET status = 'done', completed_at = ?1 WHERE id = ?2;",
            )
            .ok()?;
            stmt.bind_i64(1, now).ok()?;
            stmt.bind_i64(2, target.id).ok()?;
            stmt.step().ok()?;
        }

        self.log_event(
            "task_completed",
            &serde_json::json!({ "id": target.id, "title": target.title }).to_string(),
        );

        Some(TaskRecord {
            status: "done".to_string(),
            completed_at: Some(now),
            ..target
        })
    }

    pub fn list_tasks(&self, limit: i64) -> Vec<TaskRecord> {
        let Ok(_guard) = self.lock.lock() else {
            return Vec::new();
        };
        let Ok(conn) = Connection::open(&self.path) else {
            return Vec::new();
        };
        let Ok(stmt) = Statement::prepare(
            &conn,
            "SELECT id, title, project, status, created_at, COALESCE(completed_at, 0)
             FROM tasks
             ORDER BY id DESC
             LIMIT ?1;",
        ) else {
            return Vec::new();
        };
        let _ = stmt.bind_i64(1, limit.max(1));
        let mut out = Vec::new();
        while stmt.step().unwrap_or(false) {
            out.push(TaskRecord {
                id: stmt.col_i64(0),
                title: stmt.col_text(1),
                project: stmt.col_text(2),
                status: stmt.col_text(3),
                created_at: stmt.col_i64(4),
                completed_at: stmt.col_opt_i64(5),
            });
        }
        out
    }

    pub fn start_focus_session(
        &self,
        target_secs: i64,
        task_title: &str,
        app_class: &str,
    ) -> Option<FocusSessionRecord> {
        let now = unix_now();
        let clean_target = if target_secs <= 0 { 1500 } else { target_secs };
        let row_id = {
            let _guard = self.lock.lock().ok()?;
            let conn = Connection::open(&self.path).ok()?;
            let stmt = Statement::prepare(
                &conn,
                "INSERT INTO focus_sessions (started_at, duration_secs, target_secs, status, task_title, app_class)
                 VALUES (?1, 0, ?2, 'running', ?3, ?4);",
            )
            .ok()?;
            stmt.bind_i64(1, now).ok()?;
            stmt.bind_i64(2, clean_target).ok()?;
            stmt.bind_text(3, task_title.trim()).ok()?;
            stmt.bind_text(4, app_class.trim()).ok()?;
            stmt.step().ok()?;
            conn.last_insert_rowid()
        };

        self.log_event(
            "focus_session_started",
            &serde_json::json!({
                "id": row_id,
                "targetSecs": clean_target,
                "taskTitle": task_title.trim(),
                "appClass": app_class.trim(),
            })
            .to_string(),
        );

        Some(FocusSessionRecord {
            id: row_id,
            started_at: now,
            ended_at: None,
            duration_secs: 0,
            target_secs: clean_target,
            status: "running".to_string(),
            task_title: task_title.trim().to_string(),
            app_class: app_class.trim().to_string(),
        })
    }

    pub fn finish_focus_session(
        &self,
        id: i64,
        duration_secs: i64,
        status: &str,
    ) -> Option<FocusSessionRecord> {
        let now = unix_now();
        let clean_status = match status {
            "completed" | "cancelled" => status,
            _ => "completed",
        };
        let record = {
            let _guard = self.lock.lock().ok()?;
            let conn = Connection::open(&self.path).ok()?;
            let upd = Statement::prepare(
                &conn,
                "UPDATE focus_sessions
                 SET ended_at = ?1, duration_secs = ?2, status = ?3
                 WHERE id = ?4;",
            )
            .ok()?;
            upd.bind_i64(1, now).ok()?;
            upd.bind_i64(2, duration_secs.max(0)).ok()?;
            upd.bind_text(3, clean_status).ok()?;
            upd.bind_i64(4, id).ok()?;
            upd.step().ok()?;
            drop(upd);

            let sel = Statement::prepare(
                &conn,
                "SELECT id, started_at, COALESCE(ended_at, 0), duration_secs, target_secs, status, task_title, app_class
                 FROM focus_sessions
                 WHERE id = ?1 LIMIT 1;",
            )
            .ok()?;
            sel.bind_i64(1, id).ok()?;
            if sel.step().ok()? {
                Some(FocusSessionRecord {
                    id: sel.col_i64(0),
                    started_at: sel.col_i64(1),
                    ended_at: sel.col_opt_i64(2),
                    duration_secs: sel.col_i64(3),
                    target_secs: sel.col_i64(4),
                    status: sel.col_text(5),
                    task_title: sel.col_text(6),
                    app_class: sel.col_text(7),
                })
            } else {
                None
            }
        }?;

        self.log_event(
            "focus_session_finished",
            &serde_json::json!({
                "id": record.id,
                "durationSecs": record.duration_secs,
                "status": record.status,
            })
            .to_string(),
        );

        Some(record)
    }

    pub fn list_focus_sessions(&self, limit: i64) -> Vec<FocusSessionRecord> {
        let Ok(_guard) = self.lock.lock() else {
            return Vec::new();
        };
        let Ok(conn) = Connection::open(&self.path) else {
            return Vec::new();
        };
        let Ok(stmt) = Statement::prepare(
            &conn,
            "SELECT id, started_at, COALESCE(ended_at, 0), duration_secs, target_secs, status, task_title, app_class
             FROM focus_sessions
             ORDER BY id DESC
             LIMIT ?1;",
        ) else {
            return Vec::new();
        };
        let _ = stmt.bind_i64(1, limit.max(1));
        let mut out = Vec::new();
        while stmt.step().unwrap_or(false) {
            out.push(FocusSessionRecord {
                id: stmt.col_i64(0),
                started_at: stmt.col_i64(1),
                ended_at: stmt.col_opt_i64(2),
                duration_secs: stmt.col_i64(3),
                target_secs: stmt.col_i64(4),
                status: stmt.col_text(5),
                task_title: stmt.col_text(6),
                app_class: stmt.col_text(7),
            });
        }
        out
    }

    pub fn record_browser_session(
        &self,
        domain: &str,
        category: &str,
        intent: &str,
        duration_secs: i64,
    ) -> Option<BrowserSessionRecord> {
        let now = unix_now();
        let clean_domain = domain.trim();
        if clean_domain.is_empty() {
            return None;
        }
        let clean_cat = category.trim();
        let clean_intent = intent.trim();
        let row_id = {
            let _guard = self.lock.lock().ok()?;
            let conn = Connection::open(&self.path).ok()?;
            let stmt = Statement::prepare(
                &conn,
                "INSERT INTO browser_sessions (domain, category, intent, started_at, duration_secs)
                 VALUES (?1, ?2, ?3, ?4, ?5);",
            )
            .ok()?;
            stmt.bind_text(1, clean_domain).ok()?;
            stmt.bind_text(2, clean_cat).ok()?;
            stmt.bind_text(3, clean_intent).ok()?;
            stmt.bind_i64(4, now).ok()?;
            stmt.bind_i64(5, duration_secs.max(0)).ok()?;
            stmt.step().ok()?;
            conn.last_insert_rowid()
        };

        self.log_event(
            "browser_session_recorded",
            &serde_json::json!({
                "id": row_id,
                "domain": clean_domain,
                "category": clean_cat,
                "intent": clean_intent,
                "durationSecs": duration_secs.max(0),
            })
            .to_string(),
        );

        Some(BrowserSessionRecord {
            id: row_id,
            domain: clean_domain.to_string(),
            category: clean_cat.to_string(),
            intent: clean_intent.to_string(),
            started_at: now,
            duration_secs: duration_secs.max(0),
        })
    }

    pub fn list_browser_sessions(&self, limit: i64) -> Vec<BrowserSessionRecord> {
        let Ok(_guard) = self.lock.lock() else {
            return Vec::new();
        };
        let Ok(conn) = Connection::open(&self.path) else {
            return Vec::new();
        };
        let Ok(stmt) = Statement::prepare(
            &conn,
            "SELECT id, domain, category, intent, started_at, duration_secs
             FROM browser_sessions
             ORDER BY id DESC
             LIMIT ?1;",
        ) else {
            return Vec::new();
        };
        let _ = stmt.bind_i64(1, limit.max(1));
        let mut out = Vec::new();
        while stmt.step().unwrap_or(false) {
            out.push(BrowserSessionRecord {
                id: stmt.col_i64(0),
                domain: stmt.col_text(1),
                category: stmt.col_text(2),
                intent: stmt.col_text(3),
                started_at: stmt.col_i64(4),
                duration_secs: stmt.col_i64(5),
            });
        }
        out
    }

    pub fn record_agent_wait(
        &self,
        agent: &str,
        window_class: &str,
        started_at: i64,
        wait_secs: i64,
        detail: &str,
    ) -> Option<AgentWaitRecord> {
        let now = unix_now();
        let clean_agent = agent.trim().to_lowercase();
        if clean_agent.is_empty() {
            return None;
        }
        let clean_class = window_class.trim();
        let clean_detail = detail.trim();
        let clean_wait = wait_secs.max(1);
        let clean_start = if started_at > 0 { started_at } else { now - clean_wait };

        let row_id = {
            let _guard = self.lock.lock().ok()?;
            let conn = Connection::open(&self.path).ok()?;
            let stmt = Statement::prepare(
                &conn,
                "INSERT INTO agent_wait_sessions (agent, window_class, started_at, ended_at, wait_secs, status, detail)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'done', ?6);",
            )
            .ok()?;
            stmt.bind_text(1, &clean_agent).ok()?;
            stmt.bind_text(2, clean_class).ok()?;
            stmt.bind_i64(3, clean_start).ok()?;
            stmt.bind_i64(4, now).ok()?;
            stmt.bind_i64(5, clean_wait).ok()?;
            stmt.bind_text(6, clean_detail).ok()?;
            stmt.step().ok()?;
            conn.last_insert_rowid()
        };

        self.log_event(
            "agent_wait_recorded",
            &serde_json::json!({
                "id": row_id,
                "agent": clean_agent,
                "windowClass": clean_class,
                "waitSecs": clean_wait,
                "detail": clean_detail,
            })
            .to_string(),
        );

        Some(AgentWaitRecord {
            id: row_id,
            agent: clean_agent,
            window_class: clean_class.to_string(),
            started_at: clean_start,
            ended_at: now,
            wait_secs: clean_wait,
            status: "done".to_string(),
            detail: clean_detail.to_string(),
        })
    }

    pub fn list_agent_waits(&self, limit: i64) -> Vec<AgentWaitRecord> {
        let Ok(_guard) = self.lock.lock() else {
            return Vec::new();
        };
        let Ok(conn) = Connection::open(&self.path) else {
            return Vec::new();
        };
        let Ok(stmt) = Statement::prepare(
            &conn,
            "SELECT id, agent, window_class, started_at, ended_at, wait_secs, status, detail
             FROM agent_wait_sessions
             ORDER BY id DESC
             LIMIT ?1;",
        ) else {
            return Vec::new();
        };
        let _ = stmt.bind_i64(1, limit.max(1));
        let mut out = Vec::new();
        while stmt.step().unwrap_or(false) {
            out.push(AgentWaitRecord {
                id: stmt.col_i64(0),
                agent: stmt.col_text(1),
                window_class: stmt.col_text(2),
                started_at: stmt.col_i64(3),
                ended_at: stmt.col_i64(4),
                wait_secs: stmt.col_i64(5),
                status: stmt.col_text(6),
                detail: stmt.col_text(7),
            });
        }
        out
    }

    pub fn weekly_agent_wait_stats(&self) -> WeeklyAgentWaitStats {
        let since = unix_now() - 7 * 86_400;
        let Ok(_guard) = self.lock.lock() else {
            return WeeklyAgentWaitStats::default();
        };
        let Ok(conn) = Connection::open(&self.path) else {
            return WeeklyAgentWaitStats::default();
        };
        let Ok(stmt) = Statement::prepare(
            &conn,
            "SELECT agent, COUNT(*), COALESCE(SUM(wait_secs), 0)
             FROM agent_wait_sessions
             WHERE ended_at >= ?1
             GROUP BY agent;",
        ) else {
            return WeeklyAgentWaitStats::default();
        };
        let _ = stmt.bind_i64(1, since);
        let mut stats = WeeklyAgentWaitStats::default();
        while stmt.step().unwrap_or(false) {
            let agent = stmt.col_text(0).to_lowercase();
            let count = stmt.col_i64(1);
            let sum_secs = stmt.col_i64(2);
            stats.total_sessions += count;
            stats.total_wait_secs += sum_secs;
            if agent.contains("antigravity") {
                stats.antigravity_secs += sum_secs;
            } else if agent.contains("kiro") {
                stats.kiro_secs += sum_secs;
            } else if agent.contains("opencode") {
                stats.opencode_secs += sum_secs;
            }
        }
        stats
    }

    pub fn daily_goals_list(&self) -> Vec<DailyGoalRecord> {
        let today = today_date_string();
        let Ok(_guard) = self.lock.lock() else {
            return Vec::new();
        };
        let Ok(conn) = Connection::open(&self.path) else {
            return Vec::new();
        };
        let Ok(stmt) = Statement::prepare(
            &conn,
            "SELECT id, day, slot, title, done, created_at, COALESCE(completed_at, 0)
             FROM daily_goals
             WHERE day = ?1
             ORDER BY slot ASC, id ASC
             LIMIT 3;",
        ) else {
            return Vec::new();
        };
        let _ = stmt.bind_text(1, &today);
        let mut out = Vec::new();
        while stmt.step().unwrap_or(false) {
            out.push(DailyGoalRecord {
                id: stmt.col_i64(0),
                day: stmt.col_text(1),
                slot: stmt.col_i64(2),
                title: stmt.col_text(3),
                done: stmt.col_i64(4) != 0,
                created_at: stmt.col_i64(5),
                completed_at: stmt.col_opt_i64(6),
            });
        }
        out
    }

    pub fn daily_goal_set(&self, slot: i64, title: &str) -> Vec<DailyGoalRecord> {
        let clean_slot = slot.clamp(1, 3);
        let clean_title = title.trim();
        if clean_title.is_empty() {
            return self.daily_goals_list();
        }
        let today = today_date_string();
        let now = unix_now();
        if let Ok(_guard) = self.lock.lock() {
            if let Ok(conn) = Connection::open(&self.path) {
                let existing_id = Statement::prepare(
                    &conn,
                    "SELECT id FROM daily_goals WHERE day = ?1 AND slot = ?2 LIMIT 1;",
                )
                .ok()
                .and_then(|s| {
                    let _ = s.bind_text(1, &today);
                    let _ = s.bind_i64(2, clean_slot);
                    if s.step().unwrap_or(false) {
                        Some(s.col_i64(0))
                    } else {
                        None
                    }
                });

                if let Some(eid) = existing_id {
                    if let Ok(upd) = Statement::prepare(
                        &conn,
                        "UPDATE daily_goals SET title = ?1 WHERE id = ?2;",
                    ) {
                        let _ = upd.bind_text(1, clean_title);
                        let _ = upd.bind_i64(2, eid);
                        let _ = upd.step();
                    }
                } else if let Ok(ins) = Statement::prepare(
                    &conn,
                    "INSERT INTO daily_goals (day, slot, title, done, created_at)
                     VALUES (?1, ?2, ?3, 0, ?4);",
                ) {
                    let _ = ins.bind_text(1, &today);
                    let _ = ins.bind_i64(2, clean_slot);
                    let _ = ins.bind_text(3, clean_title);
                    let _ = ins.bind_i64(4, now);
                    let _ = ins.step();
                }
            }
        }
        self.log_event(
            "daily_goal_set",
            &serde_json::json!({ "slot": clean_slot, "title": clean_title }).to_string(),
        );
        self.daily_goals_list()
    }

    pub fn daily_goal_toggle(&self, id: i64) -> Vec<DailyGoalRecord> {
        let now = unix_now();
        if let Ok(_guard) = self.lock.lock() {
            if let Ok(conn) = Connection::open(&self.path) {
                let current_done = Statement::prepare(
                    &conn,
                    "SELECT done FROM daily_goals WHERE id = ?1 LIMIT 1;",
                )
                .ok()
                .and_then(|s| {
                    let _ = s.bind_i64(1, id);
                    if s.step().unwrap_or(false) {
                        Some(s.col_i64(0))
                    } else {
                        None
                    }
                });

                if let Some(done_val) = current_done {
                    let next_done = if done_val != 0 { 0 } else { 1 };
                    let completed_at = if next_done == 1 { now } else { 0 };
                    if let Ok(upd) = Statement::prepare(
                        &conn,
                        "UPDATE daily_goals SET done = ?1, completed_at = ?2 WHERE id = ?3;",
                    ) {
                        let _ = upd.bind_i64(1, next_done);
                        let _ = upd.bind_i64(2, completed_at);
                        let _ = upd.bind_i64(3, id);
                        let _ = upd.step();
                    }
                }
            }
        }
        self.log_event(
            "daily_goal_toggled",
            &serde_json::json!({ "id": id }).to_string(),
        );
        self.daily_goals_list()
    }

    pub fn mark_morning_recap_shown(&self) -> bool {
        let today = today_date_string();
        let now = unix_now();
        let Ok(_guard) = self.lock.lock() else {
            return false;
        };
        let Ok(conn) = Connection::open(&self.path) else {
            return false;
        };
        let Ok(upd) = Statement::prepare(
            &conn,
            "UPDATE workspace_context SET last_recap_day = ?1, updated_at = ?2 WHERE id = 1;",
        ) else {
            return false;
        };
        let _ = upd.bind_text(1, &today);
        let _ = upd.bind_i64(2, now);
        upd.step().is_ok()
    }

    pub fn morning_recap(&self, force_demo: bool) -> MorningRecapData {
        let today = today_date_string();
        let since_24h = unix_now() - 86_400;

        // Discover live git branch & most recently modified file in workspace if available
        let mut live_branch = String::new();
        let mut live_file = String::new();
        if let Ok(out) = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
        {
            if out.status.success() {
                live_branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
            }
        }
        if let Ok(out) = std::process::Command::new("git")
            .args(["diff", "--name-only", "HEAD~1", "HEAD"])
            .output()
        {
            if out.status.success() {
                if let Some(first_line) = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .find(|l| !l.trim().is_empty())
                {
                    live_file = first_line.trim().to_string();
                }
            }
        }

        let goals = self.daily_goals_list();
        let goals_total = goals.len() as i64;
        let goals_done = goals.iter().filter(|g| g.done).count() as i64;
        let unfinished_task = self
            .current_task()
            .map(|t| t.title)
            .unwrap_or_else(|| "Deep work session".to_string());

        let mut last_project = "Coucou".to_string();
        let mut last_file = "windows/src-tauri/src/lib.rs".to_string();
        let mut git_branch = "main".to_string();
        let mut last_recap_day = String::new();
        let mut focus_secs: i64 = 0;
        let mut focus_sessions: i64 = 0;
        let mut tasks_done: i64 = 0;
        let mut agent_runs: i64 = 0;

        if let Ok(_guard) = self.lock.lock() {
            if let Ok(conn) = Connection::open(&self.path) {
                if let Ok(ctx) = Statement::prepare(
                    &conn,
                    "SELECT last_project, last_file, git_branch, last_recap_day
                     FROM workspace_context WHERE id = 1 LIMIT 1;",
                ) {
                    if ctx.step().unwrap_or(false) {
                        last_project = ctx.col_text(0);
                        last_file = ctx.col_text(1);
                        git_branch = ctx.col_text(2);
                        last_recap_day = ctx.col_text(3);
                    }
                }

                if !live_branch.is_empty() || !live_file.is_empty() {
                    if !live_branch.is_empty() {
                        git_branch = live_branch;
                    }
                    if !live_file.is_empty() {
                        last_file = live_file;
                    }
                    if let Ok(upd) = Statement::prepare(
                        &conn,
                        "UPDATE workspace_context SET last_file = ?1, git_branch = ?2, updated_at = ?3 WHERE id = 1;",
                    ) {
                        let _ = upd.bind_text(1, &last_file);
                        let _ = upd.bind_text(2, &git_branch);
                        let _ = upd.bind_i64(3, unix_now());
                        let _ = upd.step();
                    }
                }

                if let Ok(fs_stmt) = Statement::prepare(
                    &conn,
                    "SELECT COUNT(*), COALESCE(SUM(duration_secs), 0)
                     FROM focus_sessions WHERE started_at >= ?1;",
                ) {
                    let _ = fs_stmt.bind_i64(1, since_24h);
                    if fs_stmt.step().unwrap_or(false) {
                        focus_sessions = fs_stmt.col_i64(0);
                        focus_secs = fs_stmt.col_i64(1);
                    }
                }

                if let Ok(tk_stmt) = Statement::prepare(
                    &conn,
                    "SELECT COUNT(*) FROM tasks WHERE status = 'done' AND COALESCE(completed_at, created_at) >= ?1;",
                ) {
                    let _ = tk_stmt.bind_i64(1, since_24h);
                    if tk_stmt.step().unwrap_or(false) {
                        tasks_done = tk_stmt.col_i64(0);
                    }
                }

                if let Ok(ag_stmt) = Statement::prepare(
                    &conn,
                    "SELECT COUNT(*) FROM agent_wait_sessions WHERE ended_at >= ?1;",
                ) {
                    let _ = ag_stmt.bind_i64(1, since_24h);
                    if ag_stmt.step().unwrap_or(false) {
                        agent_runs = ag_stmt.col_i64(0);
                    }
                }
            }
        }

        let should_show_today = force_demo || last_recap_day != today;

        MorningRecapData {
            day: today,
            should_show_today,
            last_project,
            last_file,
            git_branch,
            unfinished_task,
            focus_minutes_recent: (focus_secs / 60).max(if focus_secs > 0 { 1 } else { 0 }),
            tasks_completed_recent: tasks_done,
            focus_sessions_recent: focus_sessions,
            agent_runs_recent: agent_runs,
            goals_done,
            goals_total,
        }
    }
}

#[tauri::command]
pub fn task_current(db: State<'_, DbState>) -> Option<TaskRecord> {
    db.current_task()
}

#[tauri::command]
pub fn task_set(db: State<'_, DbState>, title: String, project: Option<String>) -> Option<TaskRecord> {
    db.set_task(&title, project.as_deref().unwrap_or("Coucou"))
}

#[tauri::command]
pub fn task_complete(db: State<'_, DbState>, id: Option<i64>) -> Option<TaskRecord> {
    db.complete_task(id)
}

#[tauri::command]
pub fn task_list(db: State<'_, DbState>, limit: Option<i64>) -> Vec<TaskRecord> {
    db.list_tasks(limit.unwrap_or(20))
}

#[tauri::command]
pub fn focus_session_start(
    db: State<'_, DbState>,
    target_secs: Option<i64>,
    task_title: Option<String>,
    app_class: Option<String>,
) -> Option<FocusSessionRecord> {
    db.start_focus_session(
        target_secs.unwrap_or(1500),
        task_title.as_deref().unwrap_or(""),
        app_class.as_deref().unwrap_or(""),
    )
}

#[tauri::command]
pub fn focus_session_finish(
    db: State<'_, DbState>,
    id: i64,
    duration_secs: i64,
    status: String,
) -> Option<FocusSessionRecord> {
    db.finish_focus_session(id, duration_secs, &status)
}

#[tauri::command]
pub fn focus_session_list(db: State<'_, DbState>, limit: Option<i64>) -> Vec<FocusSessionRecord> {
    db.list_focus_sessions(limit.unwrap_or(20))
}

#[tauri::command]
pub fn daily_goals_list(db: State<'_, DbState>) -> Vec<DailyGoalRecord> {
    db.daily_goals_list()
}

#[tauri::command]
pub fn daily_goal_set(db: State<'_, DbState>, slot: i64, title: String) -> Vec<DailyGoalRecord> {
    db.daily_goal_set(slot, &title)
}

#[tauri::command]
pub fn daily_goal_toggle(db: State<'_, DbState>, id: i64) -> Vec<DailyGoalRecord> {
    db.daily_goal_toggle(id)
}

#[tauri::command]
pub fn morning_recap_get(db: State<'_, DbState>, force: Option<bool>) -> MorningRecapData {
    db.morning_recap(force.unwrap_or(false))
}

#[tauri::command]
pub fn morning_recap_dismiss(db: State<'_, DbState>) -> bool {
    db.mark_morning_recap_shown()
}

pub fn handle_cli_args() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(first) = args.first().map(|s| s.as_str()) else {
        return false;
    };
    if !matches!(
        first,
        "--db-inspect"
            | "--db-task-set"
            | "--db-task-done"
            | "--db-focus-demo"
            | "--db-morning-recap"
            | "--db-goal-set"
            | "--db-goal-toggle"
    ) {
        return false;
    }

    let path = default_db_path();
    let db = DbState::new(path.clone());
    match first {
        "--db-task-set" => {
            let title = args.get(1).map(|s| s.as_str()).unwrap_or("Phase 3: Island interactions");
            let project = args.get(2).map(|s| s.as_str()).unwrap_or("Coucou");
            let created = db.set_task(title, project);
            println!("{}", serde_json::to_string_pretty(&created).unwrap_or_default());
        }
        "--db-task-done" => {
            let completed = db.complete_task(None);
            println!("{}", serde_json::to_string_pretty(&completed).unwrap_or_default());
        }
        "--db-focus-demo" => {
            let task_title = db
                .current_task()
                .map(|t| t.title)
                .unwrap_or_else(|| "Phase 3 focus session".to_string());
            if let Some(started) = db.start_focus_session(1500, &task_title, "antigravity") {
                let finished = db.finish_focus_session(started.id, 1500, "completed");
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "started": started,
                        "finished": finished,
                    }))
                    .unwrap_or_default()
                );
            }
        }
        "--db-morning-recap" => {
            let recap = db.morning_recap(true);
            println!("{}", serde_json::to_string_pretty(&recap).unwrap_or_default());
        }
        "--db-goal-set" => {
            let slot = args
                .get(1)
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(1);
            let title = args
                .get(2)
                .map(|s| s.as_str())
                .unwrap_or("Ship Phase 6 daily goals");
            let updated = db.daily_goal_set(slot, title);
            println!("{}", serde_json::to_string_pretty(&updated).unwrap_or_default());
        }
        "--db-goal-toggle" => {
            let id = args
                .get(1)
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(1);
            let updated = db.daily_goal_toggle(id);
            println!("{}", serde_json::to_string_pretty(&updated).unwrap_or_default());
        }
        "--db-inspect" | _ => {
            let out = serde_json::json!({
                "dbPath": path.to_string_lossy(),
                "currentTask": db.current_task(),
                "dailyGoals": db.daily_goals_list(),
                "morningRecap": db.morning_recap(false),
                "tasks": db.list_tasks(10),
                "focusSessions": db.list_focus_sessions(10),
                "browserSessions": db.list_browser_sessions(10),
                "agentWaits": db.list_agent_waits(10),
                "weeklyAgentWaitStats": db.weekly_agent_wait_stats(),
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(name: &str) -> (DbState, PathBuf) {
        let dir = std::env::temp_dir().join(format!("coucou-sqlite-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.db");
        (DbState::new(path.clone()), dir)
    }

    #[test]
    fn seeds_default_task_and_supports_set_and_complete() {
        let (db, dir) = temp_db("tasks");
        let seeded = db.current_task().expect("seeded task");
        assert_eq!(seeded.status, "active");

        let created = db.set_task("Verify KWin focus", "Coucou").expect("created task");
        assert_eq!(created.title, "Verify KWin focus");
        assert_eq!(created.status, "active");

        let done = db.complete_task(Some(created.id)).expect("completed task");
        assert_eq!(done.id, created.id);
        assert_eq!(done.status, "done");
        assert!(done.completed_at.is_some());

        // After completing the latest task, current_task falls back to the earlier active task
        let remaining = db.current_task().expect("remaining seeded task");
        assert_eq!(remaining.id, seeded.id);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn records_focus_sessions_start_and_finish_in_sqlite() {
        let (db, dir) = temp_db("focus");
        let started = db
            .start_focus_session(1500, "Phase 3 focus timer", "antigravity")
            .expect("started session");
        assert_eq!(started.status, "running");
        assert_eq!(started.target_secs, 1500);
        assert_eq!(started.task_title, "Phase 3 focus timer");

        let finished = db
            .finish_focus_session(started.id, 1500, "completed")
            .expect("finished session");
        assert_eq!(finished.id, started.id);
        assert_eq!(finished.duration_secs, 1500);
        assert_eq!(finished.status, "completed");
        assert!(finished.ended_at.is_some());

        let list = db.list_focus_sessions(10);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, started.id);
        assert_eq!(list[0].status, "completed");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn supports_daily_goals_and_morning_recap_in_sqlite() {
        let (db, dir) = temp_db("phase6");
        let goals = db.daily_goals_list();
        assert_eq!(goals.len(), 3);
        assert_eq!(goals.iter().filter(|g| g.done).count(), 1);

        let updated = db.daily_goal_set(2, "Finish Phase 6 morning recap");
        assert_eq!(updated[1].title, "Finish Phase 6 morning recap");

        let toggled = db.daily_goal_toggle(updated[1].id);
        assert!(toggled[1].done);
        assert_eq!(toggled.iter().filter(|g| g.done).count(), 2);

        let recap = db.morning_recap(false);
        assert!(recap.should_show_today);
        assert_eq!(recap.goals_done, 2);
        assert_eq!(recap.goals_total, 3);

        assert!(db.mark_morning_recap_shown());
        let recap_after = db.morning_recap(false);
        assert!(!recap_after.should_show_today);
        let recap_forced = db.morning_recap(true);
        assert!(recap_forced.should_show_today);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
