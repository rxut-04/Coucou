// Timer engine - pure logic, no I/O, no sleeping.
//
// Design: every public method first calls `advance(now)`, which attributes the
// time elapsed since the previous call to whatever state we were in *before*
// this event. Each app has its OWN tracker, so switching VS Code -> Browser ->
// VS Code never wipes either tracker (the old single `last_focus` slot did).

use crate::classify::AppType as App;
use std::time::{Duration, Instant};

/// All durations are real `Duration`s so COUCOU_FAST scaling is exact.
#[derive(Debug, Clone)]
pub struct Config {
    /// Accumulated VS Code focus before a break notification.
    pub vscode_work: Duration,
    /// Time away from VS Code (or screen locked) that counts as a real break.
    pub vscode_break_reset: Duration,
    /// Continuous browser time before the first nudge.
    pub browser_alert: Duration,
    /// Time between repeated nudges while still browsing.
    pub browser_repeat: Duration,
    /// Max time away from the browser that still counts as "continuous".
    pub browser_grace: Duration,
    /// Terminal commands shorter than this are ignored.
    pub terminal_min: Duration,
    /// Auto-dismiss time for popups.
    pub popup: Duration,
    /// A gap between calls larger than this (suspend, stalled tick loop) is
    /// treated as "away", never as focus time. NOT affected by `scaled`.
    pub max_gap: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vscode_work: Duration::from_secs(25 * 60),
            vscode_break_reset: Duration::from_secs(5 * 60),
            browser_alert: Duration::from_secs(30 * 60),
            browser_repeat: Duration::from_secs(10 * 60),
            browser_grace: Duration::from_secs(10),
            terminal_min: Duration::from_secs(30),
            popup: Duration::from_secs(10),
            max_gap: Duration::from_secs(120),
        }
    }
}

impl Config {
    /// Scale every timing by `factor` (use 1.0/60.0 for COUCOU_FAST=1).
    pub fn scaled(&self, factor: f64) -> Self {
        let s = |d: Duration| d.mul_f64(factor);
        Self {
            vscode_work: s(self.vscode_work),
            vscode_break_reset: s(self.vscode_break_reset),
            browser_alert: s(self.browser_alert),
            browser_repeat: s(self.browser_repeat),
            browser_grace: s(self.browser_grace),
            terminal_min: s(self.terminal_min),
            popup: s(self.popup),
            max_gap: self.max_gap,
        }
    }

    /// Build the config from the environment: COUCOU_FAST=1 => everything / 60.
    pub fn from_env() -> Self {
        let base = Self::default();
        match std::env::var("COUCOU_FAST").as_deref() {
            Ok("1") | Ok("true") => base.scaled(1.0 / 60.0),
            _ => base,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NotificationKind {
    VsCodeBreak,
    BrowserNudge,
    TerminalDone,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Notification {
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
    pub dismiss_after_secs: Option<u64>,
}

#[derive(Debug, Default)]
struct VsCodeTracker {
    /// Focused time since the last break notification / real break.
    accumulated: Duration,
    /// Consecutive time spent NOT in VS Code (or locked).
    away: Duration,
}

#[derive(Debug, Default)]
struct BrowserTracker {
    /// Continuous time since the last nudge (or since the session began).
    continuous: Duration,
    /// Total time in this browsing session (for the notification text).
    session: Duration,
    /// Consecutive time spent NOT in the browser (or locked).
    away: Duration,
    /// Nudges already sent in this session.
    alerts: u32,
}

pub struct Engine {
    config: Config,
    current: Option<App>,
    last_instant: Option<Instant>,
    locked: bool,
    vscode: VsCodeTracker,
    browser: BrowserTracker,
}

fn human(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 120 {
        format!("{} minutes", s / 60)
    } else {
        format!("{} seconds", s)
    }
}

impl Engine {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            current: None,
            last_instant: None,
            locked: false,
            vscode: VsCodeTracker::default(),
            browser: BrowserTracker::default(),
        }
    }

    // ---- core accounting -------------------------------------------------

    /// Attribute the time since the previous call to the state we were in.
    fn advance(&mut self, now: Instant) {
        let Some(prev) = self.last_instant else {
            self.last_instant = Some(now);
            return;
        };
        self.last_instant = Some(now);
        let dt = now.saturating_duration_since(prev);
        if dt.is_zero() {
            return;
        }

        // Locked, or an implausibly long gap (suspend): nobody was focused.
        let focused = if self.locked || dt > self.config.max_gap {
            None
        } else {
            self.current
        };

        match focused {
            Some(App::VsCode) => {
                self.vscode.accumulated += dt;
                self.vscode.away = Duration::ZERO;
                self.browser_away(dt);
            }
            Some(App::Browser) => {
                self.browser.continuous += dt;
                self.browser.session += dt;
                self.browser.away = Duration::ZERO;
                self.vscode_away(dt);
            }
            _ => {
                self.vscode_away(dt);
                self.browser_away(dt);
            }
        }
    }

    fn vscode_away(&mut self, dt: Duration) {
        self.vscode.away += dt;
        if self.vscode.away >= self.config.vscode_break_reset {
            self.vscode.accumulated = Duration::ZERO; // a real break happened
        }
    }

    fn browser_away(&mut self, dt: Duration) {
        self.browser.away += dt;
        if self.browser.away > self.config.browser_grace {
            self.browser = BrowserTracker {
                away: self.browser.away,
                ..BrowserTracker::default()
            };
        }
    }

    fn popup_secs(&self) -> u64 {
        (self.config.popup.as_secs_f64().ceil() as u64).max(1)
    }

    // ---- events ----------------------------------------------------------

    pub fn on_focus(&mut self, app: App, now: Instant) {
        self.advance(now);
        self.current = Some(app);
    }

    /// Call about once per second.
    pub fn on_tick(&mut self, now: Instant) -> Vec<Notification> {
        self.advance(now);
        let mut out = Vec::new();
        if self.locked {
            return out;
        }

        match self.current {
            Some(App::VsCode) => {
                if self.vscode.accumulated >= self.config.vscode_work {
                    out.push(Notification {
                        kind: NotificationKind::VsCodeBreak,
                        title: "5 minutes break time".to_string(),
                        body: format!(
                            "You've been coding for {}. Take a break!",
                            human(self.vscode.accumulated)
                        ),
                        dismiss_after_secs: Some(self.popup_secs()),
                    });
                    self.vscode.accumulated = Duration::ZERO;
                }
            }
            Some(App::Browser) => {
                let threshold = if self.browser.alerts == 0 {
                    self.config.browser_alert
                } else {
                    self.config.browser_repeat
                };
                if self.browser.continuous >= threshold {
                    out.push(Notification {
                        kind: NotificationKind::BrowserNudge,
                        title: "Go back to VS Code".to_string(),
                        body: format!("You've been browsing for {}.", human(self.browser.session)),
                        dismiss_after_secs: Some(self.popup_secs()),
                    });
                    self.browser.alerts += 1;
                    self.browser.continuous = Duration::ZERO;
                }
            }
            _ => {}
        }
        out
    }

    /// Global: fires regardless of which app is focused.
    pub fn on_terminal_finished(
        &mut self,
        duration_secs: f64,
        cmd: String,
        exit_code: i32,
        _now: Instant,
    ) -> Option<Notification> {
        if duration_secs >= self.config.terminal_min.as_secs_f64() {
            Some(Notification {
                kind: NotificationKind::TerminalDone,
                title: "Task is finished!".to_string(),
                body: format!("{} took {}s (exit {})", cmd, duration_secs as u64, exit_code),
                dismiss_after_secs: Some(self.popup_secs()),
            })
        } else {
            None
        }
    }

    /// Locking pauses counting. A lock at least as long as the break-reset
    /// window counts as a real break (the away time keeps accruing).
    pub fn on_lock_changed(&mut self, locked: bool, now: Instant) {
        self.advance(now); // attribute time up to now using the OLD lock state
        self.locked = locked;
    }

    /// One-line state dump for the NOTIFY / debug log.
    pub fn debug_summary(&self) -> String {
        format!(
            "current={:?} locked={} vscode={:.0}s browser={:.0}s alerts={}",
            self.current,
            self.locked,
            self.vscode.accumulated.as_secs_f64(),
            self.browser.continuous.as_secs_f64(),
            self.browser.alerts
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    /// Real-world timings, but no suspend-gap detection so tests can jump.
    fn cfg() -> Config {
        Config {
            max_gap: secs(1_000_000),
            ..Config::default()
        }
    }

    #[test]
    fn vscode_fires_after_25m() {
        let mut e = Engine::new(cfg());
        let t = Instant::now();
        e.on_focus(App::VsCode, t);
        let n = e.on_tick(t + secs(1500));
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].kind, NotificationKind::VsCodeBreak);
        assert_eq!(n[0].dismiss_after_secs, Some(10));
    }

    #[test]
    fn vscode_counter_resets_after_fire() {
        let mut e = Engine::new(cfg());
        let t = Instant::now();
        e.on_focus(App::VsCode, t);
        assert_eq!(e.on_tick(t + secs(1500)).len(), 1);
        assert_eq!(e.on_tick(t + secs(1501)).len(), 0);
    }

    #[test]
    fn vscode_short_detour_does_not_reset() {
        let mut e = Engine::new(cfg());
        let t = Instant::now();
        e.on_focus(App::VsCode, t);
        e.on_tick(t + secs(600));
        e.on_focus(App::Terminal, t + secs(600));
        e.on_tick(t + secs(612));
        e.on_focus(App::VsCode, t + secs(612));
        // 600s before + 899s after = 1499s: not yet
        assert_eq!(e.on_tick(t + secs(612 + 899)).len(), 0);
        // 600 + 900 = 1500s: fires
        assert_eq!(e.on_tick(t + secs(612 + 900)).len(), 1);
    }

    #[test]
    fn vscode_long_break_resets() {
        let mut e = Engine::new(cfg());
        let t = Instant::now();
        e.on_focus(App::VsCode, t);
        e.on_tick(t + secs(600));
        e.on_focus(App::Other, t + secs(600));
        e.on_tick(t + secs(901)); // 301s away >= 300s => real break
        e.on_focus(App::VsCode, t + secs(901));
        // only 900s since returning: must NOT fire
        assert_eq!(e.on_tick(t + secs(901 + 900)).len(), 0);
        // a full 1500s since returning: fires
        assert_eq!(e.on_tick(t + secs(901 + 1500)).len(), 1);
    }

    #[test]
    fn vscode_to_browser_and_back_keeps_counter() {
        // Regression: the old single `last_focus` slot wiped VS Code's counter here.
        let mut e = Engine::new(cfg());
        let t = Instant::now();
        e.on_focus(App::VsCode, t);
        e.on_tick(t + secs(600));
        e.on_focus(App::Browser, t + secs(600));
        e.on_tick(t + secs(660));
        e.on_focus(App::VsCode, t + secs(660));
        assert_eq!(e.on_tick(t + secs(660 + 900)).len(), 1);
    }

    #[test]
    fn browser_fires_at_30m_then_every_10m() {
        let mut e = Engine::new(cfg());
        let t = Instant::now();
        e.on_focus(App::Browser, t);
        let n = e.on_tick(t + secs(1800));
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].kind, NotificationKind::BrowserNudge);
        assert_eq!(e.on_tick(t + secs(1800 + 600)).len(), 1);
        assert_eq!(e.on_tick(t + secs(1800 + 1200)).len(), 1);
    }

    #[test]
    fn browser_short_alt_tab_keeps_timer() {
        let mut e = Engine::new(cfg());
        let t = Instant::now();
        e.on_focus(App::Browser, t);
        e.on_tick(t + secs(1200));
        e.on_focus(App::Terminal, t + secs(1200));
        e.on_tick(t + secs(1205)); // 5s away <= 10s grace
        e.on_focus(App::Browser, t + secs(1205));
        // 1200s + 600s = 1800s of browser time: fires
        assert_eq!(e.on_tick(t + secs(1205 + 600)).len(), 1);
    }

    #[test]
    fn browser_leave_over_grace_resets() {
        let mut e = Engine::new(cfg());
        let t = Instant::now();
        e.on_focus(App::Browser, t);
        e.on_tick(t + secs(1200));
        e.on_focus(App::Other, t + secs(1200));
        e.on_tick(t + secs(1215)); // 15s away > 10s grace => reset
        e.on_focus(App::Browser, t + secs(1215));
        assert_eq!(e.on_tick(t + secs(1215 + 1200)).len(), 0);
        assert_eq!(e.on_tick(t + secs(1215 + 1800)).len(), 1);
    }

    #[test]
    fn terminal_over_threshold_fires_even_when_browser_focused() {
        let mut e = Engine::new(cfg());
        let t = Instant::now();
        e.on_focus(App::Browser, t);
        e.on_tick(t + secs(100));
        let n = e.on_terminal_finished(35.0, "sleep".to_string(), 0, t + secs(100));
        assert_eq!(n.unwrap().kind, NotificationKind::TerminalDone);
    }

    #[test]
    fn terminal_under_threshold_ignored() {
        let mut e = Engine::new(cfg());
        let t = Instant::now();
        e.on_focus(App::Browser, t);
        assert!(e
            .on_terminal_finished(10.0, "ls".to_string(), 0, t + secs(100))
            .is_none());
    }

    #[test]
    fn lock_pauses_counting() {
        let mut e = Engine::new(cfg());
        let t = Instant::now();
        e.on_focus(App::VsCode, t);
        e.on_tick(t + secs(600));
        e.on_lock_changed(true, t + secs(600));
        assert_eq!(e.on_tick(t + secs(630)).len(), 0); // nothing while locked
        e.on_lock_changed(false, t + secs(660)); // 60s lock < 300s
        // 600s before the lock + 900s after = 1500s: fires
        assert_eq!(e.on_tick(t + secs(660 + 900)).len(), 1);
    }

    #[test]
    fn lock_longer_than_break_resets() {
        let mut e = Engine::new(cfg());
        let t = Instant::now();
        e.on_focus(App::VsCode, t);
        e.on_tick(t + secs(600));
        e.on_lock_changed(true, t + secs(600));
        e.on_lock_changed(false, t + secs(900)); // 300s lock == real break
        assert_eq!(e.on_tick(t + secs(900 + 900)).len(), 0);
    }

    #[test]
    fn suspend_gap_counts_as_away_not_focus() {
        let mut e = Engine::new(Config {
            max_gap: secs(120),
            ..Config::default()
        });
        let t = Instant::now();
        e.on_focus(App::VsCode, t);
        e.on_tick(t + secs(60));
        // laptop slept for an hour: must not be counted as 1h of coding
        assert_eq!(e.on_tick(t + secs(60 + 3600)).len(), 0);
        assert_eq!(e.on_tick(t + secs(60 + 3601)).len(), 0);
    }

    #[test]
    fn fast_mode_divides_by_60() {
        let c = Config::default().scaled(1.0 / 60.0);
        assert!((c.vscode_work.as_secs_f64() - 25.0).abs() < 1e-6);
        assert!((c.terminal_min.as_secs_f64() - 0.5).abs() < 1e-6);
        assert_eq!(c.max_gap, Duration::from_secs(120)); // not scaled
    }
}