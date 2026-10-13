// Power features: desktop notifications, sound alerts, session export,
// quick replies, notification rules, local logging, session analytics.

import { State } from "./state";
import { Sound } from "./sound";
import { Bridge } from "./bridge";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface NotificationRule {
  /** Minimum seconds between notifications (cooldown) */
  cooldownSeconds: number;
  /** Only notify for these agent IDs (empty = all) */
  priorityAgents: string[];
  /** Quiet hours: notifications disabled between these hours (0-23) */
  quietHoursStart: number;
  quietHoursEnd: number;
  /** Show desktop notifications */
  desktopNotifications: boolean;
  /** Play sound on notification */
  soundOnNotification: boolean;
  /** Flash the island border */
  flashOnNotification: boolean;
}

export interface QuickReply {
  id: string;
  label: string;
  text: string;
}

export interface SessionLog {
  timestamp: number;
  agentId: string;
  agentName: string;
  event: string;
  detail: string;
  sessionId: string;
}

export interface AgentAnalytics {
  agentId: string;
  agentName: string;
  totalSessions: number;
  totalSteps: number;
  totalDurationMs: number;
  lastActive: number;
  projects: Set<string>;
}

export interface PowerSettings {
  notifications: NotificationRule;
  quickReplies: QuickReply[];
  enableLogging: boolean;
  enableAnalytics: boolean;
  logRetentionDays: number;
}

export const DEFAULT_POWER_SETTINGS: PowerSettings = {
  notifications: {
    cooldownSeconds: 5,
    priorityAgents: [],
    quietHoursStart: 22,
    quietHoursEnd: 7,
    desktopNotifications: true,
    soundOnNotification: true,
    flashOnNotification: true,
  },
  quickReplies: [
    { id: "continue", label: "Continue", text: "Continue with the current approach." },
    { id: "stop", label: "Stop", text: "Stop the current task." },
    { id: "explain", label: "Explain", text: "Explain what you're doing." },
    { id: "test", label: "Test", text: "Run the tests and show me the results." },
    { id: "commit", label: "Commit", text: "Commit the changes with a descriptive message." },
  ],
  enableLogging: true,
  enableAnalytics: true,
  logRetentionDays: 30,
};

// ── State ─────────────────────────────────────────────────────────────────────

class PowerEngine {
  settings: PowerSettings = { ...DEFAULT_POWER_SETTINGS };
  private lastNotificationTime = 0;
  private sessionLogs: SessionLog[] = [];
  private analytics: Map<string, AgentAnalytics> = new Map();
  private sessionStartTimes: Map<string, number> = new Map();
  private notificationPermission: NotificationPermission = "default";

  constructor() {
    this.loadSettings();
    this.requestNotificationPermission();
  }

  private loadSettings() {
    try {
      const raw = localStorage.getItem("coucou-power-settings");
      if (raw) {
        const parsed = JSON.parse(raw);
        this.settings = {
          ...DEFAULT_POWER_SETTINGS,
          ...parsed,
          notifications: { ...DEFAULT_POWER_SETTINGS.notifications, ...parsed.notifications },
        };
      }
    } catch {
      // Use defaults
    }
  }

  saveSettings() {
    try {
      localStorage.setItem("coucou-power-settings", JSON.stringify(this.settings));
    } catch {
      // Ignore
    }
  }

  private async requestNotificationPermission() {
    if (!("Notification" in window)) return;
    if (Notification.permission === "default") {
      this.notificationPermission = await Notification.requestPermission();
    } else {
      this.notificationPermission = Notification.permission;
    }
  }

  // ── Desktop Notifications ──────────────────────────────────────────────────

  shouldNotify(): boolean {
    const now = Date.now();
    const cooldown = this.settings.notifications.cooldownSeconds * 1000;
    if (now - this.lastNotificationTime < cooldown) return false;

    const hour = new Date().getHours();
    const { quietHoursStart, quietHoursEnd } = this.settings.notifications;
    if (quietHoursStart <= quietHoursEnd) {
      if (hour >= quietHoursStart && hour < quietHoursEnd) return false;
    } else {
      if (hour >= quietHoursStart || hour < quietHoursEnd) return false;
    }

    return true;
  }

  notify(title: string, body: string, agentId?: string) {
    if (!this.shouldNotify()) return;

    if (this.settings.notifications.priorityAgents.length > 0) {
      if (!agentId || !this.settings.notifications.priorityAgents.includes(agentId)) {
        return;
      }
    }

    this.lastNotificationTime = Date.now();

    if (this.settings.notifications.desktopNotifications && this.notificationPermission === "granted") {
      try {
        const n = new Notification(title, {
          body,
          icon: "/icon.png",
          tag: `coucou-${agentId ?? "general"}`,
          requireInteraction: false,
        });
        n.onclick = () => {
          window.focus();
          n.close();
        };
      } catch {
        // Ignore notification errors
      }
    }

    if (this.settings.notifications.soundOnNotification) {
      Sound.play("finish");
    }

    if (this.settings.notifications.flashOnNotification) {
      this.flashIsland();
    }

    this.logEvent(agentId ?? "general", "notification", title, body);
  }

  private flashIsland() {
    // No querySelector outside the island webview (the settings window, the
    // tests): there is nothing to flash there.
    if (typeof document === "undefined" || typeof document.querySelector !== "function") return;
    const island = document.querySelector("#island");
    if (!island) return;
    island.classList.add("flash-border");
    setTimeout(() => island.classList.remove("flash-border"), 1000);
  }

  // ── Sound Alerts ────────────────────────────────────────────────────────────

  playAlertSound(kind: "finish" | "error" | "approval" | "question" | "work") {
    const soundMap: Record<string, string> = {
      finish: "finish",
      error: "error",
      approval: "approval",
      question: "question",
      work: "work",
    };
    Sound.play(soundMap[kind] || "finish");
  }

  // ── Session Export ──────────────────────────────────────────────────────────

  exportSessionAsMarkdown(agentId?: string): string {
    const tasks = agentId
      ? State.tasks.filter((t) => t.id === agentId)
      : State.tasks;

    let md = "# Coucou Session Export\n\n";
    md += `Generated: ${new Date().toISOString()}\n\n`;

    for (const task of tasks) {
      md += `## ${task.name}\n\n`;
      md += `- **Agent ID:** ${task.id}\n`;
      md += `- **State:** ${task.state}\n`;
      if (task.sessionCwd) md += `- **Directory:** ${task.sessionCwd}\n`;
      if (task.sessionId) md += `- **Session ID:** ${task.sessionId}\n`;
      md += `\n### Steps\n\n`;
      for (const step of task.steps) {
        md += `- ${step}\n`;
      }
      if (task.finalLine) {
        md += `\n### Final Output\n\n${task.finalLine}\n`;
      }
      md += "\n---\n\n";
    }

    return md;
  }

  exportLogsAsJSON(): string {
    return JSON.stringify(this.sessionLogs, null, 2);
  }

  downloadFile(content: string, filename: string, type: string) {
    const blob = new Blob([content], { type });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
  }

  // ── Quick Replies ───────────────────────────────────────────────────────────

  getQuickReplies(): QuickReply[] {
    return this.settings.quickReplies;
  }

  addQuickReply(label: string, text: string) {
    this.settings.quickReplies.push({
      id: `qr-${Date.now()}`,
      label,
      text,
    });
    this.saveSettings();
  }

  removeQuickReply(id: string) {
    this.settings.quickReplies = this.settings.quickReplies.filter((q) => q.id !== id);
    this.saveSettings();
  }

  sendQuickReply(reply: QuickReply) {
    // Send to the focused agent's session
    const task = State.focusTask;
    if (!task) return;
    Bridge.log(`Quick reply sent: ${reply.label}`);
    this.logEvent(task.id, "quick-reply", reply.label, reply.text);
  }

  // ── Notification Rules ──────────────────────────────────────────────────────

  updateNotificationRules(rules: Partial<NotificationRule>) {
    this.settings.notifications = { ...this.settings.notifications, ...rules };
    this.saveSettings();
  }

  isAgentPriority(agentId: string): boolean {
    return this.settings.notifications.priorityAgents.includes(agentId);
  }

  togglePriorityAgent(agentId: string) {
    const agents = this.settings.notifications.priorityAgents;
    const idx = agents.indexOf(agentId);
    if (idx >= 0) {
      agents.splice(idx, 1);
    } else {
      agents.push(agentId);
    }
    this.saveSettings();
  }

  // ── Local Logging ───────────────────────────────────────────────────────────

  logEvent(agentId: string, event: string, detail: string, sessionId: string = "") {
    if (!this.settings.enableLogging) return;

    const task = State.tasks.find((t) => t.id === agentId);
    this.sessionLogs.push({
      timestamp: Date.now(),
      agentId,
      agentName: task?.name ?? agentId,
      event,
      detail,
      sessionId,
    });

    // Retention: keep only recent logs
    const cutoff = Date.now() - this.settings.logRetentionDays * 24 * 60 * 60 * 1000;
    this.sessionLogs = this.sessionLogs.filter((l) => l.timestamp > cutoff);

    // Cap at 10000 entries
    if (this.sessionLogs.length > 10000) {
      this.sessionLogs = this.sessionLogs.slice(-10000);
    }
  }

  getLogs(): SessionLog[] {
    return [...this.sessionLogs];
  }

  clearLogs() {
    this.sessionLogs = [];
  }

  // ── Session Analytics ───────────────────────────────────────────────────────

  recordSessionStart(agentId: string) {
    this.sessionStartTimes.set(agentId, Date.now());
    this.logEvent(agentId, "session-start", "Session started");
  }

  recordSessionEnd(agentId: string) {
    const start = this.sessionStartTimes.get(agentId);
    if (start) {
      const duration = Date.now() - start;
      this.sessionStartTimes.delete(agentId);
      this.logEvent(agentId, "session-end", `Session ended (${duration}ms)`);
    }
  }

  recordStep(agentId: string, _step: string) {
    let analytics = this.analytics.get(agentId);
    if (!analytics) {
      const task = State.tasks.find((t) => t.id === agentId);
      analytics = {
        agentId,
        agentName: task?.name ?? agentId,
        totalSessions: 0,
        totalSteps: 0,
        totalDurationMs: 0,
        lastActive: Date.now(),
        projects: new Set(),
      };
      this.analytics.set(agentId, analytics);
    }
    analytics.totalSteps++;
    analytics.lastActive = Date.now();
    const task = State.tasks.find((t) => t.id === agentId);
    if (task?.sessionCwd) {
      analytics.projects.add(task.sessionCwd);
    }
  }

  getAnalytics(): AgentAnalytics[] {
    return Array.from(this.analytics.values()).map((a) => ({
      ...a,
      projects: new Set(a.projects),
    }));
  }

  getTopAgents(limit: number = 5): AgentAnalytics[] {
    return this.getAnalytics()
      .sort((a, b) => b.totalSteps - a.totalSteps)
      .slice(0, limit);
  }

  getTopProjects(limit: number = 5): { project: string; count: number }[] {
    const projectCounts = new Map<string, number>();
    for (const analytics of this.analytics.values()) {
      for (const project of analytics.projects) {
        projectCounts.set(project, (projectCounts.get(project) ?? 0) + 1);
      }
    }
    return Array.from(projectCounts.entries())
      .map(([project, count]) => ({ project, count }))
      .sort((a, b) => b.count - a.count)
      .slice(0, limit);
  }

  // ── Integration with hooks ──────────────────────────────────────────────────

  onHookEvent(agentId: string, event: string, detail: string, sessionId: string) {
    this.logEvent(agentId, event, detail, sessionId);

    if (event === "SessionStart") {
      this.recordSessionStart(agentId);
    } else if (event === "SessionEnd") {
      this.recordSessionEnd(agentId);
    } else if (event === "Stop") {
      this.notify(`${this.getAgentName(agentId)} finished`, detail, agentId);
      this.playAlertSound("finish");
    } else if (event === "StopFailure") {
      this.notify(`${this.getAgentName(agentId)} error`, detail, agentId);
      this.playAlertSound("error");
    } else if (event === "PermissionRequest") {
      this.notify(`${this.getAgentName(agentId)} needs approval`, detail, agentId);
      this.playAlertSound("approval");
    } else if (event === "Notification") {
      this.notify(`${this.getAgentName(agentId)}`, detail, agentId);
      this.playAlertSound("question");
    }

    this.recordStep(agentId, detail);
  }

  private getAgentName(agentId: string): string {
    return State.tasks.find((t) => t.id === agentId)?.name ?? agentId;
  }
}

export const Power = new PowerEngine();
