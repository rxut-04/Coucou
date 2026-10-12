// App state — mirror of AppState.swift (the parts the island needs).

import type { BotEmoteName, BotStateName, IslandMode, IslandViewName } from "./layout";
import type { EyeShape } from "../mochi/engine";
import {
  DEFAULT_MAIN_PILL, HOST_OS, availablePills, orderPills, pillDefinition, sanitizeDeclared,
  toggleDeclared, type HostOs, type PillDefinition,
} from "./pills";
import type { CodexPlanUsage, PlanUsage } from "./plan";
import type { ProviderId } from "./providers";
import type { FileDiff } from "./diff";
import type { Bindings } from "./shortcuts";
import { DEFAULT_OUTFIT, type Outfit } from "../mochi/wardrobe";
import { getQuoteForWindow } from "./quotes";
import {
  Bridge,
  type AgentLiveStatus,
  type BatteryStatus,
  type BluetoothStatus,
  type DailyGoalRecord,
  type DomainCategory,
  type HealthStatusSnapshot,
  type MediaStatus,
  type MorningRecapData,
  type StatsDashboardData,
  type TaskRecord,
  type WeatherStatus,
  type WeeklyAgentWaitStats,
} from "./bridge";

export type AgentSource = "claudeCode" | "n8n" | "agent";
export type PillBadge = "approval" | "finished" | "error";

export interface DesktopWindow {
  id: string;
  resourceClass: string;
  caption: string;
  pid: number;
  active: boolean;
  minimized: boolean;
}

export interface ActiveFocusSession {
  id: number;
  remainingSecs: number;
  targetSecs: number;
  elapsedSecs: number;
  paused: boolean;
  taskTitle: string;
  appClass: string;
}

export interface ActiveBrowserSession {
  intent: "research" | "break";
  remainingSecs: number;
  targetSecs: number;
  elapsedSecs: number;
  startedAtMs: number;
}

export interface AgentTask {
  id: string;
  name: string;
  color: string;
  state: BotStateName;
  stepIndex: number;
  steps: string[];
  /**
   * Position of the newest step in the whole session. `steps` is capped, so
   * `stepIndex` stops moving once it is full; this keeps counting.
   */
  stepSeq?: number;
  source: AgentSource;
  isIntegration: boolean;
  emote?: BotEmoteName | null;
  miniEye?: EyeShape | null;
  pillBadge?: PillBadge | null;
  sessionCwd?: string | null;
  /** Claude Code's session, so "Open terminal" can find the window it runs in. */
  sessionId?: string | null;
  /** Claude's final message after Stop, one line; cleared when a new turn starts. */
  finalLine?: string | null;
  /** Desktop window is minimized (for visual indication). */
  minimized?: boolean;
  /** Clean caption/title for a desktop window pill. */
  windowCaption?: string;
  /** Whether the desktop window is currently active/focused. */
  windowActive?: boolean;
  /** Desktop window PID for KWin focus activation. */
  windowPid?: number;
  /** Desktop window resourceClass for KWin focus activation. */
  windowClass?: string;
}

export interface ApprovalInfo {
  requestId: string;
  sessionId: string;
  /** The pill the request belongs to: VS Code's or Cursor's (Claude Code), or an agent's (Codex…). */
  pillId: string;
  tool: string;
  command: string;
  /** Set when Claude Code is asking a question rather than for a permission. */
  questions?: AskedQuestion[];
}

/** One question of an AskUserQuestion call. */
export interface AskedQuestion {
  question: string;
  options: { label: string; description: string }[];
  multiSelect: boolean;
}

export interface ChatMessage {
  id: number;
  role: "user" | "assistant";
  content: string;
}

export type PromptContext =
  | { kind: "window"; appName: string; title: string; url?: string }
  | { kind: "file"; name: string; path?: string };

export interface ResultItem {
  label: string;
  detail: string;
  url?: string;
}

export interface SearchResult {
  title: string;
  items: ResultItem[];
  note?: string;
}

/** A fresh, idle task for a catalog pill. */
function taskFor(def: PillDefinition, name = def.name): AgentTask {
  return {
    id: def.id, name, color: def.color, state: "idle", stepIndex: 0, steps: [],
    source: def.source, isIntegration: true,
  };
}

/** What an integration poller last reported. */
export interface IntegrationInfo {
  data: Record<string, unknown>;
  error: string | null;
  loaded: boolean;
  configured: boolean;
}

export interface Settings {
  soundEnabled: boolean;
  soundVolume: number;
  autoCloseInterval: number;
  absenceInterval: number;
  /** Declared pills next to the main one (at most 4), in the order they were added. */
  activeIntegrations: string[];
  /** The always-on workspace pill: VS Code, Cursor, Codex or Antigravity. */
  mainPill: string;
  /** "primary", "cursor", or `at:<x>,<y>` for one display (logical origin). */
  screen: string;
  autostart: boolean;
  hooksInstalled: boolean;
  /** Claude model used by the chat. */
  model: string;
  /** Show the Claude plan pill (5 h and weekly limits) in the island's header. */
  showPlanInNotch: boolean;
  /** Coucou's status line relay is installed in Claude Code's settings. */
  planRelayInstalled: boolean;
  /** Show the Codex plan pill in the island's header. */
  showCodexPlanInNotch: boolean;
  /** Who the chat talks to (see core/providers.ts); picked in the chat view. */
  chatProvider: ProviderId;
  /** The model picked for each provider other than Anthropic, by provider id. */
  chatModels: Record<string, string>;
  /** Model server addresses once connected; empty means not connected. */
  ollamaUrl: string;
  lmstudioUrl: string;
  customUrl: string;
  /** Global shortcuts the user changed, by action id (see core/shortcuts.ts). */
  shortcuts: Bindings;
  /**
   * Mochi's outfit: "auto" (dresses for the season), "none" or an outfit id.
   * Same raw values as the Mac's "mochiOutfit"; read it through parseOutfit.
   */
  mochiOutfit: string;
  /**
   * Interface language: "" follows the system (when Coucou has its language,
   * else English), or one of src/i18n's ten codes ("fr", "pt-BR", "zh-Hans"…).
   */
  language: string;
  /** Island size preset ("compact" | "comfortable" | "large"), default "comfortable". */
  islandSize: string;
  /** Mochi on the desktop. Rust owns it: whatever the page sends back is ignored. */
  desktopMochi?: {
    onDesktop: boolean;
    spot: { x: number; y: number; space: string } | null;
  };
}

export const DEFAULT_SETTINGS: Settings = {
  soundEnabled: true,
  soundVolume: 0.12,
  autoCloseInterval: 15,
  absenceInterval: 180,
  activeIntegrations: [
    "integration_resend", "integration_n8n", "integration_vercel", "integration_github",
  ],
  mainPill: DEFAULT_MAIN_PILL,
  screen: "primary",
  autostart: false,
  hooksInstalled: false,
  model: "claude-opus-5",
  showPlanInNotch: false,
  planRelayInstalled: false,
  showCodexPlanInNotch: false,
  chatProvider: "anthropic",
  chatModels: {},
  ollamaUrl: "",
  lmstudioUrl: "",
  customUrl: "",
  shortcuts: {},
  mochiOutfit: DEFAULT_OUTFIT,
  language: "",
  islandSize: "comfortable",
};

function windowId(window: DesktopWindow): string {
  return window.id || `window-${window.pid}`;
}

function isRelevantDesktopWindow(window: DesktopWindow): boolean {
  if (!window) return false;
  if (window.minimized && !window.active) return false;
  const cls = (window.resourceClass || "").trim().toLowerCase();
  const cap = (window.caption || "").trim().toLowerCase();
  if (!cls && !cap) return false;
  if (window.pid <= 0 && !cls) return false;
  // Filter out Coucou itself and KDE/Wayland shell/background windows so clicking
  // the island or the desktop never creates an inappropriate/dull pill.
  const ignoredClasses = [
    "coucou",
    "plasmashell",
    "org.kde.plasmashell",
    "kwin",
    "kwin_wayland",
    "krunner",
    "org.kde.krunner",
    "xdg-desktop-portal",
    "polkit",
    "org.kde.polkit",
    "xwaylandvideobridge",
    "gmenudbusmenuproxy",
    "kded",
    "ksmserver",
  ];
  if (ignoredClasses.some((ig) => cls === ig || cls.includes(ig))) return false;
  if (cap === "coucou" || cap === "desktop" || cap.startsWith("plasma")) return false;
  return true;
}

function windowLabel(resourceClass: string, caption = "", fallback = "Window"): string {
  const cls = (resourceClass || "").trim().toLowerCase();
  const cap = (caption || "").trim();
  const capLower = cap.toLowerCase();
  const text = `${cls} ${capLower}`;

  // 1. Check specific IDE / AI / Terminal classes first so window captions never mislabel them
  if (cls.includes("antigravity")) return "Antigravity";
  if (cls.includes("kiro")) return "Kiro";
  if (cls.includes("chatgpt") || cls.includes("openai")) return "ChatGPT";
  if (cls.includes("claude")) return "Claude";
  if (cls.includes("konsole")) return "Konsole";
  if (
    cls.includes("gnome-terminal") ||
    cls.includes("ptyxis") ||
    cls.includes("kgx") ||
    cls.includes("kitty") ||
    cls.includes("alacritty") ||
    cls.includes("wezterm") ||
    cls.includes("foot") ||
    cls.includes("tilix") ||
    cls.includes("terminator") ||
    cls.includes("xterm") ||
    cls.includes("yakuake") ||
    cls.includes("guake") ||
    cls.includes("warp") ||
    cls.includes("ghostty") ||
    cls.includes("terminal") ||
    cls.includes("bash") ||
    cls.includes("zsh") ||
    cls.includes("fish")
  ) {
    return "Terminal";
  }
  if (cls.includes("cursor")) return "Cursor";
  if (cls.includes("code") || cls.includes("vscode")) return "VS Code";

  // 2. Check combined class + caption for web apps, PWAs, or title-based detection
  if (text.includes("antigravity")) return "Antigravity";
  if (text.includes("kiro")) return "Kiro";
  if (text.includes("chatgpt") || text.includes("openai")) return "ChatGPT";
  if (text.includes("claude")) return "Claude";
  if (text.includes("konsole")) return "Konsole";
  if (
    text.includes("terminal") ||
    text.includes("gnome-terminal") ||
    text.includes("ptyxis") ||
    text.includes("kitty") ||
    text.includes("alacritty") ||
    text.includes("wezterm") ||
    text.includes("bash") ||
    text.includes("zsh")
  ) {
    return "Terminal";
  }
  if (text.includes("cursor")) return "Cursor";
  if (text.includes("code") || text.includes("vscode")) return "VS Code";
  if (
    text.includes("brave") ||
    text.includes("chrome") ||
    text.includes("chromium") ||
    text.includes("firefox") ||
    text.includes("edge") ||
    text.includes("vivaldi") ||
    text.includes("opera") ||
    text.includes("zen") ||
    text.includes("browser")
  ) {
    return "Browser";
  }
  if (text.includes("dolphin")) return "Files";

  const raw = cap || (resourceClass || "").trim();
  if (!raw) return fallback;
  const short = raw.split(/\s+[—–-]\s+/)[0].trim() || raw;
  return short.length > 18 ? `${short.slice(0, 16).trimEnd()}…` : short;
}

function windowColor(resourceClass: string, caption = ""): string {
  const cls = (resourceClass || "").trim().toLowerCase();
  const text = `${cls} ${(caption || "").toLowerCase()}`;

  if (cls.includes("antigravity") || text.includes("antigravity")) return "#E879F9";
  if (cls.includes("kiro") || text.includes("kiro")) return "#A855F7";
  if (cls.includes("chatgpt") || cls.includes("openai") || text.includes("chatgpt")) return "#10A37F";
  if (cls.includes("claude") || text.includes("claude")) return "#E07950";
  if (cls.includes("konsole") || text.includes("konsole")) return "#f59e0b";
  if (
    cls.includes("terminal") ||
    cls.includes("gnome-terminal") ||
    cls.includes("ptyxis") ||
    cls.includes("kgx") ||
    cls.includes("kitty") ||
    cls.includes("alacritty") ||
    cls.includes("wezterm") ||
    cls.includes("foot") ||
    cls.includes("tilix") ||
    cls.includes("terminator") ||
    cls.includes("xterm") ||
    cls.includes("bash") ||
    cls.includes("zsh") ||
    text.includes("terminal")
  ) {
    return "#10b981";
  }
  if (cls.includes("cursor") || text.includes("cursor")) return "#C0C4CC";
  if (cls.includes("code") || cls.includes("vscode") || text.includes("vscode")) return "#3b82f6";
  if (
    text.includes("brave") ||
    text.includes("chrome") ||
    text.includes("chromium") ||
    text.includes("firefox") ||
    text.includes("edge") ||
    text.includes("browser")
  ) {
    return "#22c55e";
  }
  return "#8AB4F8";
}

function buildWindowTask(window: DesktopWindow): AgentTask {
  const id = windowId(window);
  const name = windowLabel(window.resourceClass, window.caption, "Window");
  const cleanCaption = (window.caption || "").trim() || name;
  return {
    id,
    name,
    color: windowColor(window.resourceClass, window.caption),
    state: window.active ? "working" : "idle",
    stepIndex: 0,
    steps: [],
    source: "agent",
    isIntegration: false,
    minimized: window.minimized,
    windowCaption: cleanCaption,
    windowActive: window.active,
    windowPid: window.pid,
    windowClass: window.resourceClass,
  };
}

function windowPriority(value: string): number {
  const text = (value || "").toLowerCase();
  if (
    text.includes("antigravity") ||
    text.includes("kiro") ||
    text.includes("code") ||
    text.includes("cursor") ||
    text.includes("vscode")
  ) {
    return 0;
  }
  if (text.includes("claude") || text.includes("chatgpt") || text.includes("openai")) {
    return 1;
  }
  if (
    text.includes("konsole") ||
    text.includes("terminal") ||
    text.includes("gnome-terminal") ||
    text.includes("ptyxis") ||
    text.includes("kitty") ||
    text.includes("alacritty") ||
    text.includes("wezterm") ||
    text.includes("bash") ||
    text.includes("zsh")
  ) {
    return 2;
  }
  if (
    text.includes("brave") ||
    text.includes("chrome") ||
    text.includes("chromium") ||
    text.includes("firefox") ||
    text.includes("edge") ||
    text.includes("browser")
  ) {
    return 3;
  }
  return 4;
}

type Listener = () => void;

/** Live diffs kept per pill (oldest dropped first) — same cap as macOS. */
export const MAX_DIFFS_PER_PILL = 50;
/** A pill's diffs are forgotten after an hour without a new one, as on macOS. */
export const DIFF_TTL_MS = 3_600_000;

class AppState {
  mode: IslandMode = "hidden";
  view: IslandViewName = "overview";

  tasks: AgentTask[] = [];
  focusId: string | null = null;
  desktopWindows: DesktopWindow[] = [];
  private lastActiveWindowId: string | null = null;

  stateOverride: BotStateName | null = null;

  /** Cursor in logical screen pixels, origin top-left (like AppState.mousePosition). */
  mouse = { x: 0, y: 0 };
  /** Cursor relative to the island's top-left corner. */
  mouseInIsland = { x: 0, y: 0 };
  /** Logical dimensions of the monitor the island lives on (from Bridge.boot). */
  screenSize = { width: 1920, height: 1080, scale: 1 };

  isPinned = false;
  paused = false;

  uploadProgress = 0;
  uploadDuration = 2.4;
  fileDragOver = false;

  promptContext: PromptContext | null = null;
  droppedFile: { name: string; path: string } | null = null;
  noteMessage: string | null = null;
  searchResult: SearchResult | null = null;
  chatHistory: ChatMessage[] = [];
  pendingApproval: ApprovalInfo | null = null;
  /** The pill that was in front when the card came up; it comes back after. */
  focusBeforeApproval: string | null = null;

  integrations: Record<string, IntegrationInfo> = {};

  /** Claude's 5 h / weekly limits, from the status line (null until the first call). */
  planUsage: PlanUsage | null = null;
  /** Codex's limits, from `codex app-server` (null until it has answered). */
  codexPlanUsage: CodexPlanUsage | null = null;
  /** A plan card is open in place of the overview's left card. */
  showingPlanDetail = false;
  /** Which one: the Codex card rather than Claude's. */
  planDetailIsCodex = false;
  /** Per-pill file diffs, in order of reception. Steps carry their ids. */
  sessionDiffs = new Map<string, FileDiff[]>();
  private sessionDiffTimers = new Map<string, number>();
  /** Never reset, so an id can never point at a newer diff than the one tapped. */
  private nextDiffId = 0;
  /** Workflow notification to display in the island. */
  workflowNotification: import("./workflow").WorkflowNotification | null = null;
  /** Latest motivational quote so the main left pill always displays it. */
  lastWorkflowQuote: string | null = null;
  /** Latest workflow timer/detail string (never overwrites the motivational quote). */
  lastWorkflowDetail: string | null = null;
  /** Current active task stored in SQLite (Phase 3). */
  currentTaskRecord: TaskRecord | null = null;
  /** Active 25-minute focus session countdown state (Phase 3). */
  focusSession: ActiveFocusSession | null = null;
  /** Phase 4: Latest domain reported by Brave extension & its classified category. */
  browserDomain: { domain: string; category: DomainCategory } | null = null;
  /** Phase 4: Whether the "Research or break?" prompt is open on browser focus. */
  browserPromptOpen = false;
  /** Phase 4: Active timed browser session (Research 15m or Break 5m). */
  browserSession: ActiveBrowserSession | null = null;
  /** Phase 4: True when a browser session timer has finished and is nudging back to editor. */
  browserNudgeToEditor = false;
  /** Phase 4: Most recently active editor/terminal window so we can jump right back. */
  lastEditorWindow: { id: string; pid: number; resourceClass: string; name: string } | null = null;
  /** Phase 5: Currently working or recently finished AI agent (Antigravity, Kiro, OpenCode). */
  activeAgentWait: AgentLiveStatus | null = null;
  /** Phase 5: Weekly agent wait time statistics from SQLite. */
  weeklyAgentWaitStats: WeeklyAgentWaitStats = {
    totalWaitSecs: 0,
    totalSessions: 0,
    antigravitySecs: 0,
    kiroSecs: 0,
    opencodeSecs: 0,
  };
  /** Phase 6: Up to 3 daily goals stored in SQLite (`1/3`). */
  dailyGoals: DailyGoalRecord[] = [];
  /** Phase 6: Morning "Where did I leave off" recap payload. */
  morningRecap: MorningRecapData | null = null;
  /** Phase 6: Whether the Morning Recap card is open in the main left card. */
  showingMorningRecap = false;
  /** Phase 6: Whether the interactive Daily Goals card is open in the main left card. */
  showingDailyGoalsCard = false;
  /** Phase 7: Night & Health Mode status snapshot (idle, night wind-down, eye/water/posture timers). */
  healthStatus: HealthStatusSnapshot | null = null;
  /** Phase 7: Whether the interactive Night & Health Mode card is open in the main left card. */
  showingHealthCard = false;
  /** Phase 8: Daily focus score (0-100), XP, level, streak, weekly insight, and 7-day chart. */
  statsDashboard: StatsDashboardData | null = null;
  /** Phase 8: Whether the Stats & Chart view card is open in the main left card. */
  showingStatsCard = false;
  /** Whether the Pomodoro / Focus Timer control card is open in the main left card. */
  showingFocusTimerCard = false;
  /** Whether the Animated Weather Forecast card is open in the main left card. */
  showingWeatherCard = false;
  /** Which day index (0..3) is selected in the Weather Forecast card. */
  selectedWeatherDayIdx = 0;
  /** Live weather & 4-day forecast status. */
  weatherStatus: WeatherStatus | null = null;
  /** Live battery & charging status (/sys/class/power_supply). */
  batteryStatus: BatteryStatus | null = null;
  /** Live connected Bluetooth device & battery status (BlueZ DBus). */
  bluetoothStatus: BluetoothStatus | null = null;
  /** Incremented whenever the user manually requests to show the Bluetooth dynamic bar. */
  bluetoothToastTriggerSeq = 0;
  /** Live MPRIS / browser media playback status (playerctl). */
  mediaStatus: MediaStatus | null = null;
  /** Phase A-F: Layout stress test mode ("off" | "on" | "recap" | "agent" | "few" | "compact"). */
  layoutStressMode = "off";
  /** Phase A-F: Extra status badges injected during layout stress test to reach 12+ badges. */
  stressExtraBadges: string[] = [];
  /**
   * Mochi is out of the island — on the desktop, flying, or being dragged
   * there — so the island's own Mochi is hidden (AppState.mochiOnDesktop).
   */
  mochiOnDesktop = false;

  /** Outfit shown on Mochi while the pointer rests on a wardrobe button. */
  wardrobePreview: Outfit | null = null;

  lastActivity = performance.now();

  settings: Settings = { ...DEFAULT_SETTINGS };

  /** Which pills this build offers depends on it (Claude Desktop is Windows only). */
  os: HostOs = HOST_OS;

  private listeners = new Set<Listener>();

  subscribe(fn: Listener): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  /** Marks the UI dirty; the island re-renders on the next frame. */
  notify() {
    for (const fn of this.listeners) fn();
  }

  get focusTask(): AgentTask | null {
    if (this.desktopWindows.length > 0) {
      const byFocus = this.focusId
        ? this.desktopWindows.find((w) => windowId(w) === this.focusId)
        : undefined;
      const focused = byFocus ?? this.desktopWindows.find((w) => w.active) ?? this.desktopWindows[0];
      return buildWindowTask(focused);
    }
    return this.tasks.find((t) => t.id === this.focusId) ?? this.tasks[0] ?? null;
  }

  get effectiveState(): BotStateName {
    return this.stateOverride ?? this.focusTask?.state ?? "idle";
  }

  get otherTasks(): AgentTask[] {
    if (this.layoutStressMode === "on" || this.layoutStressMode === "recap" || this.layoutStressMode === "agent") {
      const stressWindows: AgentTask[] = [
        {
          id: "stress-win-antigravity",
          name: "Antigravity",
          color: "#E879F9",
          state: "working",
          stepIndex: 0,
          steps: [],
          source: "agent",
          isIntegration: false,
          pillBadge: "finished",
          windowCaption: "COUCOU — Antigravity IDE",
          windowClass: "antigravity",
        },
        {
          id: "stress-win-vscode",
          name: "VS Code",
          color: "#3B82F6",
          state: "working",
          stepIndex: 0,
          steps: [],
          source: "agent",
          isIntegration: false,
          windowCaption: "views.ts — Coucou",
          windowClass: "code",
        },
        {
          id: "stress-win-konsole",
          name: "Konsole",
          color: "#F59E0B",
          state: "idle",
          stepIndex: 0,
          steps: [],
          source: "agent",
          isIntegration: false,
          pillBadge: "finished",
          windowCaption: "cargo test --release",
          windowClass: "org.kde.konsole",
        },
        {
          id: "stress-win-browser",
          name: "Browser",
          color: "#22C55E",
          state: "idle",
          stepIndex: 0,
          steps: [],
          source: "agent",
          isIntegration: false,
          windowCaption: "docs.rs — Brave",
          windowClass: "brave-browser",
        },
        {
          id: "stress-win-kiro",
          name: "Kiro",
          color: "#A855F7",
          state: "working",
          stepIndex: 0,
          steps: [],
          source: "agent",
          isIntegration: false,
          pillBadge: "approval",
          windowCaption: "Kiro Agent — Architecture spec",
          windowClass: "kiro",
        },
        {
          id: "stress-win-terminal",
          name: "Terminal",
          color: "#10B981",
          state: "idle",
          stepIndex: 0,
          steps: [],
          source: "agent",
          isIntegration: false,
          windowCaption: "zsh — /run/media/om-raut/COUCOU",
          windowClass: "Alacritty",
        },
        {
          id: "stress-win-claude",
          name: "Claude",
          color: "#E07950",
          state: "idle",
          stepIndex: 0,
          steps: [],
          source: "agent",
          isIntegration: false,
          windowCaption: "Claude Desktop",
          windowClass: "claude",
        },
        {
          id: "stress-win-chatgpt",
          name: "ChatGPT",
          color: "#10A37F",
          state: "idle",
          stepIndex: 0,
          steps: [],
          source: "agent",
          isIntegration: false,
          windowCaption: "ChatGPT Desktop",
          windowClass: "chatgpt",
        },
      ];
      return stressWindows;
    }
    if (this.desktopWindows.length > 0) {
      const focusedId = this.focusTask?.id;
      const others = this.desktopWindows
        .filter((w) => windowId(w) !== focusedId)
        .slice()
        .sort((a, b) => {
          const p = windowPriority(`${a.resourceClass} ${a.caption}`) - windowPriority(`${b.resourceClass} ${b.caption}`);
          if (p !== 0) return p;
          return (a.caption || a.resourceClass).localeCompare(b.caption || b.resourceClass);
        })
        .map(buildWindowTask);
      return others;
    }
    return this.tasks.filter((t) => t.id !== this.focusId);
  }

  applyDesktopWindows(windows: DesktopWindow[]) {
    const filtered = windows.filter(isRelevantDesktopWindow);
    const seen = new Set<string>();
    const deduped: DesktopWindow[] = [];
    for (const w of filtered) {
      const id = windowId(w);
      if (seen.has(id)) continue;
      seen.add(id);
      deduped.push({ ...w });
    }

    const prevActiveId = this.lastActiveWindowId;
    const explicitActive = deduped.find((w) => w.active);
    if (explicitActive) {
      const nextActiveId = windowId(explicitActive);
      this.lastActiveWindowId = nextActiveId;
      if (nextActiveId !== prevActiveId) {
        this.lastWorkflowQuote = getQuoteForWindow(explicitActive.resourceClass, explicitActive.caption);
      }
      const clsLower = (explicitActive.resourceClass || "").toLowerCase();
      if (
        clsLower.includes("antigravity") ||
        clsLower.includes("kiro") ||
        clsLower.includes("code") ||
        clsLower.includes("cursor") ||
        clsLower.includes("konsole")
      ) {
        this.lastEditorWindow = {
          id: explicitActive.id,
          pid: explicitActive.pid,
          resourceClass: explicitActive.resourceClass,
          name: windowLabel(explicitActive.resourceClass, explicitActive.caption, "Editor"),
        };
      }
    } else if (this.lastActiveWindowId) {
      // When the user clicks Coucou or the desktop, KWin marks Coucou/plasmashell
      // active; preserve the active state on the last active user window.
      const prev = deduped.find((w) => windowId(w) === this.lastActiveWindowId);
      if (prev) prev.active = true;
    }

    const next = deduped.sort((a, b) => {
      if (a.active !== b.active) return Number(b.active) - Number(a.active);
      const p = windowPriority(`${a.resourceClass} ${a.caption}`) - windowPriority(`${b.resourceClass} ${b.caption}`);
      if (p !== 0) return p;
      return (a.caption || a.resourceClass).localeCompare(b.caption || b.resourceClass);
    });

    this.desktopWindows = next;
    if (this.desktopWindows.length > 0) {
      const active = this.desktopWindows.find((w) => w.active);
      if (active) {
        this.focusId = windowId(active);
      } else if (!this.focusId || !this.desktopWindows.some((w) => windowId(w) === this.focusId)) {
        this.focusId = windowId(this.desktopWindows[0]);
      }
    } else if (!this.focusId || !this.tasks.some((t) => t.id === this.focusId)) {
      this.focusId = this.mainPillId;
    }
    this.notify();
  }

  setFocus(id: string) {
    const t = this.tasks.find((x) => x.id === id);
    if (t) {
      this.focusId = id;
      this.showingPlanDetail = false;
      t.pillBadge = null;
      this.lastWorkflowQuote = getQuoteForWindow(t.name, t.windowCaption ?? "");
      this.notify();
      return;
    }
    const win = this.desktopWindows.find((w) => windowId(w) === id);
    if (win) {
      for (const w of this.desktopWindows) {
        w.active = windowId(w) === id;
      }
      win.minimized = false;
      this.focusId = id;
      this.lastActiveWindowId = id;
      this.lastWorkflowQuote = getQuoteForWindow(win.resourceClass, win.caption);
      this.notify();
      void Bridge.focusTrackerActivateWindow(win.id, win.pid, win.resourceClass);
    }
  }

  /**
   * A permission card or a question comes up: its pill comes to the front, and
   * the pill that was there is remembered (HookServer.focusBeforeApproval).
   */
  beginApproval(info: ApprovalInfo) {
    this.pendingApproval = info;
    this.isPinned = true;
    if (this.focusBeforeApproval == null) this.focusBeforeApproval = this.focusId;
    this.setFocus(info.pillId);
  }

  /**
   * The card has its answer, or is withdrawn: the session carries on, and the
   * pill you were on comes back — unless you moved to another one meanwhile.
   */
  endApproval() {
    const req = this.pendingApproval;
    if (!req) return;
    this.pendingApproval = null;
    this.isPinned = false;
    this.updateTask(req.pillId, "working");
    this.setPillBadge(req.pillId, null);
    const previous = this.focusBeforeApproval;
    this.focusBeforeApproval = null;
    if (previous && this.focusId === req.pillId && this.tasks.some((t) => t.id === previous)) {
      this.focusId = previous;
    }
    this.notify();
  }

  updateTask(id: string, state: BotStateName) {
    const t = this.tasks.find((x) => x.id === id);
    if (!t) return;
    t.state = state;
    this.notify();
  }

  appendStep(id: string, step: string) {
    const t = this.tasks.find((x) => x.id === id);
    if (!t) return;
    const newest = t.stepSeq ?? t.steps.length - 1;
    t.steps.push(step);
    if (t.steps.length > 20) t.steps.shift();
    t.stepIndex = t.steps.length - 1;
    t.stepSeq = newest + 1;
    this.notify();
  }

  setPillBadge(id: string, badge: PillBadge | null) {
    const t = this.tasks.find((x) => x.id === id);
    if (!t) return;
    t.pillBadge = badge;
    this.notify();
  }

  /** Stores a diff for a pill and returns its id (for the ticker step). */
  appendSessionDiff(pillId: string, diff: FileDiff): number {
    const id = this.nextDiffId++;
    const list = this.sessionDiffs.get(pillId) ?? [];
    list.push({ ...diff, id });
    while (list.length > MAX_DIFFS_PER_PILL) list.shift();
    this.sessionDiffs.set(pillId, list);
    // One timer per pill, re-armed on every diff — nothing polls.
    const prev = this.sessionDiffTimers.get(pillId);
    if (prev != null) window.clearTimeout(prev);
    this.sessionDiffTimers.set(
      pillId,
      window.setTimeout(() => this.clearSessionDiffs(pillId), DIFF_TTL_MS),
    );
    return id;
  }

  findDiff(pillId: string, id: number): FileDiff | null {
    return this.sessionDiffs.get(pillId)?.find((d) => d.id === id) ?? null;
  }

  clearSessionDiffs(pillId: string) {
    const timer = this.sessionDiffTimers.get(pillId);
    if (timer != null) window.clearTimeout(timer);
    this.sessionDiffTimers.delete(pillId);
    this.sessionDiffs.delete(pillId);
  }

  /** The always-on workspace pill, once the setting has been checked. */
  get mainPillId(): string {
    return sanitizeDeclared(this.settings, this.os).mainPill;
  }

  /**
   * True for a pill that stays when its session ends: the main pill and the
   * declared ones go back to idle instead of going away.
   */
  isKept(id: string): boolean {
    const d = sanitizeDeclared(this.settings, this.os);
    return id === d.mainPill || d.activeIntegrations.includes(id);
  }

  /**
   * Loads the catalog pills: the main pill always, the declared ones, and none
   * of the others — a pill that is mid-session stays until its session ends.
   * Safe to call any number of times. AppState.loadIntegrationTasks on macOS.
   */
  loadIntegrationTasks() {
    const d = sanitizeDeclared(this.settings, this.os);
    this.settings.mainPill = d.mainPill;
    this.settings.activeIntegrations = d.activeIntegrations;
    for (const def of availablePills(this.os)) {
      const shouldLoad = def.id === d.mainPill || d.activeIntegrations.includes(def.id);
      const idx = this.tasks.findIndex((t) => t.id === def.id);
      if (shouldLoad && idx < 0) this.tasks.push(taskFor(def));
      const busy = idx >= 0 && (this.tasks[idx].state !== "idle" || this.tasks[idx].steps.length > 0);
      if (!shouldLoad && idx >= 0 && !busy) this.tasks.splice(idx, 1);
    }
    this.tasks = orderPills(this.tasks, d.mainPill);
    if (!this.focusId || !this.tasks.some((t) => t.id === this.focusId)) this.focusId = d.mainPill;
    this.notify();
  }

  /**
   * A session is over. The main and declared pills are put back as they were;
   * any other pill goes away (AppState.removeTask on macOS).
   */
  removeTask(id: string) {
    const idx = this.tasks.findIndex((t) => t.id === id);
    if (idx < 0) return;
    if (this.isKept(id)) {
      const t = this.tasks[idx];
      t.state = "idle";
      t.steps = [];
      t.stepIndex = 0;
      delete t.stepSeq;
      t.pillBadge = null;
      t.finalLine = null;
      const def = pillDefinition(id);
      if (def) t.name = def.name;
      this.clearSessionDiffs(id);
      this.notify();
      return;
    }
    this.tasks.splice(idx, 1);
    this.clearSessionDiffs(id);
    if (this.focusId === id) this.focusId = this.tasks[0]?.id ?? this.mainPillId;
    this.notify();
  }

  /**
   * Creates the pill of a tagged agent on its first event; no-op if it exists.
   * Inserted right after the main pill so it is in the visible slice(0,4). A
   * catalog agent wears its catalog colour, as on macOS.
   */
  upsertExternalAgent(id: string, name: string, color: string) {
    if (this.tasks.some((t) => t.id === id)) return;
    const def = pillDefinition(id);
    this.insertAfterMain({
      id, name, color: def?.color ?? color,
      state: "idle", stepIndex: 0, steps: [],
      source: "agent", isIntegration: false,
    });
  }

  /**
   * The pill a Claude Code session belongs to (VS Code or Cursor). It is made
   * for the session when it is neither the main pill nor declared, as
   * upsertWorkspaceTask does on macOS.
   */
  upsertWorkspacePill(id: string, name: string, cwd: string): AgentTask | null {
    let t = this.tasks.find((x) => x.id === id);
    if (!t) {
      const def = pillDefinition(id);
      if (!def) return null;
      t = taskFor(def, name);
      this.insertAfterMain(t);
    }
    t.name = name;
    if (cwd) t.sessionCwd = cwd;
    return t;
  }

  private insertAfterMain(t: AgentTask) {
    const at = this.tasks.findIndex((x) => x.id === this.mainPillId) + 1;
    this.tasks.splice(at, 0, t);
    if (!this.focusId) this.focusId = t.id;
    this.notify();
  }

  /** Declares or undeclares a pill (max 4 next to the main one). */
  toggleIntegration(id: string) {
    const next = toggleDeclared(sanitizeDeclared(this.settings, this.os), id, this.os);
    if (!next) return;
    this.settings.activeIntegrations = next;
    if (!next.includes(id) && this.focusId === id) this.focusId = this.mainPillId;
    this.loadIntegrationTasks();
  }

  /**
   * What the island opens on. A card waiting for an answer comes first, so
   * reopening a folded island shows it again (Mac #117, #290).
   */
  defaultView(): IslandViewName {
    if (this.pendingApproval) return this.pendingApproval.questions ? "question" : "approval";
    return this.tasks.length === 0 ? "empty" : "overview";
  }
}

export const State = new AppState();
