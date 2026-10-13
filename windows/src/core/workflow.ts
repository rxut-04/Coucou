// Workflow tracking: app usage timers, motivational quotes, and work notifications.
// Monitors active windows and triggers island notifications based on time.

import {
  Bridge,
  onEvent,
  type AgentLiveStatus,
  type AgentWaitEventPayload,
  type BrowserDomainEvent,
  type HealthReminderEvent,
} from "./bridge";
import { Sound } from "./sound";
import { State } from "./state";
import {
  classifySessionCategory,
  getNightWindDownQuote,
  getQuoteForWindow,
  getRandomQuote,
  type QuoteCategory,
} from "./quotes";

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
  quotesIntervalSeconds: number; // 480 seconds (8 minutes)

  // Work notification
  workNotificationEnabled: boolean;
  workNotificationSeconds: number; // 480 seconds (8 minutes)

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
  quotesIntervalSeconds: 480, // 8 minutes (480 seconds)

  workNotificationEnabled: true,
  workNotificationSeconds: 480, // 8 minutes (480 seconds)

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
  quotesIntervalMinutes: 8,
  workNotificationMinutes: 8,
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

export function formatDurationCompact(totalSeconds: number): string {
  const clamped = Math.max(0, Math.floor(totalSeconds));
  const hrs = Math.floor(clamped / 3600);
  const mins = Math.floor((clamped % 3600) / 60);
  const secs = clamped % 60;
  if (hrs > 0) return `${hrs}h ${mins}m`;
  if (mins > 0) return `${mins}m ${secs}s`;
  return `${secs}s`;
}

class WorkflowEngine {
  settings: WorkflowSettings = { ...DEFAULT_WORKFLOW_SETTINGS };

  private currentApp: AppType = "other";
  private appStartTime: number = Date.now();

  // Motivational quotes & work notification tracking
  private lastQuoteTime: number = Date.now();
  private lastWorkNotificationTime: number = Date.now();
  private workSessionStartTime: number = Date.now();
  private agentPollCounter = 0;

  // Notification display timeout
  private notificationTimeout: number | null = null;

  constructor() {
    console.log("[workflow] WorkflowEngine initialized");
    this.loadSettings();
    State.lastWorkflowQuote = getQuoteForWindow("vscode");
    void this.syncCurrentTask();
    void this.initBrowserIntent();
    void this.initAgentMonitor();
    void this.initDailyGoalsAndRecap();
    void this.initHealthMode();
    void this.initStatsDashboard();
    void this.initSystemMedia();
    this.startTracking();
  }

  private async initSystemMedia() {
    await Promise.all([
      this.refreshBatteryStatus(),
      this.refreshBluetoothStatus(),
      this.refreshWeatherStatus(),
      this.refreshMediaStatus(),
    ]);
  }

  async refreshWeatherStatus() {
    const wx = await Bridge.weatherStatusGet();
    if (!wx) return;
    const prev = State.weatherStatus;
    const changed =
      !prev ||
      prev.tempC !== wx.tempC ||
      prev.condition !== wx.condition ||
      prev.location !== wx.location ||
      prev.humidity !== wx.humidity ||
      prev.uvIndex !== wx.uvIndex;
    State.weatherStatus = wx;
    if (changed) {
      State.notify();
    }
  }

  toggleWeatherCard() {
    State.showingWeatherCard = !State.showingWeatherCard;
    if (State.showingWeatherCard) {
      State.selectedWeatherDayIdx = 0;
      State.showingMorningRecap = false;
      State.showingDailyGoalsCard = false;
      State.showingHealthCard = false;
      State.showingStatsCard = false;
      State.showingFocusTimerCard = false;
      void this.refreshWeatherStatus();
    }
    Sound.play("blip");
    State.notify();
  }

  selectWeatherDay(idx: number) {
    State.selectedWeatherDayIdx = Math.max(0, Math.min(3, idx));
    Sound.play("blip");
    State.notify();
  }

  async refreshBatteryStatus() {
    const bat = await Bridge.batteryStatusGet();
    if (!bat) return;
    const prev = State.batteryStatus;
    const changed =
      !prev ||
      prev.charging !== bat.charging ||
      prev.percentage !== bat.percentage ||
      prev.status !== bat.status ||
      prev.hasBattery !== bat.hasBattery;
    State.batteryStatus = bat;
    if (changed) {
      State.notify();
    }
  }

  async refreshBluetoothStatus() {
    const bt = await Bridge.bluetoothStatusGet();
    if (!bt) return;
    const prev = State.bluetoothStatus;
    const changed =
      !prev ||
      prev.connected !== bt.connected ||
      prev.primaryName !== bt.primaryName ||
      prev.primaryAddress !== bt.primaryAddress ||
      prev.primaryBattery !== bt.primaryBattery ||
      prev.devices.length !== bt.devices.length;
    State.bluetoothStatus = bt;
    if (changed) {
      State.notify();
    }
  }

  async refreshMediaStatus() {
    const media = await Bridge.mediaStatusGet();
    if (!media) return;
    const prev = State.mediaStatus;
    const changed =
      !prev ||
      prev.active !== media.active ||
      prev.playing !== media.playing ||
      prev.title !== media.title ||
      prev.artist !== media.artist ||
      Math.floor(prev.positionSecs) !== Math.floor(media.positionSecs) ||
      Math.floor(prev.durationSecs) !== Math.floor(media.durationSecs) ||
      prev.muted !== media.muted;
    State.mediaStatus = media;
    if (changed) {
      State.notify();
    }
  }

  async mediaAction(
    action: "play-pause" | "next" | "previous" | "seek-relative" | "seek-to" | "mute",
    value?: number,
  ) {
    Sound.play("blip");
    const updated = await Bridge.mediaControl(action, value);
    if (updated) {
      State.mediaStatus = updated;
      State.notify();
    } else {
      await this.refreshMediaStatus();
    }
  }

  private async initStatsDashboard() {
    await this.refreshStatsDashboard();
  }

  async refreshStatsDashboard() {
    const stats = await Bridge.statsDashboardGet();
    if (stats) {
      State.statsDashboard = stats;
      State.notify();
    }
  }

  toggleStatsCard() {
    State.showingStatsCard = !State.showingStatsCard;
    if (State.showingStatsCard) {
      State.showingMorningRecap = false;
      State.showingDailyGoalsCard = false;
      State.showingHealthCard = false;
      State.showingFocusTimerCard = false;
      State.showingWeatherCard = false;
      void this.refreshStatsDashboard();
    }
    Sound.play("blip");
    State.notify();
  }

  private async initHealthMode() {
    const snap = await Bridge.healthModeStatus();
    if (snap) {
      State.healthStatus = snap;
      if (snap.isNightMode) {
        State.lastWorkflowQuote = getNightWindDownQuote();
      }
      State.notify();
    }

    await onEvent<HealthReminderEvent>("health-reminder-event", (evt) => {
      if (!evt) return;
      void Bridge.healthModeStatus().then((latest) => {
        if (latest) State.healthStatus = latest;
      });
      this.showNotification({
        message: `${evt.title}: ${evt.message}`,
        detail: evt.detail,
      });
    });

    if (typeof window !== "undefined") {
      let lastPing = 0;
      const onUserInteract = () => {
        const now = Date.now();
        if (now - lastPing > 5000) {
          lastPing = now;
          void Bridge.healthModeActivityPing();
        }
      };
      window.addEventListener("mousemove", onUserInteract, { passive: true });
      window.addEventListener("keydown", onUserInteract, { passive: true });
      window.addEventListener("mousedown", onUserInteract, { passive: true });
    }
  }

  toggleHealthCard() {
    State.showingHealthCard = !State.showingHealthCard;
    if (State.showingHealthCard) {
      State.showingMorningRecap = false;
      State.showingDailyGoalsCard = false;
      State.showingStatsCard = false;
      State.showingFocusTimerCard = false;
      State.showingWeatherCard = false;
      void Bridge.healthModeStatus().then((snap) => {
        if (snap) {
          State.healthStatus = snap;
          State.notify();
        }
      });
    }
    Sound.play("blip");
    State.notify();
  }

  async triggerHealthDemo(kind: "eye" | "water" | "posture" | "night" | "day" | "idle" | "active") {
    const snap = await Bridge.healthModeTrigger(kind);
    if (snap) {
      State.healthStatus = snap;
      if (kind === "night" || snap.isNightMode) {
        State.lastWorkflowQuote = getNightWindDownQuote();
      } else if (kind === "day") {
        State.lastWorkflowQuote = getRandomQuote(this.resolveActiveCategory());
      }
      if (snap.activeReminder && (kind === "eye" || kind === "water" || kind === "posture" || kind === "night")) {
        this.showNotification({
          message: `${snap.activeReminder.title}: ${snap.activeReminder.message}`,
          detail: snap.activeReminder.detail,
        });
      } else {
        Sound.play("blip");
        State.notify();
      }
    }
  }

  async acknowledgeHealthReminder() {
    await Bridge.healthModeAcknowledge();
    if (State.healthStatus) {
      State.healthStatus.activeReminder = null;
    }
    Sound.play("blip");
    State.notify();
  }

  private tickHealthMode() {
    if (this.agentPollCounter % 2 === 0) {
      void Bridge.healthModeStatus().then((snap) => {
        if (!snap) return;
        const prevNight = State.healthStatus?.isNightMode;
        const prevIdle = State.healthStatus?.isIdle;
        State.healthStatus = snap;
        if (snap.isNightMode && !prevNight) {
          State.lastWorkflowQuote = getNightWindDownQuote();
        }
        if (snap.isNightMode !== prevNight || snap.isIdle !== prevIdle) {
          State.notify();
        }
      });
    }
  }

  private async initDailyGoalsAndRecap() {
    const goals = await Bridge.dailyGoalsList();
    if (goals) {
      State.dailyGoals = goals;
      State.notify();
    }
    const recap = await Bridge.morningRecapGet(false);
    if (recap) {
      State.morningRecap = recap;
      if (recap.shouldShowToday) {
        State.showingMorningRecap = true;
        Sound.play("peek");
      }
      State.notify();
    }
  }

  async toggleDailyGoal(id: number) {
    const goals = await Bridge.dailyGoalToggle(id);
    if (goals) {
      State.dailyGoals = goals;
      void this.refreshStatsDashboard();
      const doneCount = goals.filter((g) => g.done).length;
      const toggled = goals.find((g) => g.id === id);
      Sound.play(toggled?.done ? "complete" : "blip");
      if (toggled?.done) {
        this.showNotification({
          message: `Daily goal completed: "${toggled.title}" (${doneCount}/${goals.length}) 🎯`,
          detail: doneCount === goals.length ? "All daily goals complete! Outstanding work 🚀" : `Progress: ${doneCount}/${goals.length} daily goals`,
        });
      } else {
        State.notify();
      }
    }
  }

  async setDailyGoal(slot: number, title: string) {
    const clean = title.trim();
    if (!clean) return;
    const goals = await Bridge.dailyGoalSet(slot, clean);
    if (goals) {
      State.dailyGoals = goals;
      void this.refreshStatsDashboard();
      Sound.play("blip");
      State.notify();
    }
  }

  async addDailyGoal(title: string) {
    const clean = title.trim();
    if (!clean) return;
    const goals = await Bridge.dailyGoalAdd(clean);
    if (goals) {
      State.dailyGoals = goals;
      void this.refreshStatsDashboard();
      Sound.play("blip");
      State.notify();
    }
  }

  async deleteDailyGoal(id: number) {
    const goals = await Bridge.dailyGoalDelete(id);
    if (goals) {
      State.dailyGoals = goals;
      void this.refreshStatsDashboard();
      Sound.play("blip");
      State.notify();
    }
  }

  toggleDailyGoalsCard() {
    State.showingDailyGoalsCard = !State.showingDailyGoalsCard;
    if (State.showingDailyGoalsCard) {
      State.showingMorningRecap = false;
      State.showingHealthCard = false;
      State.showingStatsCard = false;
      State.showingFocusTimerCard = false;
      State.showingWeatherCard = false;
      void Bridge.dailyGoalsList().then((goals) => {
        if (goals) {
          State.dailyGoals = goals;
          State.notify();
        }
      });
    }
    Sound.play("blip");
    State.notify();
  }

  async triggerMorningRecapDemo() {
    const [recap, goals] = await Promise.all([
      Bridge.morningRecapGet(true),
      Bridge.dailyGoalsList(),
    ]);
    if (goals) {
      State.dailyGoals = goals;
    }
    if (recap) {
      State.morningRecap = recap;
      State.showingDailyGoalsCard = false;
      State.showingHealthCard = false;
      State.showingStatsCard = false;
      State.showingFocusTimerCard = false;
      State.showingWeatherCard = false;
      State.showingMorningRecap = true;
      Sound.play("peek");
      State.notify();
    }
  }

  async dismissMorningRecap() {
    State.showingMorningRecap = false;
    Sound.play("blip");
    State.notify();
    await Bridge.morningRecapDismiss();
  }

  private async initAgentMonitor() {
    const snap = await Bridge.agentMonitorStatus();
    if (snap) {
      State.activeAgentWait = snap.activeAgent;
      State.weeklyAgentWaitStats = snap.weeklyStats;
      State.notify();
    }

    await onEvent<AgentWaitEventPayload>("agent-wait-event", (evt) => {
      if (!evt) return;
      State.weeklyAgentWaitStats = evt.weeklyStats;
      if (evt.state === "working") {
        State.activeAgentWait = {
          agent: evt.agent,
          displayName: evt.displayName,
          windowClass: evt.windowClass,
          state: "working",
          detail: evt.detail,
          startedAt: evt.timestamp,
          elapsedSecs: 0,
          lastWaitSecs: 0,
          sourcePath: "",
        };
        Sound.play("peek");
        State.notify();
      } else if (evt.state === "done") {
        State.activeAgentWait = {
          agent: evt.agent,
          displayName: evt.displayName,
          windowClass: evt.windowClass,
          state: "done",
          detail: evt.detail,
          startedAt: evt.timestamp - evt.waitSecs,
          elapsedSecs: 0,
          lastWaitSecs: evt.waitSecs,
          sourcePath: "",
        };
        Sound.play("complete");
        const weeklyStr = formatDurationCompact(evt.weeklyStats.totalWaitSecs);
        this.showNotification({
          message: `🤖 ${evt.displayName} done, review it! (waited ${formatDurationCompact(evt.waitSecs)})`,
          detail: `Click to jump to ${evt.displayName} · Weekly agent wait: ${weeklyStr}`,
        });
      }
    });

    await onEvent<{ agent?: string; cmd: string; duration: number; exit_code: number }>(
      "terminal-finished",
      (evt) => {
        if (!evt) return;
        const agName = evt.agent === "konsole" ? "Konsole" : "Terminal";
        this.showNotification({
          message: `💻 ${agName}: '${evt.cmd}' finished (${evt.duration.toFixed(1)}s, exit ${evt.exit_code})`,
          detail: `Click to jump to ${agName}`,
        });
      },
    );
  }

  async focusAndAcknowledgeAgent(target?: AgentLiveStatus | null) {
    const ag = target ?? State.activeAgentWait;
    if (!ag) return;
    Sound.play("blip");

    const winClass = ag.windowClass || ag.agent;
    const match = State.desktopWindows.find((w) =>
      (w.resourceClass || "").toLowerCase().includes(winClass.toLowerCase()),
    );
    if (match) {
      await Bridge.focusTrackerActivateWindow(match.id, match.pid, match.resourceClass);
    } else {
      await Bridge.focusTrackerActivateWindow("", 0, winClass);
    }

    if (ag.state === "done") {
      await Bridge.agentMonitorAcknowledge(ag.agent);
      State.activeAgentWait = null;
      State.notify();
    }
  }

  private tickAgentMonitor() {
    if (State.activeAgentWait?.state === "working") {
      State.activeAgentWait.elapsedSecs += 1;
      State.notify();
    }
    this.agentPollCounter += 1;
    if (this.agentPollCounter % 2 === 0) {
      void Bridge.agentMonitorStatus().then((snap) => {
        if (!snap) return;
        State.weeklyAgentWaitStats = snap.weeklyStats;
        const prevState = State.activeAgentWait?.state;
        const prevAgent = State.activeAgentWait?.agent;
        State.activeAgentWait = snap.activeAgent;
        if (
          snap.activeAgent?.state !== prevState ||
          snap.activeAgent?.agent !== prevAgent
        ) {
          State.notify();
        }
      });
    }
  }

  private async initBrowserIntent() {
    const status = await Bridge.browserIntentStatus();
    if (status?.lastDomain && status?.lastCategory) {
      State.browserDomain = {
        domain: status.lastDomain,
        category: status.lastCategory,
      };
      State.notify();
    }

    await onEvent<BrowserDomainEvent>("browser-domain", (evt) => {
      if (!evt?.domain) return;
      State.browserDomain = {
        domain: evt.domain,
        category: evt.category,
      };

      const badgeLabel =
        evt.category === "productive"
          ? "Productive"
          : evt.category === "distracting"
            ? "Distracting"
            : "Neutral";

      if (
        evt.category === "distracting" &&
        (State.browserSession?.intent === "research" || State.focusSession != null)
      ) {
        const editorName = State.lastEditorWindow?.name || "your editor";
        this.showNotification({
          message: `Heads up: ${evt.domain} is marked distracting during focus.`,
          detail: `Click "↩ ${editorName}" whenever you're ready to jump back.`,
        });
        State.browserNudgeToEditor = true;
      } else {
        State.lastWorkflowDetail = `🌐 ${evt.domain} · ${badgeLabel}`;
      }
      State.notify();
    });
  }

  startBrowserSession(intent: "research" | "break", customSecs?: number) {
    const targetSecs = customSecs ?? (intent === "research" ? 900 : 300);
    State.browserPromptOpen = false;
    State.browserNudgeToEditor = false;
    State.browserSession = {
      intent,
      remainingSecs: targetSecs,
      targetSecs,
      elapsedSecs: 0,
      startedAtMs: Date.now(),
    };
    const label = intent === "research" ? "🔍 Research" : "☕ Break";
    const domainPart = State.browserDomain ? ` · ${State.browserDomain.domain}` : "";
    State.lastWorkflowDetail = `${label} ${formatMmSs(targetSecs)}${domainPart}`;
    Sound.play("peek");
    State.notify();
  }

  cancelBrowserSession() {
    const active = State.browserSession;
    State.browserSession = null;
    State.browserPromptOpen = false;
    if (active && active.elapsedSecs > 0) {
      const domain = State.browserDomain?.domain || "browser";
      const category = State.browserDomain?.category || "neutral";
      void Bridge.browserIntentRecordSession(domain, category, active.intent, active.elapsedSecs);
    }
    Sound.play("close");
    State.notify();
  }

  dismissBrowserPrompt() {
    State.browserPromptOpen = false;
    State.notify();
  }

  async jumpBackToEditor() {
    State.browserNudgeToEditor = false;
    State.browserPromptOpen = false;
    Sound.play("blip");

    if (State.lastEditorWindow) {
      await Bridge.focusTrackerActivateWindow(
        State.lastEditorWindow.id,
        State.lastEditorWindow.pid,
        State.lastEditorWindow.resourceClass,
      );
      State.notify();
      return;
    }

    const editorWin = State.desktopWindows.find((w) => {
      const cls = (w.resourceClass || "").toLowerCase();
      return (
        cls.includes("antigravity") ||
        cls.includes("kiro") ||
        cls.includes("code") ||
        cls.includes("cursor") ||
        cls.includes("konsole")
      );
    });
    if (editorWin) {
      await Bridge.focusTrackerActivateWindow(
        editorWin.id,
        editorWin.pid,
        editorWin.resourceClass,
      );
    } else {
      await Bridge.focusTrackerActivateWindow("", 0, "antigravity");
    }
    State.notify();
  }

  private tickBrowserSession() {
    const session = State.browserSession;
    if (!session) return;
    if (State.healthStatus?.isIdle) return;

    session.remainingSecs = Math.max(0, session.remainingSecs - 1);
    session.elapsedSecs += 1;

    const label = session.intent === "research" ? "🔍 Research" : "☕ Break";
    const domainPart = State.browserDomain ? ` · ${State.browserDomain.domain}` : "";
    if (!State.focusSession) {
      State.lastWorkflowDetail = `${label} ${formatMmSs(session.remainingSecs)}${domainPart}`;
    }

    if (session.remainingSecs <= 0) {
      const finished = { ...session };
      State.browserSession = null;
      State.browserNudgeToEditor = true;
      Sound.play("complete");

      const domain = State.browserDomain?.domain || "browser";
      const category = State.browserDomain?.category || "neutral";
      void Bridge.browserIntentRecordSession(
        domain,
        category,
        finished.intent,
        finished.targetSecs,
      );

      const mins = Math.max(1, Math.round(finished.targetSecs / 60));
      const editorName = State.lastEditorWindow?.name || "your editor";
      const kindLabel = finished.intent === "research" ? "Research" : "Break";
      this.showNotification({
        message: `${mins}m ${kindLabel} time is up! Ready to return to ${editorName}? 🚀`,
        detail: `Click "↩ ${editorName}" in the bar to jump right back`,
      });
      return;
    }

    State.notify();
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
    void this.refreshStatsDashboard();
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

  toggleFocusTimerCard(forceOpen?: boolean) {
    State.showingFocusTimerCard =
      forceOpen !== undefined ? forceOpen : !State.showingFocusTimerCard;
    if (State.showingFocusTimerCard) {
      State.showingMorningRecap = false;
      State.showingDailyGoalsCard = false;
      State.showingHealthCard = false;
      State.showingStatsCard = false;
      State.showingWeatherCard = false;
    }
    Sound.play("blip");
    State.notify();
  }

  addFocusSessionTime(deltaSecs: number) {
    if (!State.focusSession) return;
    State.focusSession.remainingSecs = Math.max(10, State.focusSession.remainingSecs + deltaSecs);
    State.focusSession.targetSecs = Math.max(
      State.focusSession.remainingSecs,
      State.focusSession.targetSecs + deltaSecs,
    );
    State.lastWorkflowDetail = State.focusSession.paused
      ? `⏸ Paused ${formatMmSs(State.focusSession.remainingSecs)} · ${State.focusSession.taskTitle}`
      : `⏱ Focus ${formatMmSs(State.focusSession.remainingSecs)} · ${State.focusSession.taskTitle}`;
    Sound.play("blip");
    State.notify();
  }

  async startFocusSession(targetSecs = 1500) {
    if (State.focusSession) {
      await this.cancelFocusSession(true);
    }
    const mins = Math.max(1, Math.round(targetSecs / 60));
    const taskTitle = State.currentTaskRecord?.title || `${mins}m Focus Session`;
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
    State.showingFocusTimerCard = true;
    State.showingMorningRecap = false;
    State.showingDailyGoalsCard = false;
    State.showingHealthCard = false;
    State.showingStatsCard = false;
    State.showingWeatherCard = false;
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

  async cancelFocusSession(keepCardOpen = false) {
    const active = State.focusSession;
    if (!active) return;
    State.focusSession = null;
    State.lastWorkflowDetail = null;
    if (!keepCardOpen) {
      State.showingFocusTimerCard = false;
      Sound.play("close");
    }
    State.notify();
    await Bridge.focusSessionFinish(active.id, active.elapsedSecs, "cancelled");
    void this.refreshStatsDashboard();
  }

  private tickFocusSession() {
    const session = State.focusSession;
    if (!session) return;
    if (session.paused || State.healthStatus?.isIdle) return;

    session.remainingSecs = Math.max(0, session.remainingSecs - 1);
    session.elapsedSecs += 1;
    State.lastWorkflowDetail = `⏱ Focus ${formatMmSs(session.remainingSecs)} · ${session.taskTitle}`;

    if (session.remainingSecs <= 0) {
      const finishedSession = { ...session };
      State.focusSession = null;
      State.showingFocusTimerCard = false;
      Sound.play("complete");
      void Bridge.focusSessionFinish(
        finishedSession.id,
        finishedSession.targetSecs,
        "completed",
      ).then(() => this.refreshStatsDashboard());
      const mins = Math.max(1, Math.round(finishedSession.targetSecs / 60));
      this.showNotification({
        message: `${mins}-minute focus session complete! Great work, review & stretch 🎯`,
        detail: `Completed "${finishedSession.taskTitle}" · Logged to SQLite`,
      });
      return;
    }

    State.notify();
  }

  private tickSystemMedia() {
    void this.refreshBatteryStatus();
    if (this.agentPollCounter % 2 === 0) {
      void this.refreshBluetoothStatus();
    }
    const isBrowserFocused =
      State.focusTask?.name === "Browser" ||
      (State.focusTask?.windowClass || "").toLowerCase().includes("brave") ||
      (State.focusTask?.windowClass || "").toLowerCase().includes("chrome") ||
      (State.focusTask?.windowClass || "").toLowerCase().includes("firefox");
    if (State.mode === "expanded" || isBrowserFocused || this.agentPollCounter % 2 === 0) {
      void this.refreshMediaStatus();
    }
  }

  private loadSettings() {
    try {
      const raw = localStorage.getItem("coucou-workflow-settings");
      if (raw) {
        this.settings = { ...DEFAULT_WORKFLOW_SETTINGS, ...JSON.parse(raw) };
      }
      if (this.settings.quotesIntervalSeconds < 480) {
        this.settings.quotesIntervalSeconds = 480;
      }
      if (this.settings.workNotificationSeconds < 480) {
        this.settings.workNotificationSeconds = 480;
      }
      if (!this.settings.quotesIntervalMinutes || this.settings.quotesIntervalMinutes < 8) {
        this.settings.quotesIntervalMinutes = 8;
      }
      if (!this.settings.workNotificationMinutes || this.settings.workNotificationMinutes < 8) {
        this.settings.workNotificationMinutes = 8;
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

  private lastStressUpdatedAt = 0;
  private activityTickCounter = 0;
  private lastTrackedDayKey = "";

  private localDayKey(): string {
    const d = new Date();
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${day}`;
  }

  private tickDailyTaskActivity() {
    const todayKey = this.localDayKey();
    if (!this.lastTrackedDayKey) {
      this.lastTrackedDayKey = todayKey;
    } else if (todayKey !== this.lastTrackedDayKey) {
      // Midnight (00:00) rollover: reset daily goals & daily progress score for the new day
      this.lastTrackedDayKey = todayKey;
      void this.initDailyGoalsAndRecap();
      void this.refreshStatsDashboard();
    }

    if (State.healthStatus?.isIdle) return;

    this.activityTickCounter += 1;
    if (this.activityTickCounter >= 15) {
      this.activityTickCounter = 0;
      const taskTitle = State.currentTaskRecord?.title || "Deep work session";
      const appClass =
        this.currentApp !== "other"
          ? this.currentApp
          : State.focusTask?.name || "workspace";
      void Bridge.taskActivityRecord(taskTitle, appClass, 15).then(() => {
        void this.refreshStatsDashboard();
      });
    }
  }

  private tick() {
    this.tickLayoutStressTest();
    this.detectActiveApp();
    this.tickDailyTaskActivity();
    this.tickFocusSession();
    this.tickBrowserSession();
    this.tickAgentMonitor();
    this.tickHealthMode();
    this.tickSystemMedia();
    this.checkNotifications();
  }

  private tickLayoutStressTest() {
    void Bridge.layoutStressTestStatus().then((st) => {
      if (!st) return;
      if (st.updatedAt === this.lastStressUpdatedAt && st.mode === State.layoutStressMode) {
        return;
      }
      this.lastStressUpdatedAt = st.updatedAt;
      const mode = st.enabled ? st.mode : "off";
      State.layoutStressMode = mode;

      if (st.sizePreset && (st.sizePreset === "compact" || st.sizePreset === "comfortable" || st.sizePreset === "large")) {
        State.settings.islandSize = st.sizePreset;
      }

      if (mode === "off") {
        State.stressExtraBadges = [];
        State.showingMorningRecap = false;
        State.workflowNotification = null;
        void this.syncCurrentTask();
        void this.initDailyGoalsAndRecap();
        State.notify();
        return;
      }

      if (mode === "on" || mode === "recap" || mode === "agent") {
        State.stressExtraBadges = [
          "🐚 cargo test --release · 14.2s ✓",
          "🐛 Fixed in 11m · rustc E0308",
          "🌐 docs.rs · Productive",
          "⚡ Dev Server :1420",
        ];
        State.currentTaskRecord = {
          id: 999,
          title: "Next focus milestone: Refactor Wayland layer-shell dynamic island geometry & ResizeObserver",
          status: "active",
          project: "coucou-desktop-wayland-companion-overhaul",
          createdAt: Math.floor(Date.now() / 1000),
          completedAt: null,
        };
      } else {
        State.stressExtraBadges = [];
      }

      if (mode === "on" || mode === "recap") {
        State.morningRecap = {
          day: "2026-10-11",
          shouldShowToday: true,
          lastProject: "coucou-desktop-wayland-companion-overhaul",
          lastFile: "src-tauri/src/platform/linux_wayland_layer_shell_geometry.rs",
          gitBranch: "feat/phase-d-dynamic-wayland-layer-shell-resize-observer",
          unfinishedTask: "Implement ResizeObserver + GTK layer-shell dynamic window sizing without clipping",
          focusMinutesRecent: 247,
          tasksCompletedRecent: 7,
          focusSessionsRecent: 6,
          agentRunsRecent: 14,
          goalsDone: 2,
          goalsTotal: 3,
        };
        State.showingMorningRecap = true;
        State.showingDailyGoalsCard = false;
        State.showingHealthCard = false;
        State.showingStatsCard = false;
        State.workflowNotification = null;
      } else if (mode === "agent") {
        State.showingMorningRecap = false;
        State.showingDailyGoalsCard = false;
        State.showingHealthCard = false;
        State.showingStatsCard = false;
        const longMsg =
          "🤖 Antigravity completed a 14-file architectural refactor across src-tauri/src/island.rs, src-tauri/src/platform/linux.rs, and src/core/layout.ts (branch feat/phase-d-dynamic-wayland-layer-shell-resize-observer) — all 4 SQLite & layout verification checks passed in 18.4s!";
        const longDetail =
          "Project: coucou-desktop-wayland-companion-overhaul · File: src-tauri/src/platform/linux_wayland_layer_shell_geometry.rs · Click ↗ to review diff";
        State.lastWorkflowQuote = longMsg;
        State.lastWorkflowDetail = longDetail;
        State.workflowNotification = {
          message: longMsg,
          detail: longDetail,
        };
      } else if (mode === "few") {
        State.showingMorningRecap = false;
        State.showingDailyGoalsCard = false;
        State.showingHealthCard = false;
        State.showingStatsCard = false;
        State.workflowNotification = null;
      }

      State.notify();
    });
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
          State.lastWorkflowQuote = State.healthStatus?.isNightMode
            ? getNightWindDownQuote()
            : getRandomQuote(newApp as QuoteCategory);
          if (newApp === "browser") {
            if (!State.browserSession) {
              State.browserPromptOpen = true;
            }
          } else {
            State.browserPromptOpen = false;
            State.browserNudgeToEditor = false;
          }
          State.notify();
        } else if (this.currentApp === "other" && State.focusTask) {
          const fallbackApp = this.classifyApp(
            State.focusTask.name,
            State.focusTask.windowCaption ?? "",
          );
          if (fallbackApp !== "other") {
            this.currentApp = fallbackApp;
            State.lastWorkflowQuote = State.healthStatus?.isNightMode
              ? getNightWindDownQuote()
              : getRandomQuote(fallbackApp as QuoteCategory);
            if (fallbackApp === "browser" && !State.browserSession) {
              State.browserPromptOpen = true;
            }
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
    if (State.healthStatus?.isIdle) return;

    const now = Date.now();
    const interval = Math.max(
      this.settings.quotesIntervalSeconds || 480,
      this.settings.workNotificationSeconds || 480,
    );
    const elapsedSinceQuote = (now - this.lastQuoteTime) / 1000;
    const elapsedSinceWork = (now - this.lastWorkNotificationTime) / 1000;

    if (elapsedSinceQuote < interval && elapsedSinceWork < interval) return;

    this.lastQuoteTime = now;
    this.lastWorkNotificationTime = now;

    // Always pick a fresh, non-repeating motivational quote for the active session
    // (or a calm wind-down quote when Night Mode is active)
    const category = this.resolveActiveCategory();
    const freshQuote = State.healthStatus?.isNightMode
      ? getNightWindDownQuote()
      : getRandomQuote(category);
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
    if (settings.quotesIntervalMinutes !== undefined) {
      this.settings.quotesIntervalSeconds = settings.quotesIntervalMinutes * 60;
    }
    if (settings.workNotificationMinutes !== undefined) {
      this.settings.workNotificationSeconds = settings.workNotificationMinutes * 60;
    }
    this.saveSettings();
  }
}

// Create and export the singleton instance
// This will start tracking as soon as the module is imported
export const Workflow = new WorkflowEngine();
