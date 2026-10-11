// Workflow tracking: app usage timers, motivational quotes, and work notifications.
// Monitors active windows and triggers island notifications based on time.

import { Bridge } from "./bridge";
import { Sound } from "./sound";
import { State } from "./state";
import { classifySessionCategory, getQuoteForWindow, getRandomQuote, type QuoteCategory } from "./quotes";

export type AppType =
  | "vscode"
  | "antigravity"
  | "kiro"
  | "claude"
  | "chatgpt"
  | "konsole"
  | "terminal"
  | "browser"
  | "other";

export interface WorkflowNotification {
  message: string;
  detail?: string;
}

export interface WorkflowSettings {
  // Motivational quotes
  quotesEnabled: boolean;
  quotesIntervalSeconds: number; // 240 seconds (4 minutes)

  // Work notification
  workNotificationEnabled: boolean;
  workNotificationSeconds: number; // 240 seconds (4 minutes)

  // Legacy settings for UI compatibility
  vscodeEnabled?: boolean;
  vscodeLimitMinutes?: number;
  vscodeBreakMessage?: string;
  vscodeBreakDuration?: number;
  browserEnabled?: boolean;
  browserLimitMinutes?: number;
  browserMessage?: string;
  browserReminderInterval?: number;
  terminalEnabled?: boolean;
  terminalMessage?: string;
  terminalMinDuration?: number;
  quotesIntervalMinutes?: number;
  workNotificationMinutes?: number;
}

export const DEFAULT_WORKFLOW_SETTINGS: WorkflowSettings = {
  quotesEnabled: true,
  quotesIntervalSeconds: 240, // 4 minutes (240 seconds)

  workNotificationEnabled: true,
  workNotificationSeconds: 240, // 4 minutes (240 seconds)

  // Legacy defaults for UI compatibility
  vscodeEnabled: true,
  vscodeLimitMinutes: 25,
  vscodeBreakMessage: "5 minutes break time",
  vscodeBreakDuration: 10,
  browserEnabled: true,
  browserLimitMinutes: 30,
  browserMessage: "Go back to VS Code",
  browserReminderInterval: 10,
  terminalEnabled: true,
  terminalMessage: "Task is finished!",
  terminalMinDuration: 10,
  quotesIntervalMinutes: 4,
  workNotificationMinutes: 4,
};

function activityDescription(app: AppType): string {
  switch (app) {
    case "antigravity":
      return "building in Antigravity";
    case "kiro":
      return "architecting in Kiro";
    case "claude":
      return "collaborating with Claude";
    case "chatgpt":
      return "brainstorming in ChatGPT";
    case "konsole":
      return "working in Konsole";
    case "terminal":
      return "working in Terminal";
    case "vscode":
      return "coding in VS Code";
    case "browser":
      return "researching in Browser";
    default:
      return "focusing";
  }
}

export function formatMmSs(totalSeconds: number): string {
  const clamped = Math.max(0, Math.floor(totalSeconds));
  const mins = Math.floor(clamped / 60);
  const secs = clamped % 60;
  return `${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
}

class WorkflowEngine {
  settings: WorkflowSettings = { ...DEFAULT_WORKFLOW_SETTINGS };

  private currentApp: AppType = "other";
  private appStartTime: number = Date.now();

  // Motivational quotes & work notification tracking
  private lastQuoteTime: number = Date.now();
  private lastWorkNotificationTime: number = Date.now();
  private workSessionStartTime: number = Date.now();

  // Notification display timeout
  private notificationTimeout: number | null = null;

  constructor() {
    console.log("[workflow] WorkflowEngine initialized");
    this.loadSettings();
    State.lastWorkflowQuote = getQuoteForWindow("vscode");
    void this.syncCurrentTask();
    this.startTracking();
  }

  async syncCurrentTask() {
    const task = await Bridge.taskCurrent();
    if (task) {
      State.currentTaskRecord = task;
      State.notify();
    } else {
      const seeded = await Bridge.taskSet("Deep work session", "Coucou");
      if (seeded) {
        State.currentTaskRecord = seeded;
        State.notify();
      }
    }
  }

  async setCurrentTask(title: string) {
    const clean = title.trim();
    if (!clean) return;
    const created = await Bridge.taskSet(clean, "Coucou");
    if (created) {
      State.currentTaskRecord = created;
      Sound.play("blip");
      State.notify();
    }
  }

  async completeCurrentTask() {
    const current = State.currentTaskRecord;
    const done = await Bridge.taskComplete(current?.id);
    Sound.play("complete");
    const completedTitle = done?.title || current?.title || "Current task";
    const next = await Bridge.taskCurrent();
    if (next) {
      State.currentTaskRecord = next;
    } else {
      const fresh = await Bridge.taskSet("Next focus milestone", "Coucou");
      State.currentTaskRecord = fresh;
    }
    this.showNotification({
      message: `Task marked done: "${completedTitle}" ✅`,
      detail: State.currentTaskRecord
        ? `Up next: ${State.currentTaskRecord.title}`
        : "Saved to SQLite task log",
    });
  }

  async startFocusSession(targetSecs = 1500) {
    if (State.focusSession) {
      await this.cancelFocusSession();
    }
    const taskTitle = State.currentTaskRecord?.title || "25m Focus Session";
    const appClass = State.focusTask?.windowClass || State.focusTask?.name || this.currentApp;
    const record = await Bridge.focusSessionStart(targetSecs, taskTitle, appClass);
    const sessionId = record?.id ?? Date.now();
    State.focusSession = {
      id: sessionId,
      remainingSecs: targetSecs,
      targetSecs,
      elapsedSecs: 0,
      paused: false,
      taskTitle,
      appClass,
    };
    State.lastWorkflowDetail = `⏱ Focus ${formatMmSs(targetSecs)} · ${taskTitle}`;
    Sound.play("peek");
    State.notify();
  }

  togglePauseFocusSession() {
    if (!State.focusSession) return;
    State.focusSession.paused = !State.focusSession.paused;
    State.lastWorkflowDetail = State.focusSession.paused
      ? `⏸ Paused ${formatMmSs(State.focusSession.remainingSecs)} · ${State.focusSession.taskTitle}`
      : `⏱ Focus ${formatMmSs(State.focusSession.remainingSecs)} · ${State.focusSession.taskTitle}`;
    Sound.play("blip");
    State.notify();
  }

  async cancelFocusSession() {
    const active = State.focusSession;
    if (!active) return;
    State.focusSession = null;
    State.lastWorkflowDetail = null;
    Sound.play("close");
    State.notify();
    await Bridge.focusSessionFinish(active.id, active.elapsedSecs, "cancelled");
  }

  private tickFocusSession() {
    const session = State.focusSession;
    if (!session) return;
    if (session.paused) return;

    session.remainingSecs = Math.max(0, session.remainingSecs - 1);
    session.elapsedSecs += 1;
    State.lastWorkflowDetail = `⏱ Focus ${formatMmSs(session.remainingSecs)} · ${session.taskTitle}`;

    if (session.remainingSecs <= 0) {
      const finishedSession = { ...session };
      State.focusSession = null;
      Sound.play("complete");
      void Bridge.focusSessionFinish(
        finishedSession.id,
        finishedSession.targetSecs,
        "completed",
      );
      const mins = Math.max(1, Math.round(finishedSession.targetSecs / 60));
      this.showNotification({
        message: `${mins}-minute focus session complete! Great work, review & stretch 🎯`,
        detail: `Completed "${finishedSession.taskTitle}" · Logged to SQLite`,
      });
      return;
    }

    State.notify();
  }

  private loadSettings() {
    try {
      const raw = localStorage.getItem("coucou-workflow-settings");
      if (raw) {
        this.settings = { ...DEFAULT_WORKFLOW_SETTINGS, ...JSON.parse(raw) };
      }
      if (this.settings.quotesIntervalSeconds < 240) {
        this.settings.quotesIntervalSeconds = 240;
      }
      if (this.settings.workNotificationSeconds < 240) {
        this.settings.workNotificationSeconds = 240;
      }
      this.saveSettings();
    } catch {
      // Use defaults
    }
  }

  saveSettings() {
    try {
      localStorage.setItem("coucou-workflow-settings", JSON.stringify(this.settings));
    } catch {
      // Ignore
    }
  }

  private startTracking() {
    // Track active window every second
    setInterval(() => this.tick(), 1000);
  }

  private tick() {
    this.detectActiveApp();
    this.tickFocusSession();
    this.checkNotifications();
  }

  private detectActiveApp() {
    Bridge.focusTrackerWindowInfo()
      .then((info) => {
        const name = info?.name || State.focusTask?.name || "";
        const title = info?.title || State.focusTask?.windowCaption || "";
        if (!name && !title) return;

        const newApp = this.classifyApp(name, title);
        if (newApp !== "other" && newApp !== this.currentApp) {
          console.log(`[workflow] App changed: ${this.currentApp} -> ${newApp}`);
          this.currentApp = newApp;
          this.appStartTime = Date.now();
          State.lastWorkflowQuote = getRandomQuote(newApp as QuoteCategory);
          State.notify();
        } else if (this.currentApp === "other" && State.focusTask) {
          const fallbackApp = this.classifyApp(
            State.focusTask.name,
            State.focusTask.windowCaption ?? "",
          );
          if (fallbackApp !== "other") {
            this.currentApp = fallbackApp;
            State.lastWorkflowQuote = getRandomQuote(fallbackApp as QuoteCategory);
            State.notify();
          }
        }
      })
      .catch((err) => {
        console.error("[workflow] focusTrackerWindowInfo failed:", err);
      });
  }

  private classifyApp(windowClass: string, title: string): AppType {
    const category = classifySessionCategory(windowClass, title);
    if (category === "general") return "other";
    return category;
  }

  private resolveActiveCategory(): QuoteCategory {
    if (this.currentApp !== "other") {
      return this.currentApp;
    }
    if (State.focusTask) {
      return classifySessionCategory(
        State.focusTask.name,
        State.focusTask.windowCaption ?? "",
      );
    }
    return "vscode";
  }

  private formatFocusDetail(now: number, category: QuoteCategory): string | undefined {
    if (State.focusSession) {
      return `⏱ Focus ${formatMmSs(State.focusSession.remainingSecs)} · ${State.focusSession.taskTitle}`;
    }
    if (!this.settings.workNotificationEnabled) return undefined;
    const sessionElapsed = Math.max(0, Math.floor((now - this.workSessionStartTime) / 1000));
    if (sessionElapsed < 60) return undefined;
    const mins = Math.floor(sessionElapsed / 60);
    const durationStr = `${mins} minute${mins === 1 ? "" : "s"}`;
    const appType: AppType = category === "general" ? this.currentApp : category;
    const activity = activityDescription(appType);
    return `Great focus! You've been ${activity} for ${durationStr} 🎯`;
  }

  private checkNotifications() {
    if (!this.settings.quotesEnabled && !this.settings.workNotificationEnabled) return;

    const now = Date.now();
    const interval = Math.max(
      this.settings.quotesIntervalSeconds || 240,
      this.settings.workNotificationSeconds || 240,
    );
    const elapsedSinceQuote = (now - this.lastQuoteTime) / 1000;
    const elapsedSinceWork = (now - this.lastWorkNotificationTime) / 1000;

    if (elapsedSinceQuote < interval && elapsedSinceWork < interval) return;

    this.lastQuoteTime = now;
    this.lastWorkNotificationTime = now;

    // Always pick a fresh, non-repeating motivational quote for the active session
    const category = this.resolveActiveCategory();
    const freshQuote = getRandomQuote(category);
    const focusDetail = this.formatFocusDetail(now, category);

    this.showNotification({
      message: freshQuote,
      detail: focusDetail,
    });
  }

  private showNotification(notification: WorkflowNotification) {
    console.log("[workflow] Showing notification:", notification);

    // Play sound
    Sound.play("blip");

    // Store the rotating motivational quote and optional timer detail in State
    // so the main left pill always displays a fresh quote (never overwritten by static timer text)
    State.lastWorkflowQuote = notification.message;
    if (notification.detail) {
      State.lastWorkflowDetail = notification.detail;
    }
    State.workflowNotification = notification;
    State.notify();

    // Auto-hide transient popup flag after 6 seconds while keeping lastWorkflowQuote in the main pill
    if (this.notificationTimeout) {
      clearTimeout(this.notificationTimeout);
    }
    this.notificationTimeout = window.setTimeout(() => {
      State.workflowNotification = null;
      State.notify();
    }, 6000);
  }

  getCurrentApp(): AppType {
    return this.currentApp;
  }

  getAppDuration(): number {
    return (Date.now() - this.appStartTime) / 1000;
  }

  updateSettings(settings: Partial<WorkflowSettings>) {
    this.settings = { ...this.settings, ...settings };
    this.saveSettings();
  }
}

// Create and export the singleton instance
// This will start tracking as soon as the module is imported
export const Workflow = new WorkflowEngine();
