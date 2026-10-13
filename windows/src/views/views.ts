// Island views — DOM ports of IslandViewContent.swift. Paddings, font sizes,
// colours and wording are copied from the Swift views so both platforms read
// identically.

import { h, svg, clear, dot } from "./dom";
import { ICONS } from "./icons";
import { Ticker } from "./ticker";
import { State, type AgentTask } from "../core/state";
import { washRGBA, type IslandViewName, type Wash } from "../core/layout";
import { createMiniBot, pruneMiniBots } from "../mochi/minibots";
import { buildPrompt } from "./chat";
import { buildChoose, buildUpload, buildUploading } from "./upload";
import { renderIntegrationCard, type IntegrationCardHooks } from "./integrations";
import { pillDefinition, sessionSubtitle } from "../core/pills";
import {
  PlanCard, buildPlanPill, claudePillVisible, codexPillVisible, planCardOpen, refreshCodexPlanUsage,
} from "./usage";
import { buildDiffCard } from "./diff";
import { lastTextStep } from "../core/diff";
import { Bridge } from "../core/bridge";
import { buildRecap } from "./recap";
import { buildWardrobe } from "./wardrobe";
import type { Outfit, OutfitSelection } from "../mochi/wardrobe";
import { language, t, tl, type Msg } from "../i18n/i18n";
import { Workflow, formatDurationCompact, formatMmSs } from "../core/workflow";

export interface ViewActions {
  setView(v: IslandViewName): void;
  /** "Cancel" on a dropped file: forgets it and goes back home. */
  cancelDrop(): void;
  /** Opens a file chooser dialog from the upload or chat view. */
  pickFile?(): void;
  collapse(): void;
  /** Folds a waiting card to the compact island without answering it. */
  foldApproval(): void;
  setFocus(id: string): void;
  openTerminal(): void;
  /** The ↗ button: opens whatever the focused pill points at. */
  openTarget(): void;
  openUrl(url: string): void;
  decide(d: "allow" | "deny"): void;
  /** Answers the question Claude Code / OpenCode / Antigravity asked: question text → chosen label. */
  answer(answers: Record<string, string | string[]>): void;
  /** Sends direct text/option input to a terminal PTY, OpenCode, or Antigravity prompt. */
  submitInput(text: string): void;
  /** Hands the pending request back to the terminal. */
  answerInTerminal(): void;
  toggleSound(): void;
  setVolume(v: number): void;
  setAutoClose(seconds: number): void;
  setIslandSize(preset: string): void;
  openSettingsWindow(): void;
  blip(): void;
  /** Wardrobe click: keeps the outfit ("auto" and "none" included). */
  chooseOutfit(selection: OutfitSelection): void;
  /** Wardrobe hover: shows an outfit on Mochi without keeping it; null ends it. */
  previewOutfit(outfit: Outfit | null): void;
}

export interface ViewHost {
  el: HTMLElement;
  sync(): void;
  /** Called when the view becomes active, for views with a text field. */
  focus?(): void;
  /** Called every frame while the view is on screen. True = needs another frame. */
  tick?(nowMs: number): boolean | void;
}

// ── Shared pieces ─────────────────────────────────────────────────────────────

function card(wash: Wash, ...children: (Node | string)[]): HTMLElement {
  const el = h("div", { class: wash ? "card wash" : "card" }, ...children);
  if (wash) el.style.setProperty("--wash", washRGBA(wash));
  return el;
}

function btn(
  label: string | Msg,
  kind: "primary" | "secondary",
  onClick: () => void,
  kbd?: string,
): HTMLElement {
  return h(
    "button",
    { class: `btn ${kind}`, onclick: onClick },
    h("span", { text: label }),
    kbd ? h("span", { class: "kbd", text: kbd }) : null,
  );
}

/** AgentWho — coloured dot + task name + grey label. */
function agentWho(task: AgentTask | null, label: string): HTMLElement {
  const row = h("div", { class: "who-row" });
  if (task) {
    row.append(dot(task.color, 8), h("span", { class: "n", text: task.name }));
  }
  row.append(h("span", { text: label }));
  return row;
}

function stack(padLeft: number, padRight: number, ...children: Node[]): HTMLElement {
  const el = h("div", { class: "stack" }, ...children);
  el.style.padding = `4px ${padRight}px 4px ${padLeft}px`;
  return el;
}

// ── Header ────────────────────────────────────────────────────────────────────

export function buildHeader(actions: ViewActions): ViewHost {
  const tabHome = h("button", { class: "tab", title: tl("Overview"), onclick: () => go("overview") }, svg(ICONS.house, 13));
  const tabChat = h("button", { class: "tab", title: tl("Ask"), onclick: () => go("prompt") }, svg(ICONS.bubble, 13));
  const tabDrop = h("button", { class: "tab", title: tl("Drop"), onclick: () => go("upload") }, svg(ICONS.plus, 13));

  const gearBtn = h("button", { title: tl("Settings"), onclick: () => go("settings") }, svg(ICONS.gear, 14));
  const soundBtn = h("button", { title: tl("Mute"), onclick: () => actions.toggleSound() }, svg(ICONS.speakerOn, 14));
  const collapseBtn = h(
    "button",
    {
      class: "header-collapse-btn",
      title: tl("Collapse island (Esc)"),
      onclick: (e: Event) => {
        e.stopPropagation();
        actions.blip();
        actions.collapse();
      },
    },
    svg(ICONS.chevronUp, 14, { stroke: 2.2 }),
  );
  // Plan usage pills (off by default): before the gear, Claude first, as on the Mac.
  const claudePill = buildPlanPill(false);
  const codexPill = buildPlanPill(true);
  const planPills = h("div", { class: "plan-pills" }, claudePill.el, codexPill.el);
  let codexShown = false;

  // ── Phase 3: Current Task Pill + Mark Task Done + 25m Focus Session ───────
  let editingTask = false;
  const taskLabelEl = h("span", {
    class: "p3-task-title",
    title: "Click to edit current task",
    onclick: (e: Event) => {
      e.stopPropagation();
      editingTask = true;
      void Bridge.focusWindow(true);
      syncPhase3Bar();
      taskInputEl.value = State.currentTaskRecord?.title || "Deep work session";
      taskInputEl.focus();
      taskInputEl.select();
    },
  });

  const taskInputEl = h("input", {
    class: "p3-task-input",
    type: "text",
    placeholder: "Current task...",
  }) as HTMLInputElement;

  const commitTaskEdit = () => {
    if (!editingTask) return;
    editingTask = false;
    const nextVal = taskInputEl.value.trim();
    void Bridge.focusWindow(false);
    if (nextVal) {
      void Workflow.setCurrentTask(nextVal);
    } else {
      syncPhase3Bar();
    }
  };

  taskInputEl.addEventListener("keydown", (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Enter") {
      e.preventDefault();
      commitTaskEdit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      editingTask = false;
      void Bridge.focusWindow(false);
      syncPhase3Bar();
    }
  });
  taskInputEl.addEventListener("blur", () => commitTaskEdit());

  const taskDoneBtn = h(
    "button",
    {
      class: "p3-btn p3-done-btn",
      title: "Mark task done",
      onclick: (e: Event) => {
        e.stopPropagation();
        void Workflow.completeCurrentTask();
      },
    },
    h("span", { text: "✓ Done" }),
  );

  const taskPillEl = h(
    "div",
    { class: "p3-pill p3-task-pill" },
    dot("#A855F7", 6),
    taskLabelEl,
    taskInputEl,
    taskDoneBtn,
  );

  const focusStartBtn = h(
    "button",
    {
      class: "p3-pill p3-focus-start",
      title: "Start 25-minute Pomodoro focus timer & open timer controls",
      onclick: (e: Event) => {
        e.stopPropagation();
        void Workflow.startFocusSession(1500);
      },
    },
    svg(ICONS.timer, 11),
    h("span", { text: "25m Focus" }),
  );

  const focusTimeSpan = h(
    "span",
    {
      class: "p3-focus-time",
      text: "25:00",
      title: "Click to open Pomodoro Timer controls in main pill",
    },
  );
  const focusPauseBtn = h(
    "button",
    {
      class: "p3-btn p3-pause-btn",
      title: "Pause or resume focus session",
      onclick: (e: Event) => {
        e.stopPropagation();
        Workflow.togglePauseFocusSession();
      },
    },
    h("span", { text: "⏸" }),
  );
  const focusCancelBtn = h(
    "button",
    {
      class: "p3-btn p3-cancel-btn",
      title: "Cancel focus session",
      onclick: (e: Event) => {
        e.stopPropagation();
        void Workflow.cancelFocusSession();
      },
    },
    h("span", { text: "✕" }),
  );

  const focusActivePill = h(
    "div",
    {
      class: "p3-pill p3-focus-active",
      title: "Click to toggle Pomodoro Timer controls in main pill",
      onclick: (e: Event) => {
        e.stopPropagation();
        Workflow.toggleFocusTimerCard();
      },
    },
    dot("#22c55e", 6),
    focusTimeSpan,
    focusPauseBtn,
    focusCancelBtn,
  );

  // ── Phase 4: Browser Intent ("Research or break?", timer, & Back to Editor) ─
  const browserResearchBtn = h(
    "button",
    {
      class: "p3-btn p4-research-btn",
      title: "Start 15-minute Research session",
      onclick: (e: Event) => {
        e.stopPropagation();
        Workflow.startBrowserSession("research", 900);
      },
    },
    h("span", { text: "🔍 Research" }),
  );

  const browserBreakBtn = h(
    "button",
    {
      class: "p3-btn p4-break-btn",
      title: "Start 5-minute Break session",
      onclick: (e: Event) => {
        e.stopPropagation();
        Workflow.startBrowserSession("break", 300);
      },
    },
    h("span", { text: "☕ Break" }),
  );

  const browserPromptPill = h(
    "div",
    { class: "p3-pill p4-prompt-pill" },
    h("span", { class: "p4-prompt-label", text: "Research or break?" }),
    browserResearchBtn,
    browserBreakBtn,
  );

  const browserSessionTimeSpan = h("span", { class: "p3-focus-time", text: "🔍 15:00" });
  const browserSessionCancelBtn = h(
    "button",
    {
      class: "p3-btn p3-cancel-btn",
      title: "End browser session",
      onclick: (e: Event) => {
        e.stopPropagation();
        Workflow.cancelBrowserSession();
      },
    },
    h("span", { text: "✕" }),
  );

  const browserSessionPill = h(
    "div",
    { class: "p3-pill p4-session-pill" },
    browserSessionTimeSpan,
    browserSessionCancelBtn,
  );

  const backToEditorBtn = h(
    "button",
    {
      class: "p3-pill p4-editor-btn",
      title: "Jump back to your editor",
      onclick: (e: Event) => {
        e.stopPropagation();
        void Workflow.jumpBackToEditor();
      },
    },
    h("span", { text: "↩ Editor" }),
  );

  // ── Phase 5: AI Agent Waiting Gap Pill ("Agent working" / "Agent done, review it")
  const agentWaitLabel = h("span", { class: "p5-agent-label", text: "🤖 Agent working" });
  const agentWaitPill = h(
    "button",
    {
      class: "p3-pill p5-agent-pill",
      title: "Click to jump to agent window",
      onclick: (e: Event) => {
        e.stopPropagation();
        void Workflow.focusAndAcknowledgeAgent();
      },
    },
    agentWaitLabel,
  );

  // ── Phase 6: Daily Goals Progress Pill ("🎯 1/3") & Morning Recap Button
  const dailyGoalsLabel = h("span", { class: "p6-goals-label", text: "🎯 0/3" });
  const dailyGoalsPill = h(
    "button",
    {
      class: "p3-pill p6-goals-pill",
      title: "Click to view & toggle today's 3 daily goals",
      onclick: (e: Event) => {
        e.stopPropagation();
        Workflow.toggleDailyGoalsCard();
      },
    },
    dailyGoalsLabel,
  );

  const morningRecapBtn = h(
    "button",
    {
      class: "p3-pill p6-recap-btn",
      title: "Show 'Where did I leave off' recap",
      onclick: (e: Event) => {
        e.stopPropagation();
        void Workflow.triggerMorningRecapDemo();
      },
    },
    h("span", { text: "🌅" }),
  );

  // ── Phase 7: Night & Health Mode Pill ("🌿" / "🌙" / "💤")
  const healthModeLabel = h("span", { class: "p7-health-label", text: "🌿" });
  const healthModePill = h(
    "button",
    {
      class: "p3-pill p7-health-pill",
      title: "Night & Health Mode (Eye 20m · Water 45m · Posture 60m · Night 23:00)",
      onclick: (e: Event) => {
        e.stopPropagation();
        Workflow.toggleHealthCard();
      },
    },
    healthModeLabel,
  );

  // ── Phase 8: Daily Score, XP, Level & Streak Pill ("🏆 84 · L3 🔥1d")
  const statsPillLabel = h("span", { class: "p8-stats-label", text: "🏆 --" });
  const statsPill = h(
    "button",
    {
      class: "p3-pill p8-stats-pill",
      title: "Daily Focus Score, Streaks, XP, Levels & 7-Day Chart",
      onclick: (e: Event) => {
        e.stopPropagation();
        Workflow.toggleStatsCard();
      },
    },
    statsPillLabel,
  );

  // ── Live Weather Forecast Pill ("⛅ 34° · Clouds")
  const weatherPillLabel = h("span", { class: "p3-wx-label", text: "⛅ 34°" });
  const weatherPill = h(
    "button",
    {
      class: "p3-pill p3-wx-pill",
      title: "Click to open animated Weather & 4-Day Forecast in main pill",
      onclick: (e: Event) => {
        e.stopPropagation();
        Workflow.toggleWeatherCard();
      },
    },
    weatherPillLabel,
  );

  // ── Bluetooth Connected Pill ("🎧 Noise Pop Buds · 85%")
  const bluetoothPillLabel = h("span", { class: "p3-bt-label", text: "🎧 Connected" });
  const bluetoothPill = h(
    "button",
    {
      class: "p3-pill p3-bt-pill",
      style: "display:none",
      title: "Bluetooth connected — Click to preview Bluetooth Dynamic Bar",
      onclick: (e: Event) => {
        e.stopPropagation();
        State.bluetoothToastTriggerSeq += 1;
        actions.collapse();
        State.notify();
      },
    },
    bluetoothPillLabel,
  );

  const stressExtraContainer = h("span", { style: "display:contents" });

  const phase3Bar = h(
    "div",
    { class: "phase3-bar" },
    taskPillEl,
    weatherPill,
    bluetoothPill,
    dailyGoalsPill,
    morningRecapBtn,
    healthModePill,
    statsPill,
    focusStartBtn,
    focusActivePill,
    agentWaitPill,
    browserPromptPill,
    browserSessionPill,
    backToEditorBtn,
    stressExtraContainer,
  );

  let lastStressKey = "";

  function syncPhase3Bar() {
    const isStressMany =
      State.layoutStressMode === "on" ||
      State.layoutStressMode === "recap" ||
      State.layoutStressMode === "agent";

    const rawTaskTitle = State.currentTaskRecord?.title || "Deep work session";
    const taskTitle = cleanTaskLabel(rawTaskTitle);
    taskLabelEl.textContent = taskTitle;
    taskLabelEl.title = `${taskTitle} — Click to rename current task`;
    taskPillEl.title = `Current task: ${taskTitle}`;
    taskLabelEl.style.display = editingTask ? "none" : "";
    taskInputEl.style.display = editingTask ? "" : "none";

    const goals = State.dailyGoals;
    const goalsTotal = goals.length;
    const goalsDone = goals.filter((g) => g.done).length;
    dailyGoalsLabel.textContent = `🎯 ${goalsDone}/${goalsTotal}`;
    dailyGoalsPill.classList.toggle("active", State.showingDailyGoalsCard);
    dailyGoalsPill.classList.toggle("all-done", goalsDone === goalsTotal && goalsTotal > 0);
    dailyGoalsPill.title = goals.length > 0
      ? `Today's To-Do (${goalsDone}/${goalsTotal}):\n` + goals.map((g) => `${g.done ? "✅" : "⬜"} ${g.slot}. ${g.title}`).join("\n")
      : "Click to open today's To-Do list & add tasks";
    morningRecapBtn.classList.toggle("active", State.showingMorningRecap);

    const hs = State.healthStatus;
    healthModePill.classList.toggle("active", State.showingHealthCard);
    healthModePill.classList.toggle("night", Boolean(hs?.isNightMode));
    healthModePill.classList.toggle("idle", Boolean(hs?.isIdle));
    if (hs?.isIdle) {
      healthModeLabel.textContent = "💤 Idle";
      healthModePill.title = `Wayland Idle (${hs.idleSource}) — All timers paused`;
    } else if (hs?.isNightMode) {
      healthModeLabel.textContent = "🌙 Night";
      healthModePill.title = `Night Wind-Down active (start ${hs.config.nightStartHour}:00) — Click for Health & Night controls`;
    } else {
      healthModeLabel.textContent = "🌿";
      healthModePill.title = hs
        ? `Health timers active · Next: 👀 ${Math.ceil(hs.eyeRemainingSecs / 60)}m · 💧 ${Math.ceil(hs.waterRemainingSecs / 60)}m · 🧘 ${Math.ceil(hs.postureRemainingSecs / 60)}m`
        : "Night & Health Mode";
    }

    const sd = State.statsDashboard;
    statsPill.classList.toggle("active", State.showingStatsCard);
    if (sd) {
      const streakStr = sd.streakDays > 0 ? ` 🔥${sd.streakDays}d` : "";
      statsPillLabel.textContent = `🏆 ${sd.dailyScore} · L${sd.level}${streakStr}`;
      statsPill.title = `Daily Score: ${sd.dailyScore}/100 · ${sd.levelTitle} (${sd.totalXp} XP) · Streak: ${sd.streakDays}d\n${sd.formulaDoc}`;
    } else {
      statsPillLabel.textContent = "🏆 Stats";
    }

    const wx = State.weatherStatus;
    weatherPill.classList.toggle("active", State.showingWeatherCard);
    if (wx) {
      weatherPillLabel.textContent = `⛅ ${wx.tempC}° · ${wx.condition}`;
      weatherPill.title = `${wx.location}: ${wx.tempC}°C (${wx.condition}) · Humidity ${wx.humidity}% · Wind ${wx.windMs} m/s — Click for 4-Day Forecast`;
    } else {
      weatherPillLabel.textContent = "⛅ 34° · Clouds";
    }

    const bt = State.bluetoothStatus;
    if (bt?.connected) {
      bluetoothPill.style.display = "";
      bluetoothPillLabel.textContent = `🎧 ${bt.primaryName} · ${bt.primaryBattery}%`;
      bluetoothPill.title = `Connected: ${bt.primaryName} (${bt.primaryBattery}%) — Click to show Bluetooth Dynamic Bar`;
    } else {
      bluetoothPill.style.display = "none";
    }

    const session = State.focusSession;
    if (!session) {
      focusStartBtn.style.display = "";
      focusActivePill.style.display = "none";
      focusStartBtn.classList.toggle("active", State.showingFocusTimerCard);
    } else {
      focusStartBtn.style.display = "none";
      focusActivePill.style.display = "";
      focusTimeSpan.textContent = `${session.paused ? "⏸ " : ""}${formatMmSs(session.remainingSecs)}`;
      focusActivePill.classList.toggle("paused", session.paused);
      focusActivePill.classList.toggle("active", State.showingFocusTimerCard);
      focusPauseBtn.textContent = session.paused ? "▶" : "⏸";
    }

    const ag = State.activeAgentWait;
    const weekly = State.weeklyAgentWaitStats;
    const weeklySummary = `Weekly agent wait: ${formatDurationCompact(weekly.totalWaitSecs)} (${weekly.totalSessions} runs)`;
    if (isStressMany && (!ag || ag.state === "idle")) {
      agentWaitPill.style.display = "";
      agentWaitPill.classList.remove("working");
      agentWaitPill.classList.add("done");
      agentWaitLabel.textContent = "🤖 Antigravity done, review it ↗";
      agentWaitPill.title = "Stress-test badge: Antigravity done, review it";
    } else if (!ag || ag.state === "idle") {
      agentWaitPill.style.display = "none";
    } else if (ag.state === "working") {
      agentWaitPill.style.display = "";
      agentWaitPill.classList.remove("done");
      agentWaitPill.classList.add("working");
      agentWaitLabel.textContent = `🤖 ${ag.displayName} working · ${formatMmSs(ag.elapsedSecs)}`;
      agentWaitPill.title = `${ag.detail || "Agent working"} — Click to focus ${ag.displayName} (${weeklySummary})`;
    } else if (ag.state === "done") {
      agentWaitPill.style.display = "";
      agentWaitPill.classList.remove("working");
      agentWaitPill.classList.add("done");
      agentWaitLabel.textContent = `🤖 ${ag.displayName} done, review it ↗`;
      agentWaitPill.title = `Waited ${formatDurationCompact(ag.lastWaitSecs)} · Click to focus ${ag.displayName} (${weeklySummary})`;
    }

    const bSession = State.browserSession;
    const isBrowserFocused =
      State.focusTask?.name === "Browser" ||
      (State.focusTask?.windowClass || "").toLowerCase().includes("brave");

    if (isStressMany) {
      browserPromptPill.style.display = "";
      browserSessionPill.style.display = "";
      browserSessionTimeSpan.textContent = "🔍 18:42";
      backToEditorBtn.style.display = "";
      backToEditorBtn.textContent = "↩ Antigravity";
    } else {
      if (bSession) {
        browserPromptPill.style.display = "none";
        browserSessionPill.style.display = "";
        const icon = bSession.intent === "research" ? "🔍" : "☕";
        browserSessionTimeSpan.textContent = `${icon} ${formatMmSs(bSession.remainingSecs)}`;
        browserSessionPill.classList.toggle("break-mode", bSession.intent === "break");
      } else if (State.browserPromptOpen || isBrowserFocused) {
        browserPromptPill.style.display = "";
        browserSessionPill.style.display = "none";
      } else {
        browserPromptPill.style.display = "none";
        browserSessionPill.style.display = "none";
      }

      const showBackEditor = State.browserNudgeToEditor || isBrowserFocused;
      backToEditorBtn.style.display = showBackEditor ? "" : "none";
      const editorName = State.lastEditorWindow?.name || "Editor";
      backToEditorBtn.textContent = `↩ ${editorName}`;
      backToEditorBtn.classList.toggle("nudge", State.browserNudgeToEditor);
    }

    const stressKey = State.stressExtraBadges.join("|");
    if (stressKey !== lastStressKey) {
      lastStressKey = stressKey;
      stressExtraContainer.replaceChildren(
        ...State.stressExtraBadges.map((label) =>
          h(
            "button",
            {
              class: "p3-pill p3-stress-extra",
              title: label,
              onclick: (e: Event) => e.stopPropagation(),
            },
            h("span", { text: label }),
          ),
        ),
      );
    }
  }

  function go(v: IslandViewName) {
    actions.blip();
    actions.setView(v);
  }

  const el = h(
    "div",
    { id: "header" },
    h("div", { class: "tabs" }, tabHome, tabChat, tabDrop),
    phase3Bar,
    h("div", { class: "header-actions" }, planPills, gearBtn, soundBtn, collapseBtn),
  );
  const headerActions = el.lastElementChild as HTMLElement;

  return {
    el,
    sync() {
      const v = State.view;
      tabHome.classList.toggle("on", v === "overview" || v === "empty");
      tabChat.classList.toggle("on", v === "prompt");
      tabDrop.classList.toggle("on", v === "upload");
      gearBtn.classList.toggle("on", v === "settings");
      clear(gearBtn);
      gearBtn.append(svg(v === "settings" ? ICONS.gearFill : ICONS.gear, 14));
      clear(soundBtn);
      soundBtn.append(svg(State.settings.soundEnabled ? ICONS.speakerOn : ICONS.speakerOff, 14));
      syncPhase3Bar();
      syncPlanPills();
      el.style.opacity = v === "confused" ? "0" : "1";
    },
  };

  function syncPlanPills() {
    const claudeOn = claudePillVisible();
    const codexOn = codexPillVisible();
    claudePill.el.style.display = claudeOn ? "" : "none";
    codexPill.el.style.display = codexOn ? "" : "none";
    planPills.classList.toggle("on", claudeOn || codexOn);
    // Both pills: the right side tightens so it still clears the screen edge.
    headerActions.classList.toggle("both-plans", claudeOn && codexOn);
    if (claudeOn) claudePill.sync();
    if (codexOn) codexPill.sync();
    // Codex is asked when its pill comes into view (stale answers only).
    const shown = codexOn && State.mode === "expanded";
    if (shown && !codexShown) refreshCodexPlanUsage();
    codexShown = shown;
  }
}

// ── Overview ──────────────────────────────────────────────────────────────────

function isBrowserTask(task: AgentTask | null): boolean {
  if (!task) return false;
  const cls = (task.windowClass || "").toLowerCase();
  return (
    task.name === "Browser" ||
    cls.includes("brave") ||
    cls.includes("chrome") ||
    cls.includes("chromium") ||
    cls.includes("firefox") ||
    cls.includes("edge")
  );
}

function shouldShowBrowserMediaCard(task: AgentTask | null): boolean {
  if (!isBrowserTask(task)) return false;
  if (State.mediaStatus?.active) return true;
  const cap = (task?.windowCaption || "").toLowerCase();
  return cap.includes("youtube") || cap.includes("spotify") || cap.includes("soundcloud");
}

function buildOverview(actions: ViewActions): ViewHost {
  /** The diff open in the left card (a FileDiff id), as activeDiffId on macOS. */
  let activeDiffId: number | null = null;
  const closeDiff = () => {
    if (activeDiffId == null) return;
    activeDiffId = null;
    State.notify();
  };
  const ticker = new Ticker((diffId) => {
    actions.blip();
    activeDiffId = diffId;
    State.notify();
  });
  const who = h("div", { class: "who" });
  const tickerBody = h("div", { class: "card-body" }, who, ticker.el);
  const leftBody = h("div", { class: "left-body" });
  const jump = h(
    "button",
    { class: "icon-btn jump", title: tl("Open"), onclick: () => actions.openTarget() },
    svg(ICONS.arrowUpRight, 8),
  );
  const left = card(null, leftBody, jump);
  const pills = h("div", { class: "pills" });
  const right = card(null, pills);
  // Opened from a plan pill in the header: stands in for the left card.
  const plan = new PlanCard();
  let planTimer: number | null = null;
  const focusTimerHost = buildFocusTimerHost();
  const mediaPlayerHost = buildMediaPlayerHost();

  const el = h("div", { class: "view overview" },
    h("div", { class: "left" }, left),
    h("div", { class: "right" }, right),
  );

  let pillIds = "";
  let detailOpen = false;
  let lastFocus: string | null = null;
  let mode:
    | "ticker"
    | "card"
    | "plan"
    | "diff"
    | "workflow"
    | "recap"
    | "goals"
    | "health"
    | "stats"
    | "focustimer"
    | "weather"
    | "media"
    | null = null;
  let cardKey = "";

  // Leaving the overview or folding the island closes the diff, as on macOS.
  State.subscribe(() => {
    if (activeDiffId != null && (State.view !== "overview" || State.mode !== "expanded")) {
      activeDiffId = null;
    }
  });
  // Escape steps back out of the diff before it closes the island.
  window.addEventListener(
    "keydown",
    (e) => {
      if (e.key !== "Escape" || activeDiffId == null || State.view !== "overview") return;
      e.stopImmediatePropagation();
      closeDiff();
    },
    true,
  );

  const hooks: IntegrationCardHooks = {
    get detailOpen() {
      return detailOpen;
    },
    openDetail() {
      detailOpen = true;
      cardKey = "";
      State.notify();
    },
    closeDetail() {
      detailOpen = false;
      cardKey = "";
      State.notify();
    },
    openSettings: () => actions.openSettingsWindow(),
  };

  /** The countdowns move every 30 s while a card is open, and only then. */
  function syncPlanTimer(open: boolean) {
    const stop = () => {
      if (planTimer != null) window.clearInterval(planTimer);
      planTimer = null;
    };
    if (!open) return stop();
    if (planTimer != null) return;
    planTimer = window.setInterval(() => {
      if (planCardOpen() && State.mode === "expanded") State.notify();
      else stop();
    }, 30_000);
  }

  return {
    el,
    tick(nowMs: number) {
      if (mode !== "ticker") return false;
      ticker.tick(nowMs);
      return ticker.animating;
    },
    sync() {
      const task = State.focusTask;
      if (task?.id !== lastFocus) {
        lastFocus = task?.id ?? null;
        detailOpen = false;
        activeDiffId = null;
        cardKey = "";
        mode = null;
      }

      // A workspace or agent pill with a live session keeps the ticker; every
      // other pill shows its own card, exactly like IntegrationCardView.
      const sessionActive = task != null && hasSessionTicker(task);

      const planOpen = planCardOpen();
      if (mode === "plan" && !planOpen) {
        mode = null;
        cardKey = "";
      }
      syncPlanTimer(planOpen);

      // A diff that has since been dropped (cap, expiry, session end) just closes.
      const diff = task && activeDiffId != null ? State.findDiff(task.id, activeDiffId) : null;
      if (!diff) activeDiffId = null;

      const note = State.workflowNotification;
      if (mode === "workflow" && !note) {
        mode = null;
        cardKey = "";
      }
      if (mode === "recap" && !State.showingMorningRecap) {
        mode = null;
        cardKey = "";
      }
      if (mode === "goals" && !State.showingDailyGoalsCard) {
        mode = null;
        cardKey = "";
      }
      if (mode === "health" && !State.showingHealthCard) {
        mode = null;
        cardKey = "";
      }
      if (mode === "stats" && !State.showingStatsCard) {
        mode = null;
        cardKey = "";
      }
      if (mode === "focustimer" && !State.showingFocusTimerCard) {
        mode = null;
        cardKey = "";
      }
      if (mode === "weather" && !State.showingWeatherCard) {
        mode = null;
        cardKey = "";
      }
      if (mode === "media" && !shouldShowBrowserMediaCard(task)) {
        mode = null;
        cardKey = "";
      }

      if (planOpen) {
        if (mode !== "plan") {
          clear(leftBody);
          leftBody.append(plan.el);
          mode = "plan";
        }
        plan.sync();
      } else if (State.showingWeatherCard) {
        const wx = State.weatherStatus;
        const selIdx = State.selectedWeatherDayIdx;
        const key = `weather~${wx?.location ?? ""}~${wx?.tempC ?? 34}~${wx?.condition ?? ""}~${wx?.humidity ?? 59}~${selIdx}`;
        if (key !== cardKey) {
          const animateEntrance = mode !== "weather";
          cardKey = key;
          mode = "weather";
          clear(leftBody);
          leftBody.append(renderWeatherCard(animateEntrance));
        }
      } else if (State.showingFocusTimerCard) {
        if (mode !== "focustimer") {
          clear(leftBody);
          leftBody.append(focusTimerHost.el);
          mode = "focustimer";
          cardKey = "focustimer";
        }
        focusTimerHost.sync();
      } else if (State.showingMorningRecap && State.morningRecap) {
        const r = State.morningRecap;
        const goalsKey = State.dailyGoals.map((g) => `${g.id}:${g.done}:${g.title}`).join("|");
        const key = `recap~${r.lastProject}~${r.lastFile}~${r.gitBranch}~${r.unfinishedTask}~${goalsKey}`;
        if (key !== cardKey) {
          cardKey = key;
          mode = "recap";
          clear(leftBody);
          leftBody.append(renderMorningRecapCard(r));
        }
      } else if (State.showingDailyGoalsCard) {
        const goalsKey = State.dailyGoals.map((g) => `${g.id}:${g.done}:${g.title}`).join("|");
        const key = `goals~${goalsKey}`;
        if (key !== cardKey) {
          cardKey = key;
          mode = "goals";
          clear(leftBody);
          leftBody.append(renderDailyGoalsCard());
        }
      } else if (State.showingHealthCard) {
        const hs = State.healthStatus;
        const eyeM = Math.ceil((hs?.eyeRemainingSecs ?? 0) / 60);
        const waterM = Math.ceil((hs?.waterRemainingSecs ?? 0) / 60);
        const postureM = Math.ceil((hs?.postureRemainingSecs ?? 0) / 60);
        const key = `health~${hs?.isNightMode ?? false}~${hs?.isIdle ?? false}~${eyeM}~${waterM}~${postureM}~${hs?.activeReminder?.kind ?? ""}`;
        if (key !== cardKey) {
          cardKey = key;
          mode = "health";
          clear(leftBody);
          leftBody.append(renderHealthModeCard());
        }
      } else if (State.showingStatsCard && State.statsDashboard) {
        const sd = State.statsDashboard;
        const key = `stats~${sd.dailyScore}~${sd.totalXp}~${sd.level}~${sd.streakDays}~${sd.weeklyInsight}~${sd.dailyChart.map((p) => p.score).join(",")}`;
        if (key !== cardKey) {
          cardKey = key;
          mode = "stats";
          clear(leftBody);
          leftBody.append(renderStatsDashboardCard(sd));
        }
      } else if (task && diff) {
        const key = `diff~${task.id}~${diff.id}`;
        if (key !== cardKey) {
          cardKey = key;
          mode = "diff";
          clear(leftBody);
          leftBody.append(buildDiffCard(diff, {
            dismiss: () => {
              actions.blip();
              closeDiff();
            },
            open: (path) => void Bridge.openFileInVSCode(path),
          }));
        }
      } else if (note) {
        const key = `workflow~${task?.id ?? "none"}~${note.message}~${note.detail ?? ""}`;
        if (key !== cardKey) {
          cardKey = key;
          mode = "workflow";
          clear(leftBody);
          leftBody.append(renderWorkflowQuoteCard(task, note.message, note.detail));
        }
      } else if (task && sessionActive) {
        if (mode !== "ticker") {
          clear(leftBody);
          leftBody.append(tickerBody);
          mode = "ticker";
          cardKey = "";
        }
        clear(who);
        // The agent's name is already the pill's: the label says what kind of
        // pill it is, as on the Mac (PillDefinition.sessionSubtitle).
        who.append(
          dot(task.color, 7),
          h("span", { class: "name", text: task.name }),
          h("span", { class: "tool", text: t(sessionSubtitle(task.id)) }),
        );
        if (task.steps.length > 1) {
          who.append(h("span", {
            class: "count",
            text: `${Math.min(task.stepIndex + 1, task.steps.length)}/${task.steps.length}`,
          }));
        }
        ticker.sync(task);
      } else if (task && shouldShowBrowserMediaCard(task)) {
        if (mode !== "media") {
          clear(leftBody);
          leftBody.append(mediaPlayerHost.el);
          mode = "media";
          cardKey = "media";
        }
        mediaPlayerHost.sync(task);
      } else if (task) {
        const isDesktopWindow = task.windowCaption !== undefined || State.desktopWindows.some((w) => {
          const id = w.id || `window-${w.pid}`;
          return id === task.id;
        });
        if (isDesktopWindow) {
          const quoteText = State.lastWorkflowQuote ?? "";
          const detailText = State.lastWorkflowDetail ?? "";
          const domainKey = `${State.browserDomain?.domain ?? ""}:${State.browserDomain?.category ?? ""}`;
          // Exclude per-second detailText from cardKey so the card never re-mounts or blinks every second.
          const key = `window~${task.id}~${task.windowActive ?? false}~${task.windowCaption ?? ""}~${quoteText}~${domainKey}`;
          if (key !== cardKey || mode !== "card") {
            cardKey = key;
            mode = "card";
            clear(leftBody);
            leftBody.append(renderDesktopWindowCard(task, quoteText, detailText));
          } else {
            const statusBox = leftBody.querySelector<HTMLElement>(".wf-structured-status");
            if (statusBox) {
              const nextCap = detailText || task.windowCaption || task.name;
              const isActive = task.windowActive ?? (task.state !== "idle");
              const statusColor = isActive ? "#22C55E" : "#6B7079";
              if (statusBox.dataset.rawCaption !== nextCap) {
                populateStructuredWindowStatus(statusBox, statusColor, nextCap, task.name);
              }
            }
          }
        } else {
          const info = State.integrations[task.id];
          const quoteText = State.lastWorkflowQuote ?? "";
          const key = [
            language(), task.id, detailOpen, task.state, task.steps.join("|"),
            info?.loaded, info?.error, info?.configured,
            JSON.stringify(info?.data ?? {}), quoteText,
          ].join("~");
          if (key !== cardKey) {
            cardKey = key;
            mode = "card";
            clear(leftBody);
            leftBody.append(renderIntegrationCard(task, hooks));
          }
        }
      }

      jump.style.display =
        detailOpen ||
        mode === "plan" ||
        mode === "diff" ||
        mode === "workflow" ||
        mode === "recap" ||
        mode === "goals" ||
        mode === "health" ||
        mode === "stats" ||
        mode === "focustimer"
          ? "none"
          : "";

      const visible = State.otherTasks;
      const pillKey = visible.map((t) => `${t.id}:${t.color}:${t.pillBadge ?? ""}:${t.state}`).join("|");
      
      if (pillKey !== pillIds) {
        pillIds = pillKey;
        clear(pills);
        for (const t of visible) pills.append(buildPill(t, actions));
        pruneMiniBots();
      }
    },
  };
}

/**
 * IntegrationCardView.agentSessionActive: a workspace tool or an agent — or
 * any other tagged agent — with something going on.
 */
export function hasSessionTicker(task: AgentTask): boolean {
  // Desktop window tasks are rendered via renderDesktopWindowCard, never the empty ticker.
  if (task.windowCaption !== undefined) return false;
  // A pill made for an agent's session (Gemini CLI, Codex… not declared) only
  // exists while that session does: it keeps the ticker from the first event.
  if (task.source === "agent" && !task.isIntegration) return task.steps.length > 0 || task.state !== "idle";
  const category = pillDefinition(task.id)?.category;
  const isSession = category === "workspace" || category === "agent" ||
    (category == null && task.id.startsWith("agent_"));
  return isSession && (task.state !== "idle" || task.steps.length > 0);
}

// ── Pomodoro Focus Timer Control Card (In-Place Synced, Zero Blinking) ────────

function buildFocusTimerHost(): { el: HTMLElement; sync: () => void } {
  const headDot = dot("#22c55e", 7);
  const headTitle = h("b", { class: "ft-title", text: "⏱ Timer · 25 min" });

  const presets = [5, 15, 25, 45];
  const presetBtns = presets.map((mins) =>
    h(
      "button",
      {
        class: "ft-preset-btn",
        title: `Start ${mins}-minute Pomodoro timer`,
        onclick: (e: Event) => {
          e.stopPropagation();
          void Workflow.startFocusSession(mins * 60);
        },
      },
      h("span", { text: `${mins}m` }),
    ),
  );

  const closeBtn = h(
    "button",
    {
      class: "p3-btn p6-close-btn",
      title: "Close timer card (timer keeps running in header)",
      onclick: (e: Event) => {
        e.stopPropagation();
        Workflow.toggleFocusTimerCard(false);
      },
    },
    h("span", { text: "✕" }),
  );

  const bigTimeEl = h("div", { class: "ft-big-time", text: "25:00" });
  const endsAtEl = h("div", { class: "ft-ends-at", text: "Ends at --:--" });

  // SVG circular progress ring (r = 21, circumference = 2 * pi * 21 ≈ 131.95)
  const SVG_NS = "http://www.w3.org/2000/svg";
  const circumference = 2 * Math.PI * 21;
  const ringSvg = document.createElementNS(SVG_NS, "svg");
  ringSvg.setAttribute("viewBox", "0 0 50 50");
  ringSvg.setAttribute("class", "ft-ring-svg");

  const ringTrack = document.createElementNS(SVG_NS, "circle");
  ringTrack.setAttribute("cx", "25");
  ringTrack.setAttribute("cy", "25");
  ringTrack.setAttribute("r", "21");
  ringTrack.setAttribute("class", "ft-ring-track");

  const ringProgress = document.createElementNS(SVG_NS, "circle");
  ringProgress.setAttribute("cx", "25");
  ringProgress.setAttribute("cy", "25");
  ringProgress.setAttribute("r", "21");
  ringProgress.setAttribute("class", "ft-ring-progress");
  ringProgress.style.strokeDasharray = `${circumference}`;
  ringProgress.style.strokeDashoffset = "0";

  ringSvg.append(ringTrack, ringProgress);
  const ringLabel = h("span", { class: "ft-ring-pct", text: "100%" });
  const ringWrap = h("div", { class: "ft-ring-wrap" }, ringSvg, ringLabel);

  const pauseResumeLabel = h("span", { text: "Pause" });
  const pauseResumeBtn = h(
    "button",
    {
      class: "ft-btn ft-btn-pause",
      onclick: (e: Event) => {
        e.stopPropagation();
        if (!State.focusSession) {
          void Workflow.startFocusSession(1500);
        } else {
          Workflow.togglePauseFocusSession();
        }
      },
    },
    pauseResumeLabel,
  );

  const plus1Btn = h(
    "button",
    {
      class: "ft-btn",
      title: "Add 1 minute",
      onclick: (e: Event) => {
        e.stopPropagation();
        if (!State.focusSession) {
          void Workflow.startFocusSession(60);
        } else {
          Workflow.addFocusSessionTime(60);
        }
      },
    },
    h("span", { text: "+1 min" }),
  );

  const plus5Btn = h(
    "button",
    {
      class: "ft-btn",
      title: "Add 5 minutes",
      onclick: (e: Event) => {
        e.stopPropagation();
        if (!State.focusSession) {
          void Workflow.startFocusSession(300);
        } else {
          Workflow.addFocusSessionTime(300);
        }
      },
    },
    h("span", { text: "+5 min" }),
  );

  const stopBtn = h(
    "button",
    {
      class: "ft-btn ft-btn-stop",
      title: "Stop Pomodoro timer",
      onclick: (e: Event) => {
        e.stopPropagation();
        void Workflow.cancelFocusSession();
      },
    },
    h("span", { text: "Stop" }),
  );

  const el = h(
    "div",
    { class: "int-card wf-card ft-card" },
    h(
      "div",
      { class: "int-head ft-head" },
      headDot,
      headTitle,
      h("div", { class: "ft-presets" }, ...presetBtns),
      closeBtn,
    ),
    h(
      "div",
      { class: "ft-body-row" },
      h("div", { class: "ft-main-col" }, bigTimeEl, endsAtEl),
      ringWrap,
    ),
    h("div", { class: "ft-actions-row" }, pauseResumeBtn, plus1Btn, plus5Btn, stopBtn),
  );

  return {
    el,
    sync() {
      const s = State.focusSession;
      if (!s) {
        headDot.style.background = "#6b7079";
        headTitle.textContent = "⏱ Timer · Ready";
        bigTimeEl.textContent = "25:00";
        endsAtEl.textContent = "Pick a preset or click Start";
        ringProgress.style.strokeDashoffset = "0";
        ringProgress.style.stroke = "#22c55e";
        ringLabel.textContent = "25m";
        pauseResumeLabel.textContent = "Start 25m";
        pauseResumeBtn.classList.remove("paused");
        for (let i = 0; i < presets.length; i++) {
          presetBtns[i].classList.toggle("on", presets[i] === 25);
        }
        return;
      }

      const targetMins = Math.max(1, Math.round(s.targetSecs / 60));
      headDot.style.background = s.paused ? "#f59e0b" : "#22c55e";
      headTitle.textContent = `⏱ Timer · ${targetMins} min`;
      bigTimeEl.textContent = formatMmSs(s.remainingSecs);

      if (s.paused) {
        endsAtEl.textContent = `Paused · ${s.taskTitle}`;
      } else {
        const endDate = new Date(Date.now() + s.remainingSecs * 1000);
        const endStr = endDate.toLocaleTimeString("en-US", {
          hour: "numeric",
          minute: "2-digit",
          hour12: true,
        });
        endsAtEl.textContent = `Ends at ${endStr}`;
      }

      const ratio = s.targetSecs > 0 ? Math.max(0, Math.min(1, s.remainingSecs / s.targetSecs)) : 0;
      const offset = circumference * (1 - ratio);
      ringProgress.style.strokeDashoffset = `${offset.toFixed(2)}`;
      ringProgress.style.stroke = s.paused ? "#f59e0b" : "#22c55e";
      ringLabel.textContent = `${Math.round(ratio * 100)}%`;

      pauseResumeLabel.textContent = s.paused ? "Resume" : "Pause";
      pauseResumeBtn.classList.toggle("paused", s.paused);

      for (let i = 0; i < presets.length; i++) {
        presetBtns[i].classList.toggle("on", presets[i] === targetMins);
      }
    },
  };
}

// ── Browser YouTube / Media Now-Playing Card (In-Place Synced) ────────────────

function cleanMediaTitle(rawTitle: string): { title: string; artist: string } {
  let cleaned = rawTitle
    .replace(/\s*[-–—]\s*(Brave|Google Chrome|Chromium|Mozilla Firefox|Microsoft Edge)\s*$/i, "")
    .replace(/^\(\d+\)\s*/, "")
    .trim();
  if (/[-–—]\s*YouTube$/i.test(cleaned)) {
    cleaned = cleaned.replace(/\s*[-–—]\s*YouTube$/i, "").trim();
    return { title: cleaned || "YouTube Video", artist: "YouTube" };
  }
  return { title: cleaned || "Browser Media", artist: "Browser" };
}

function buildMediaPlayerHost(): { el: HTMLElement; sync: (task: AgentTask) => void } {
  const titleEl = h("div", { class: "mp-title", text: "YouTube Video" });
  const artistEl = h("span", { class: "mp-artist", text: "YouTube" });
  const liveDot = dot("#22c55e", 6);
  const liveText = h("span", { class: "mp-live-lbl", text: "Now playing" });

  const subRow = h(
    "div",
    { class: "mp-sub-row" },
    artistEl,
    h("span", { class: "mp-sep", text: "·" }),
    liveDot,
    liveText,
  );

  const barFill = h("div", { class: "mp-bar-fill", style: "width: 0%" });
  const barTrack = h(
    "div",
    {
      class: "mp-bar-track",
      title: "Click to seek in video/audio",
      onclick: (e: Event) => {
        e.stopPropagation();
        const dur = State.mediaStatus?.durationSecs ?? 0;
        if (dur <= 0) return;
        const me = e as MouseEvent;
        const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
        const ratio = Math.max(0, Math.min(1, (me.clientX - rect.left) / Math.max(1, rect.width)));
        void Workflow.mediaAction("seek-to", ratio * dur);
      },
    },
    barFill,
  );

  const posTimeEl = h("span", { class: "mp-time", text: "0:00" });
  const durTimeEl = h("span", { class: "mp-time", text: "0:00" });

  const prevBtn = h(
    "button",
    {
      class: "mp-ctrl-btn",
      title: "Previous track / video",
      onclick: (e: Event) => {
        e.stopPropagation();
        void Workflow.mediaAction("previous");
      },
    },
    svg(ICONS.mediaPrev, 11),
  );

  const back10Btn = h(
    "button",
    {
      class: "mp-ctrl-btn mp-seek-btn",
      title: "Rewind 10 seconds (-10s)",
      onclick: (e: Event) => {
        e.stopPropagation();
        void Workflow.mediaAction("seek-relative", -10);
      },
    },
    h("span", { text: "-10s" }),
  );

  const playPauseBtn = h(
    "button",
    {
      class: "mp-play-btn",
      title: "Play / Pause",
      onclick: (e: Event) => {
        e.stopPropagation();
        void Workflow.mediaAction("play-pause");
      },
    },
    svg(ICONS.mediaPause, 12),
  );

  const fwd10Btn = h(
    "button",
    {
      class: "mp-ctrl-btn mp-seek-btn",
      title: "Forward 10 seconds (+10s)",
      onclick: (e: Event) => {
        e.stopPropagation();
        void Workflow.mediaAction("seek-relative", 10);
      },
    },
    h("span", { text: "+10s" }),
  );

  const nextBtn = h(
    "button",
    {
      class: "mp-ctrl-btn",
      title: "Next track / video",
      onclick: (e: Event) => {
        e.stopPropagation();
        void Workflow.mediaAction("next");
      },
    },
    svg(ICONS.mediaNext, 11),
  );

  const muteBtn = h(
    "button",
    {
      class: "mp-ctrl-btn mp-mute-btn",
      title: "Toggle system audio mute",
      onclick: (e: Event) => {
        e.stopPropagation();
        void Workflow.mediaAction("mute");
      },
    },
    svg(ICONS.speakerOn, 12),
  );

  let currentTask: AgentTask | null = null;
  let lastPlaying: boolean | null = null;
  let lastMuted: boolean | null = null;

  const headRow = h(
    "div",
    {
      class: "mp-top-row",
      title: "Click to jump to Browser window",
      onclick: () => {
        if (!currentTask) return;
        void Bridge.focusTrackerActivateWindow(
          currentTask.id,
          currentTask.windowPid ?? 0,
          currentTask.windowClass ?? currentTask.name,
        );
      },
    },
    h("div", { class: "mp-meta-col" }, titleEl, subRow),
  );

  const el = h(
    "div",
    { class: "int-card wf-card mp-card" },
    headRow,
    h(
      "div",
      { class: "mp-progress-wrap" },
      barTrack,
      h("div", { class: "mp-time-row" }, posTimeEl, durTimeEl),
    ),
    h(
      "div",
      { class: "mp-controls-row" },
      prevBtn,
      back10Btn,
      playPauseBtn,
      fwd10Btn,
      nextBtn,
      muteBtn,
    ),
  );

  return {
    el,
    sync(task: AgentTask) {
      currentTask = task;
      const ms = State.mediaStatus;
      const fallback = cleanMediaTitle(task.windowCaption || task.name);
      const title = ms?.active && ms.title ? ms.title : fallback.title;
      const artist =
        ms?.active && ms.artist
          ? ms.artist
          : fallback.artist;
      const playing = ms?.active ? ms.playing : true;
      const pos = ms?.active ? Math.max(0, ms.positionSecs) : 0;
      const dur = ms?.active ? Math.max(0, ms.durationSecs) : 0;
      const muted = Boolean(ms?.muted);

      if (titleEl.textContent !== title) titleEl.textContent = title;
      titleEl.title = title;
      if (artistEl.textContent !== artist) artistEl.textContent = artist;

      liveDot.style.background = playing ? "#22c55e" : "#f59e0b";
      const statusText = playing ? "Now playing" : "Paused";
      if (liveText.textContent !== statusText) liveText.textContent = statusText;

      const pct = dur > 0 ? Math.max(0, Math.min(100, (pos / dur) * 100)) : playing ? 100 : 0;
      barFill.style.width = `${pct.toFixed(1)}%`;

      const posStr = formatMmSs(pos);
      const durStr = dur > 0 ? formatMmSs(dur) : "LIVE";
      if (posTimeEl.textContent !== posStr) posTimeEl.textContent = posStr;
      if (durTimeEl.textContent !== durStr) durTimeEl.textContent = durStr;

      if (playing !== lastPlaying) {
        lastPlaying = playing;
        clear(playPauseBtn);
        playPauseBtn.append(svg(playing ? ICONS.mediaPause : ICONS.mediaPlay, 12));
        playPauseBtn.classList.toggle("playing", playing);
      }

      if (muted !== lastMuted) {
        lastMuted = muted;
        clear(muteBtn);
        muteBtn.append(svg(muted ? ICONS.speakerOff : ICONS.speakerOn, 12));
        muteBtn.classList.toggle("muted", muted);
      }
    },
  };
}

function renderStatsDashboardCard(sd: import("../core/bridge").StatsDashboardData): HTMLElement {
  const xpPct = sd.nextLevelXp > 0
    ? Math.min(100, Math.round((sd.currentLevelXp / sd.nextLevelXp) * 100))
    : 100;

  const chartBars = sd.dailyChart.map((pt, idx) => {
    const isToday = idx === sd.dailyChart.length - 1;
    const heightPx = Math.max(4, Math.round((pt.score / 100) * 30));
    return h(
      "div",
      {
        class: `p8-bar-col ${isToday ? "today" : ""}`,
        title: `${pt.label} (${pt.day}): Score ${pt.score}/100 · ${pt.focusMins}m active work · ${pt.tasksDone} tasks done`,
      },
      h(
        "div",
        { class: "p8-bar-track" },
        h("div", { class: "p8-bar-fill", style: `height:${heightPx}px` }),
      ),
      h("span", { class: "p8-bar-lbl", text: pt.label }),
    );
  });

  const focusMins = sd.focusMinsToday ?? sd.dailyChart[sd.dailyChart.length - 1]?.focusMins ?? 0;
  const tasksDone = sd.tasksDoneToday ?? sd.dailyChart[sd.dailyChart.length - 1]?.tasksDone ?? 0;
  const goalsDone = sd.goalsDoneToday ?? State.dailyGoals.filter((g) => g.done).length;
  const goalsTotal = sd.goalsTotalToday ?? State.dailyGoals.length;
  const agentRuns = sd.agentRunsToday ?? 0;

  const breakdownItems = [
    {
      icon: "⚡",
      label: "Work",
      pts: `+${sd.scoreBreakdown.focusPoints}`,
      sub: `${focusMins}m`,
      cls: "focus",
    },
    {
      icon: "✅",
      label: "Tasks",
      pts: `+${sd.scoreBreakdown.taskPoints}`,
      sub: `${tasksDone}`,
      cls: "task",
    },
    {
      icon: "🎯",
      label: "To-Dos",
      pts: `+${sd.scoreBreakdown.goalPoints}`,
      sub: `${goalsDone}/${goalsTotal}`,
      cls: "goal",
    },
    {
      icon: "🌐",
      label: "Research",
      pts: `+${sd.scoreBreakdown.browserPoints}`,
      sub: "",
      cls: "browser",
    },
    {
      icon: "🤖",
      label: "Agents",
      pts: `+${sd.scoreBreakdown.agentPoints}`,
      sub: `${agentRuns}`,
      cls: "agent",
    },
  ];

  const breakdownChips = breakdownItems.map((item) =>
    h(
      "div",
      { class: `p8-stat-chip p8-chip-${item.cls}` },
      h("span", { class: "p8-chip-lbl", text: `${item.icon} ${item.label}` }),
      h("b", { class: "p8-chip-pts", text: item.pts }),
      item.sub ? h("span", { class: "p8-chip-sub", text: item.sub }) : null,
    ),
  );

  const rawTopTasks = sd.topTasks && sd.topTasks.length > 0
    ? sd.topTasks
    : [
        {
          title: cleanTaskLabel(State.currentTaskRecord?.title || "Deep work session"),
          app: State.focusTask?.name || "Workspace",
          mins: focusMins,
        },
      ];

  const taskPills = rawTopTasks.slice(0, 2).map((t) => {
    const cleanTitle = cleanTaskLabel(t.title);
    return h(
      "div",
      {
        class: "p8-task-pill",
        title: `${cleanTitle} (${t.app} · ${t.mins}m today)`,
      },
      h("span", { class: "p8-task-name", text: `📌 ${cleanTitle}` }),
      h("span", { class: "p8-task-meta", text: `${t.app} · ${t.mins}m` }),
    );
  });

  return h(
    "div",
    {
      class: "int-card wf-card p8-stats-card",
      title: sd.formulaDoc,
    },
    h(
      "div",
      { class: "int-head p6-card-head" },
      dot("#F59E0B", 7),
      h("b", { text: `🏆 Today ${sd.dailyScore}/100` }),
      h("span", { class: "p8-head-badge", text: `L${sd.level} · ${sd.totalXp} XP` }),
      h("span", { class: "p8-head-badge streak", text: `🔥 ${sd.streakDays}d` }),
      h("span", { class: "p8-head-badge reset", text: "🕛 00:00" }),
      h(
        "button",
        {
          class: "p3-btn p6-close-btn",
          title: "Close stats view",
          onclick: (e: Event) => {
            e.stopPropagation();
            Workflow.toggleStatsCard();
          },
        },
        h("span", { text: "✕" }),
      ),
    ),
    h(
      "div",
      { class: "p8-body-row" },
      h("div", { class: "p8-chart" }, ...chartBars),
      h(
        "div",
        { class: "p8-meta" },
        h("div", { class: "p8-breakdown-grid" }, ...breakdownChips),
        h(
          "div",
          { class: "p8-bottom-row" },
          h("div", { class: "p8-tasks-row" }, ...taskPills),
          h(
            "div",
            { class: "p8-xp-row", title: `${sd.levelTitle}` },
            h(
              "div",
              { class: "p8-xp-bar" },
              h("div", { class: "p8-xp-fill", style: `width:${xpPct}%` }),
            ),
            h("span", {
              class: "p8-xp-txt",
              text: `${sd.currentLevelXp}/${sd.nextLevelXp} XP`,
            }),
          ),
        ),
      ),
    ),
  );
}

function renderWeather3dSvg(kind: string, variant: "hero" | "forecast", uid: string): HTMLElement {
  const wrap = h("div", { class: `wx-svg-wrap wx-svg-${variant} wx-kind-${kind}` });
  const w = variant === "hero" ? 40 : 30;
  const ht = variant === "hero" ? 26 : 25;

  if (variant === "hero" || kind === "clouds") {
    wrap.innerHTML = `<svg class="wx-svg" viewBox="0 0 56 38" width="${w}" height="${ht}" aria-hidden="true">
      <defs>
        <linearGradient id="wxCloudMain_${uid}" x1="20%" y1="10%" x2="80%" y2="95%">
          <stop offset="0%" stop-color="#ffffff"/>
          <stop offset="55%" stop-color="#e8f3ff"/>
          <stop offset="100%" stop-color="#b9d8fa"/>
        </linearGradient>
        <linearGradient id="wxCloudBack_${uid}" x1="15%" y1="0%" x2="85%" y2="100%">
          <stop offset="0%" stop-color="#f0f7ff"/>
          <stop offset="100%" stop-color="#9ec5f0"/>
        </linearGradient>
      </defs>
      ${
         variant === "forecast"
          ? `<g class="wx-cloud-back">
              <path d="M22 25H46C49.9 25 53 21.9 53 18C53 14.4 50.3 11.4 46.8 11C45.4 6.4 41.1 3 36 3C30.2 3 25.4 7.3 24.6 12.9C20.9 13.4 18 16.6 18 20.5C18 23 19.8 25 22 25Z" fill="url(#wxCloudBack_${uid})" opacity="0.88"/>
            </g>`
          : ""
      }
      <g class="wx-cloud-front">
        <path d="M14 32H41C46.5 32 51 27.5 51 22C51 16.9 47.2 12.7 42.2 12.1C40.3 6.2 34.8 2 28.2 2C20.4 2 14 8 13.3 15.6C8.6 16.3 5 20.3 5 25.2C5 29 8.1 32 14 32Z" fill="url(#wxCloudMain_${uid})"/>
      </g>
    </svg>`;
    return wrap;
  }

  // "sun_rain" / "sun" / "storm" 3D icon (golden sun + glossy 3D cloud + animated blue raindrop)
  wrap.innerHTML = `<svg class="wx-svg" viewBox="0 0 48 44" width="${w}" height="${ht}" aria-hidden="true">
    <defs>
      <radialGradient id="wxSun_${uid}" cx="38%" cy="35%" r="65%">
        <stop offset="0%" stop-color="#fde047"/>
        <stop offset="55%" stop-color="#facc15"/>
        <stop offset="100%" stop-color="#eab308"/>
      </radialGradient>
      <linearGradient id="wxCloud_${uid}" x1="20%" y1="10%" x2="80%" y2="95%">
        <stop offset="0%" stop-color="#ffffff"/>
        <stop offset="60%" stop-color="#e6f2ff"/>
        <stop offset="100%" stop-color="#b5d6fa"/>
      </linearGradient>
      <linearGradient id="wxDrop_${uid}" x1="0%" y1="0%" x2="100%" y2="100%">
        <stop offset="0%" stop-color="#60a5fa"/>
        <stop offset="100%" stop-color="#2563eb"/>
      </linearGradient>
    </defs>
    <circle class="wx-sun-disc" cx="23" cy="14" r="10.5" fill="url(#wxSun_${uid})"/>
    <g class="wx-cloud-front">
      <path d="M12 31H35.5C39.9 31 43.5 27.4 43.5 23C43.5 18.9 40.4 15.6 36.4 15.1C34.9 10.4 30.5 7 25.2 7C19 7 13.9 11.8 13.3 17.8C9.5 18.4 6.5 21.6 6.5 25.5C6.5 28.6 9 31 12 31Z" fill="url(#wxCloud_${uid})"/>
    </g>
    <path class="wx-rain-drop" d="M24 32.2C24 32.2 20.3 36.8 20.3 39.1C20.3 41.1 21.9 42.7 24 42.7C26.1 42.7 27.7 41.1 27.7 39.1C27.7 36.8 24 32.2 24 32.2Z" fill="url(#wxDrop_${uid})"/>
  </svg>`;
  return wrap;
}

function renderWeatherCard(animateEntrance = true): HTMLElement {
  const wx = State.weatherStatus ?? {
    location: "Pune, India",
    dayFull: "Thursday",
    dateLabel: "15, Sep",
    tempC: 34,
    condition: "Clouds",
    iconKind: "clouds",
    uvIndex: 0,
    humidity: 59,
    windMs: 2.0,
    forecast: [
      { dayShort: "Thu", dayFull: "Thursday", dateLabel: "15, Sep", tempC: 32, condition: "Clouds", iconKind: "sun_rain", uvIndex: 0, humidity: 59, windMs: 2.0 },
      { dayShort: "Fri", dayFull: "Friday", dateLabel: "16, Sep", tempC: 27, condition: "Rain Showers", iconKind: "sun_rain", uvIndex: 2, humidity: 68, windMs: 2.8 },
      { dayShort: "Sat", dayFull: "Saturday", dateLabel: "17, Sep", tempC: 27, condition: "Partly Cloudy", iconKind: "sun_rain", uvIndex: 3, humidity: 64, windMs: 2.4 },
      { dayShort: "Sun", dayFull: "Sunday", dateLabel: "18, Sep", tempC: 29, condition: "Clouds", iconKind: "clouds", uvIndex: 1, humidity: 56, windMs: 1.9 },
    ],
  };

  const selIdx = Math.max(0, Math.min(wx.forecast.length - 1, State.selectedWeatherDayIdx));
  const activeDay = wx.forecast[selIdx];

  const heroDayFull = selIdx === 0 ? wx.dayFull : activeDay?.dayFull || wx.dayFull;
  const heroDateLabel = selIdx === 0 ? wx.dateLabel : activeDay?.dateLabel || wx.dateLabel;
  const heroTemp = selIdx === 0 ? wx.tempC : activeDay?.tempC ?? wx.tempC;
  const heroCond = selIdx === 0 ? wx.condition : activeDay?.condition || wx.condition;
  const heroUv = selIdx === 0 ? wx.uvIndex : activeDay?.uvIndex ?? wx.uvIndex;
  const heroHumidity = selIdx === 0 ? wx.humidity : activeDay?.humidity ?? wx.humidity;
  const heroWind = selIdx === 0 ? wx.windMs : activeDay?.windMs ?? wx.windMs;

  const forecastPills = wx.forecast.slice(0, 4).map((d, idx) =>
    h(
      "button",
      {
        class: `wx-day-pill${idx === selIdx ? " active" : ""}`,
        style: `animation-delay: ${110 + idx * 55}ms`,
        title: `${d.dayFull} (${d.dateLabel}): ${d.tempC}°C · ${d.condition}`,
        onclick: (e: Event) => {
          e.stopPropagation();
          Workflow.selectWeatherDay(idx);
        },
      },
      h("span", { class: "wx-day-name", text: d.dayShort }),
      renderWeather3dSvg(d.iconKind, "forecast", `d${idx}`),
      h("span", { class: "wx-day-temp", text: `${d.tempC}°` }),
    ),
  );

  return h(
    "div",
    { class: `int-card wf-card wx-card${animateEntrance ? " wx-enter" : ""}` },
    h(
      "button",
      {
        class: "p3-btn wx-close-btn",
        title: "Close Weather card",
        onclick: (e: Event) => {
          e.stopPropagation();
          Workflow.toggleWeatherCard();
        },
      },
      h("span", { text: "✕" }),
    ),
    h(
      "div",
      { class: "wx-card-grid" },
      h(
        "div",
        { class: "wx-left-col" },
        h(
          "div",
          { class: "wx-date-loc" },
          h("div", { class: "wx-day-full", text: heroDayFull }),
          h("div", { class: "wx-date-str", text: heroDateLabel }),
          h("div", { class: "wx-location", text: wx.location }),
        ),
        h(
          "div",
          { class: "wx-hero" },
          renderWeather3dSvg("clouds", "hero", "hero"),
          h("div", { class: "wx-hero-temp", text: `${heroTemp}°` }),
          h("div", { class: "wx-hero-cond", text: heroCond }),
        ),
      ),
      h(
        "div",
        { class: "wx-right-col" },
        h(
          "div",
          { class: "wx-metrics" },
          h(
            "div",
            { class: "wx-metric-row" },
            h("span", { class: "wx-metric-lbl", text: "UV INDEX" }),
            h("span", { class: "wx-metric-val", text: `${heroUv}` }),
          ),
          h(
            "div",
            { class: "wx-metric-row" },
            h("span", { class: "wx-metric-lbl", text: "HUMIDITY" }),
            h("span", { class: "wx-metric-val", text: `${heroHumidity}%` }),
          ),
          h(
            "div",
            { class: "wx-metric-row" },
            h("span", { class: "wx-metric-lbl", text: "WIND" }),
            h("span", { class: "wx-metric-val", text: `${heroWind} m/s` }),
          ),
        ),
        h("div", { class: "wx-forecast-row" }, ...forecastPills),
      ),
    ),
  );
}

function renderHealthModeCard(): HTMLElement {
  const hs = State.healthStatus;
  const eyeMins = hs ? Math.ceil(hs.eyeRemainingSecs / 60) : 20;
  const waterMins = hs ? Math.ceil(hs.waterRemainingSecs / 60) : 45;
  const postureMins = hs ? Math.ceil(hs.postureRemainingSecs / 60) : 60;
  const nightStart = hs?.config.nightStartHour ?? 23;
  const stateBadge = hs?.isIdle
    ? `💤 Idle (${hs.idleSource}) · Timers paused`
    : hs?.isNightMode
      ? `🌙 Night wind-down (${nightStart}:00+)`
      : `⚡ Active (${formatDurationCompact(hs?.activeStretchSecs ?? 0)})`;

  return h(
    "div",
    { class: "int-card wf-card p7-health-card" },
    h(
      "div",
      { class: "int-head p6-card-head" },
      dot(hs?.isIdle ? "#94A3B8" : hs?.isNightMode ? "#818CF8" : "#10B981", 7),
      h("b", { text: "🌿 Night & Health Mode" }),
      h("span", { text: stateBadge }),
      h(
        "button",
        {
          class: "p3-btn p6-close-btn",
          title: "Close Night & Health card",
          onclick: (e: Event) => {
            e.stopPropagation();
            Workflow.toggleHealthCard();
          },
        },
        h("span", { text: "✕" }),
      ),
    ),
    h(
      "div",
      { class: "p6-recap-line" },
      h("span", { class: "p6-chip", text: `👀 Eye: ${eyeMins}m` }),
      h("span", { class: "p6-chip", text: `💧 Water: ${waterMins}m` }),
      h("span", { class: "p6-chip", text: `🧘 Posture: ${postureMins}m` }),
      h("span", { class: "p6-chip", text: `🌙 Night: ${nightStart}:00` }),
    ),
    h(
      "div",
      { class: "p7-demo-row" },
      h("span", { class: "p7-demo-lbl", text: "Fast-forward:" }),
      h(
        "button",
        {
          class: "p3-btn p7-demo-btn",
          title: "Trigger 20-20-20 Eye reminder",
          onclick: (e: Event) => {
            e.stopPropagation();
            void Workflow.triggerHealthDemo("eye");
          },
        },
        h("span", { text: "👀 Eye" }),
      ),
      h(
        "button",
        {
          class: "p3-btn p7-demo-btn",
          title: "Trigger Hydration reminder",
          onclick: (e: Event) => {
            e.stopPropagation();
            void Workflow.triggerHealthDemo("water");
          },
        },
        h("span", { text: "💧 Water" }),
      ),
      h(
        "button",
        {
          class: "p3-btn p7-demo-btn",
          title: "Trigger Posture reminder",
          onclick: (e: Event) => {
            e.stopPropagation();
            void Workflow.triggerHealthDemo("posture");
          },
        },
        h("span", { text: "🧘 Posture" }),
      ),
      h(
        "button",
        {
          class: "p3-btn p7-demo-btn",
          title: "Toggle Night Wind-Down mode",
          onclick: (e: Event) => {
            e.stopPropagation();
            void Workflow.triggerHealthDemo(hs?.isNightMode ? "day" : "night");
          },
        },
        h("span", { text: hs?.isNightMode ? "☀️ Day" : "🌙 Night" }),
      ),
      h(
        "button",
        {
          class: "p3-btn p7-demo-btn",
          title: "Toggle Wayland Idle simulation (pauses all timers)",
          onclick: (e: Event) => {
            e.stopPropagation();
            void Workflow.triggerHealthDemo(hs?.isIdle ? "active" : "idle");
          },
        },
        h("span", { text: hs?.isIdle ? "▶ Resume" : "💤 Idle" }),
      ),
    ),
  );
}

function renderMorningRecapCard(recap: import("../core/bridge").MorningRecapData): HTMLElement {
  const projectName = recap.lastProject.split("/").filter(Boolean).pop() || recap.lastProject;
  const branchAndProj = `${recap.gitBranch} · ${projectName}`;
  return h(
    "div",
    { class: "int-card wf-card p6-recap-card" },
    h(
      "div",
      { class: "int-head p6-card-head" },
      dot("#F59E0B", 7),
      h("b", { text: "🌅 Where did I leave off" }),
      h("span", { text: branchAndProj, title: branchAndProj }),
      h(
        "button",
        {
          class: "p3-btn p6-close-btn",
          title: "Dismiss morning recap",
          onclick: (e: Event) => {
            e.stopPropagation();
            void Workflow.dismissMorningRecap();
          },
        },
        h("span", { text: "✓ Got it" }),
      ),
    ),
    h(
      "div",
      { class: "p6-recap-line" },
      h("span", { class: "p6-chip", text: `📄 ${recap.lastFile}`, title: recap.lastFile }),
      h("span", { class: "p6-chip", text: `🌿 ${recap.gitBranch}`, title: recap.gitBranch }),
      h("span", { class: "p6-chip p6-chip-task", text: `📌 ${recap.unfinishedTask}`, title: recap.unfinishedTask }),
    ),
    h(
      "div",
      { class: "p6-stats-line" },
      h(
        "span",
        {
          text: `Recent: ${recap.focusMinutesRecent}m focus (${recap.focusSessionsRecent} sessions) · ${recap.tasksCompletedRecent} tasks done · ${recap.agentRunsRecent} agent runs · 🎯 ${recap.goalsDone}/${recap.goalsTotal} goals`,
        },
      ),
    ),
  );
}

function renderDailyGoalsCard(): HTMLElement {
  const goals = State.dailyGoals;
  const doneCount = goals.filter((g) => g.done).length;

  const addInput = h("input", {
    class: "p6-goal-input",
    type: "text",
    placeholder: "+ Add a new to-do for today & press Enter...",
    onclick: (e: Event) => {
      e.stopPropagation();
      void Bridge.focusWindow(true);
    },
    onfocus: () => {
      void Bridge.focusWindow(true);
    },
    onblur: () => {
      void Bridge.focusWindow(false);
    },
  }) as HTMLInputElement;

  const submitAddGoal = () => {
    const val = addInput.value.trim();
    if (!val) return;
    addInput.value = "";
    void Bridge.focusWindow(false);
    void Workflow.addDailyGoal(val);
  };

  addInput.addEventListener("keydown", (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Enter") {
      e.preventDefault();
      submitAddGoal();
    } else if (e.key === "Escape") {
      e.preventDefault();
      addInput.blur();
      void Bridge.focusWindow(false);
    }
  });

  const addBtn = h(
    "button",
    {
      class: "p3-btn p6-goal-add-btn",
      title: "Add to-do item",
      onclick: (e: Event) => {
        e.stopPropagation();
        submitAddGoal();
      },
    },
    h("span", { text: "+ Add" }),
  );

  const buildGoalRow = (g: import("../core/bridge").DailyGoalRecord): HTMLElement => {
    const textSpan = h("span", {
      class: "p6-goal-text",
      text: `${g.slot}. ${g.title}`,
      title: "Click to toggle done · Double-click or click ✎ to rename",
    });

    const editInput = h("input", {
      class: "p6-goal-edit-input",
      type: "text",
      value: g.title,
      style: "display:none",
      onclick: (e: Event) => e.stopPropagation(),
    }) as HTMLInputElement;

    let editingRow = false;
    const startEdit = (e: Event) => {
      e.stopPropagation();
      editingRow = true;
      textSpan.style.display = "none";
      editInput.style.display = "";
      editInput.value = g.title;
      void Bridge.focusWindow(true);
      requestAnimationFrame(() => {
        editInput.focus();
        editInput.select();
      });
    };

    const commitEdit = () => {
      if (!editingRow) return;
      editingRow = false;
      const nextVal = editInput.value.trim();
      editInput.style.display = "none";
      textSpan.style.display = "";
      void Bridge.focusWindow(false);
      if (nextVal && nextVal !== g.title) {
        void Workflow.setDailyGoal(g.slot, nextVal);
      }
    };

    editInput.addEventListener("keydown", (e: KeyboardEvent) => {
      e.stopPropagation();
      if (e.key === "Enter") {
        e.preventDefault();
        commitEdit();
      } else if (e.key === "Escape") {
        e.preventDefault();
        editingRow = false;
        editInput.style.display = "none";
        textSpan.style.display = "";
        void Bridge.focusWindow(false);
      }
    });
    editInput.addEventListener("blur", () => commitEdit());

    return h(
      "div",
      {
        class: `p6-goal-item ${g.done ? "done" : ""}`,
        onclick: (e: Event) => {
          e.stopPropagation();
          if (!editingRow) {
            void Workflow.toggleDailyGoal(g.id);
          }
        },
        ondblclick: startEdit,
      },
      h("span", {
        class: "p6-goal-check",
        text: g.done ? "✅" : "⬜",
        title: "Click to toggle completion (+8 pts)",
      }),
      textSpan,
      editInput,
      h(
        "div",
        { class: "p6-goal-actions" },
        h(
          "button",
          {
            class: "p6-goal-act-btn",
            title: "Rename to-do item",
            onclick: startEdit,
          },
          h("span", { text: "✎" }),
        ),
        h(
          "button",
          {
            class: "p6-goal-act-btn p6-goal-del-btn",
            title: "Delete to-do item",
            onclick: (e: Event) => {
              e.stopPropagation();
              void Workflow.deleteDailyGoal(g.id);
            },
          },
          h("span", { text: "✕" }),
        ),
      ),
    );
  };

  return h(
    "div",
    { class: "int-card wf-card p6-goals-card" },
    h(
      "div",
      { class: "int-head p6-card-head" },
      dot("#10B981", 7),
      h("b", { text: `🎯 Today's To-Do (${doneCount}/${goals.length})` }),
      h("span", { text: "+8 pts each · Resets at 00:00" }),
      h(
        "button",
        {
          class: "p3-btn p6-close-btn",
          title: "Close daily to-do list",
          onclick: (e: Event) => {
            e.stopPropagation();
            Workflow.toggleDailyGoalsCard();
          },
        },
        h("span", { text: "✕" }),
      ),
    ),
    h(
      "div",
      { class: "p6-goal-add-row" },
      addInput,
      addBtn,
    ),
    h(
      "div",
      { class: "p6-goals-list" },
      ...(goals.length > 0
        ? goals.map(buildGoalRow)
        : [
            h("div", {
              class: "p6-goals-empty",
              text: "No to-dos yet for today — type above & press Enter to add one!",
            }),
          ]),
    ),
  );
}

function renderWorkflowQuoteCard(task: AgentTask | null, message: string, detail?: string): HTMLElement {
  const color = task?.color ?? "#3b82f6";
  const label = task ? (task.id === "integration_claude" ? "VS Code" : task.name) : "Coucou";
  return h(
    "div",
    {
      class: "int-card wf-card wf-clickable",
      title: "Click to jump to window",
      onclick: () => {
        if (State.activeAgentWait) {
          void Workflow.focusAndAcknowledgeAgent();
        } else if (task) {
          void Bridge.focusTrackerActivateWindow(
            task.id,
            task.windowPid ?? 0,
            task.windowClass ?? task.name,
          );
        }
      },
    },
    h(
      "div",
      { class: "int-head" },
      dot(color, 7),
      h("b", { text: label }),
      h("span", { text: "Focus" }),
    ),
    h("div", { class: "wf-note-msg", text: message }),
    detail ? h("div", { class: "wf-note-detail", text: detail }) : null,
  );
}

function shortenInlinePaths(input: string): string {
  // Replace quoted paths like "/run/media/.../Coucou_0.2.0_amd64.deb" with their basename
  const withoutQuotedPaths = input.replace(/"([^"]+)"|'([^']+)'/g, (full, dquote, squote) => {
    const inner = (dquote || squote || "").trim();
    if (inner.startsWith("/") && inner.split("/").filter(Boolean).length >= 2) {
      return inner.split("/").filter(Boolean).pop() || inner;
    }
    return full;
  });
  // Also shorten unquoted absolute paths
  return withoutQuotedPaths
    .split(/\s+/)
    .map((tok) => {
      if (tok.startsWith("/") && tok.split("/").filter(Boolean).length >= 2) {
        return tok.split("/").filter(Boolean).pop() || tok;
      }
      return tok;
    })
    .join(" ");
}

function cleanTaskLabel(raw: string): string {
  const trimmed = (raw || "").trim();
  if (!trimmed) return "Deep work session";
  const lower = trimmed.toLowerCase();
  if (lower.includes("dpkg -i") || lower.includes(">/dev/null") || lower.includes("killall coucou")) {
    return "Deep work session";
  }
  const shortened = shortenInlinePaths(trimmed.split(/\s+[—–]\s+/)[0].trim() || trimmed)
    .replace(/\s*>\s*\/dev\/null.*$/i, "")
    .trim();
  return shortened.length > 44 ? `${shortened.slice(0, 42).trimEnd()}…` : shortened;
}

function parseStructuredStatusChips(
  rawCaption: string,
  taskName: string,
): Array<{ text: string; isCode?: boolean }> {
  const raw = (rawCaption || "").trim();
  if (!raw) return [{ text: taskName || "Active" }];

  // Strip trailing app title suffix like " — Konsole", " — Antigravity", " - Brave"
  const withoutAppSuffix = raw
    .replace(/\s+[—–-]\s+(Konsole|Terminal|Antigravity|Visual Studio Code|VS Code|Kiro|Brave|Google Chrome|Firefox|Cursor)\s*$/i, "")
    .trim();

  const looksLikeShellCmd =
    withoutAppSuffix.includes("&&") ||
    withoutAppSuffix.includes(">/dev/null") ||
    withoutAppSuffix.includes("2>&1") ||
    /^(sudo|dpkg|apt|cargo|npm|npx|git|killall|nohup|systemctl|docker|python|node|sh|bash|zsh)\b/i.test(
      withoutAppSuffix,
    );

  if (looksLikeShellCmd) {
    const stages = withoutAppSuffix
      .split(/\s*(?:&&|;)\s*/)
      .map((part) =>
        shortenInlinePaths(part)
          .replace(/\s*>\s*\/dev\/null.*$/i, "")
          .replace(/\s*2>&1\s*&?\s*$/i, "")
          .replace(/\s+&\s*$/, "")
          .trim(),
      )
      .filter(Boolean);

    if (stages.length > 0) {
      return stages.slice(0, 3).map((cmd, idx) => {
        const icon = idx === 0 ? "🐚" : cmd.startsWith("kill") ? "⚡" : "▶";
        const capped = cmd.length > 38 ? `${cmd.slice(0, 36).trimEnd()}…` : cmd;
        return { text: `${icon} ${capped}`, isCode: true };
      });
    }
  }

  // IDE or multi-segment window title: "views.ts — COUCOU"
  const segments = withoutAppSuffix
    .split(/\s+[—–]\s+/)
    .map((s) => shortenInlinePaths(s.trim()))
    .filter(Boolean);

  if (segments.length >= 2) {
    return segments.slice(0, 3).map((seg, idx) => {
      const capped = seg.length > 38 ? `${seg.slice(0, 36).trimEnd()}…` : seg;
      const prefix = idx === 0 ? "📄 " : idx === 1 ? "📁 " : "";
      return { text: `${prefix}${capped}` };
    });
  }

  const single = shortenInlinePaths(withoutAppSuffix);
  const capped = single.length > 56 ? `${single.slice(0, 54).trimEnd()}…` : single;
  return [{ text: capped }];
}

function populateStructuredWindowStatus(
  container: HTMLElement,
  statusColor: string,
  rawCaption: string,
  taskName: string,
) {
  clear(container);
  container.dataset.rawCaption = rawCaption;
  container.title = rawCaption;
  container.append(dot(statusColor, 6));

  const chips = parseStructuredStatusChips(rawCaption, taskName);
  for (const chip of chips) {
    container.append(
      h("span", {
        class: `wf-status-chip ${chip.isCode ? "cmd" : ""}`,
        text: chip.text,
        title: rawCaption,
      }),
    );
  }
}

function renderDesktopWindowCard(task: AgentTask, quoteText: string, detailText = ""): HTMLElement {
  const isActive = task.windowActive ?? (task.state !== "idle");
  const statusColor = isActive ? "#22C55E" : "#6B7079";
  const statusLabel = detailText || task.windowCaption || task.name;
  const isBrowser =
    task.name === "Browser" ||
    (task.windowClass || "").toLowerCase().includes("brave");

  const bDomain = isBrowser ? State.browserDomain : null;
  const domainBadgeEl = bDomain
    ? h("span", {
        class: `p4-domain-badge p4-cat-${bDomain.category}`,
        text: `${bDomain.domain} · ${bDomain.category}`,
      })
    : null;

  const statusContainer = h("div", { class: "int-status wf-structured-status" });
  populateStructuredWindowStatus(statusContainer, statusColor, statusLabel, task.name);

  return h(
    "div",
    { class: "int-card wf-card" },
    h(
      "div",
      {
        class: "int-head wf-win-head",
        title: "Click to jump to window",
        onclick: () => {
          void Bridge.focusTrackerActivateWindow(
            task.id,
            task.windowPid ?? 0,
            task.windowClass ?? task.name,
          );
        },
      },
      dot(task.color, 7),
      h("b", { text: task.name }),
      h("span", { text: isActive ? "Active" : "Idle" }),
      domainBadgeEl,
    ),
    quoteText
      ? h("div", { class: "wf-note-msg", text: quoteText })
      : null,
    statusContainer,
  );
}

function buildPill(task: AgentTask, actions: ViewActions): HTMLElement {
  const label = task.id === "integration_claude" ? "VS Code" : task.name;
  const tooltip = task.windowCaption ? `${label} — ${task.windowCaption}` : label;
  const canvas = createMiniBot(task, 24);
  const pill = h(
    "div",
    { class: "pill", title: tooltip, onclick: () => actions.setFocus(task.id) },
    canvas,
    h("span", { class: "lbl", text: label }),
  );
  pill.style.borderColor = `${task.color}24`;
  
  pill.addEventListener("mouseenter", () => {
    pill.style.background = `${task.color}2e`;
    pill.style.borderColor = `${task.color}8c`;
    pill.style.boxShadow = `0 2px 10px ${task.color}59`;
    (pill.querySelector(".lbl") as HTMLElement).style.color = lighten(task.color, 0.3);
  });
  pill.addEventListener("mouseleave", () => {
    pill.style.background = "";
    pill.style.borderColor = `${task.color}24`;
    pill.style.boxShadow = "";
    (pill.querySelector(".lbl") as HTMLElement).style.color = "";
  });

  if (task.pillBadge) {
    const colors = { approval: "#F5A524", finished: "#22C55E", error: "#F4505E" } as const;
    const icons = { approval: ICONS.bang, finished: ICONS.check, error: ICONS.xmark } as const;
    const inner = h("i", { style: `background:${colors[task.pillBadge]}` }, svg(icons[task.pillBadge], 6, { stroke: task.pillBadge === "finished" ? 3 : 0 }));
    const badge = h("div", { class: "pill-badge" }, inner);
    badge.style.boxShadow = `0 0 4px ${colors[task.pillBadge]}99`;
    pill.append(badge);
  }
  return pill;
}

function lighten(hex: string, amount: number): string {
  const v = parseInt(hex.replace("#", ""), 16);
  const c = [(v >> 16) & 255, (v >> 8) & 255, v & 255].map((x) =>
    Math.min(255, Math.round(x + amount * 255)),
  );
  return `rgb(${c[0]},${c[1]},${c[2]})`;
}

// ── Empty ─────────────────────────────────────────────────────────────────────

function buildEmpty(actions: ViewActions): ViewHost {
  const body = h(
    "div",
    { class: "stack", style: "padding:0 18px 0 118px;flex-direction:row;align-items:center;gap:16px" },
    h(
      "div",
      { style: "display:flex;flex-direction:column;gap:5px" },
      h("div", { class: "title", text: tl("Nothing running right now.") }),
      h("div", { class: "sub", text: tl("Drop a file or window, or ask me anything.") }),
    ),
    h("div", { class: "grow" }),
    btn(tl("Ask Claude"), "primary", () => actions.setView("prompt")),
  );
  return { el: h("div", { class: "view" }, card(null, body)), sync() {} };
}

// ── Approval ──────────────────────────────────────────────────────────────────

/** How long a fresh permission card ignores clicks on its buttons. */
const CLICK_GUARD_MS = 600;

/**
 * The ⌃ in the corner of a waiting card: folds the island to its compact size
 * and leaves the request waiting — nothing is answered (Mac #290). Opening the
 * island again brings the card back.
 */
function foldButton(actions: ViewActions): HTMLElement {
  return h(
    "button",
    { class: "icon-btn fold", title: tl("Later — keep it waiting"), onclick: () => actions.foldApproval() },
    svg(ICONS.chevronUp, 8, { stroke: 2.4 }),
  );
}

function createInlinePromptBar(
  onSubmit: (val: string) => void,
  onSkipToTerminal: () => void,
): {
  rowEl: HTMLElement;
  inputEl: HTMLInputElement;
  configure: (isPassword: boolean, placeholder: string) => void;
} {
  const inputEl = h("input", {
    type: "text",
    class: "coucou-inline-prompt-input",
    placeholder: "Type input or option & press Enter…",
    autocomplete: "off",
    spellcheck: "false",
    onmousedown: (e: Event) => {
      e.stopPropagation();
      void Bridge.focusWindow(true);
    },
    onfocus: () => {
      void Bridge.focusWindow(true);
    },
    onblur: () => {
      void Bridge.focusWindow(false);
    },
    onkeydown: (ev: Event) => {
      const e = ev as KeyboardEvent;
      e.stopPropagation();
      if (e.key === "Enter") {
        e.preventDefault();
        const val = inputEl.value;
        inputEl.value = "";
        void Bridge.focusWindow(false);
        onSubmit(val);
      } else if (e.key === "Escape") {
        e.preventDefault();
        inputEl.blur();
        void Bridge.focusWindow(false);
      }
    },
  }) as HTMLInputElement;

  const sendBtn = h(
    "button",
    {
      class: "btn primary coucou-inline-send-btn",
      title: "Send input directly from Coucou (Enter)",
      onclick: (e: Event) => {
        e.stopPropagation();
        const val = inputEl.value;
        inputEl.value = "";
        void Bridge.focusWindow(false);
        onSubmit(val);
      },
    },
    h("span", { text: "Send ↵" }),
  );

  const termBtn = h(
    "button",
    {
      class: "link-btn coucou-inline-term-btn",
      title: "Focus application / terminal window",
      onclick: (e: Event) => {
        e.stopPropagation();
        void Bridge.focusWindow(false);
        onSkipToTerminal();
      },
    },
    h("span", { text: "Terminal ↗" }),
  );

  const rowEl = h(
    "div",
    { class: "coucou-inline-input-row" },
    inputEl,
    sendBtn,
    termBtn,
  );

  return {
    rowEl,
    inputEl,
    configure(isPassword: boolean, placeholder: string) {
      const targetType = isPassword ? "password" : "text";
      if (inputEl.type !== targetType) inputEl.type = targetType;
      if (inputEl.placeholder !== placeholder) inputEl.placeholder = placeholder;
    },
  };
}

function buildApproval(actions: ViewActions): ViewHost {
  const who = h("div");
  const code = h("div", { class: "code" });
  const row = h("div", { class: "actions coucou-interactive-actions" });
  const inlineBar = createInlinePromptBar(
    (val) => actions.submitInput(val),
    () => {
      actions.openTarget();
      actions.answerInTerminal();
    },
  );
  const el = h(
    "div",
    { class: "view" },
    card("amber", stack(116, 16, who, code, row, inlineBar.rowEl), foldButton(actions)),
  );
  let rowKey = "";
  let shownFor: string | null = null;
  let shownAt = 0;
  const guardedDecide = (d: "allow" | "deny") => () => {
    if (performance.now() - shownAt < CLICK_GUARD_MS) return;
    actions.decide(d);
  };
  const guardedInput = (val: string) => () => {
    if (performance.now() - shownAt < CLICK_GUARD_MS) return;
    if (val === "y" || val === "allow") actions.decide("allow");
    else if (val === "n" || val === "deny") actions.decide("deny");
    else actions.submitInput(val);
  };
  return {
    el,
    focus() {
      if (
        State.pendingApproval?.promptKind === "password" ||
        State.pendingApproval?.promptKind === "input"
      ) {
        void Bridge.focusWindow(true);
        inlineBar.inputEl.focus();
      }
    },
    sync() {
      const pending = State.pendingApproval;
      const req = pending?.requestId ?? null;
      if (req !== shownFor) {
        shownFor = req;
        shownAt = performance.now();
        inlineBar.inputEl.value = "";
      }
      clear(who);
      const pKind = pending?.promptKind ?? "";
      const statusText =
        pKind === "password"
          ? "🔒 password required"
          : pKind === "confirm"
            ? "⌨ waiting for confirmation"
            : pKind === "input"
              ? "⌨ waiting for input"
              : t("needs permission");
      who.append(agentWho(State.focusTask, statusText));
      code.textContent = pending?.command || pending?.tool || "…";
      code.title = pending?.command || pending?.tool || "";

      const isPass = pKind === "password";
      inlineBar.configure(
        isPass,
        isPass
          ? "Enter sudo / SSH password & press Enter…"
          : "Type input, option, or command reply & press Enter…",
      );

      const quickOpts = pending?.quickOptions ?? [];
      const nextKey = `${req ?? "none"}:${pKind}:${quickOpts.map((o) => `${o.label}=${o.value}`).join("|")}`;
      if (rowKey === nextKey) return;
      rowKey = nextKey;
      clear(row);

      if (quickOpts.length > 0) {
        for (const opt of quickOpts) {
          const isPrimary = opt.value === "y" || opt.value === "allow" || opt.value === "1";
          row.append(
            btn(
              opt.label,
              isPrimary ? "primary" : "secondary",
              guardedInput(opt.value),
              opt.value === "y" ? "Y" : opt.value === "n" ? "N" : undefined,
            ),
          );
        }
      } else {
        row.append(
          btn(tl("Deny"), "secondary", guardedDecide("deny"), "N"),
          btn(tl("Allow"), "primary", guardedDecide("allow"), "Y"),
        );
      }
    },
  };
}

// ── Question ──────────────────────────────────────────────────────────────────

function buildQuestion(actions: ViewActions): ViewHost {
  const who = h("div");
  const title = h("div", { class: "title question-text" });
  const row = h("div", { class: "actions options coucou-interactive-actions" });
  const fold = foldButton(actions);

  let requestId = "";
  let index = 0;
  let answers: Record<string, string | string[]> = {};
  let picked = new Set<string>();
  let rowKey = "";

  const next = (question: string, answer: string | string[], total: number) => {
    answers[question] = answer;
    picked = new Set();
    index += 1;
    if (index >= total) actions.answer(answers);
    else State.notify();
  };

  const inlineBar = createInlinePromptBar(
    (val) => {
      const questions = State.pendingApproval?.questions;
      if (questions && questions.length > 0) {
        const q = questions[Math.min(index, questions.length - 1)];
        next(q.question, val, questions.length);
      } else {
        actions.submitInput(val);
      }
    },
    () => {
      actions.openTarget();
      actions.answerInTerminal();
    },
  );

  const el = h(
    "div",
    { class: "view" },
    card("cyan", stack(116, 16, who, title, row, inlineBar.rowEl), fold),
  );

  return {
    el,
    sync() {
      const pending = State.pendingApproval;
      const questions = pending?.questions;
      clear(who);
      fold.style.display = pending ? "" : "none";
      inlineBar.configure(false, "Or type custom answer / option number & press Enter…");

      // A question that arrived without structured options still gets quick buttons + input bar!
      if (!questions || questions.length === 0) {
        who.append(agentWho(State.focusTask, t("is asking a question")));
        const task = State.focusTask;
        title.textContent =
          pending?.command || (task && lastTextStep(task.steps)) || t("Waiting for your response…");
        if (rowKey !== "terminal-quick") {
          rowKey = "terminal-quick";
          clear(row);
          for (const quick of [
            { label: "Yes (y)", val: "y", primary: true },
            { label: "No (n)", val: "n", primary: false },
            { label: "Option 1", val: "1", primary: false },
            { label: "Option 2", val: "2", primary: false },
            { label: "Option 3", val: "3", primary: false },
          ]) {
            row.append(
              btn(quick.label, quick.primary ? "primary" : "secondary", () =>
                actions.submitInput(quick.val),
              ),
            );
          }
        }
        return;
      }

      if (pending!.requestId !== requestId) {
        requestId = pending!.requestId;
        index = 0;
        answers = {};
        picked = new Set();
        inlineBar.inputEl.value = "";
      }
      const q = questions[Math.min(index, questions.length - 1)];
      const asking =
        questions.length > 1
          ? t("is asking ({index} of {total})", { index: index + 1, total: questions.length })
          : t("is asking");
      who.append(agentWho(State.focusTask, asking));
      title.textContent = q.question;
      title.title = q.question;

      const key = `${requestId}:${index}:${[...picked].join("|")}:${q.options.map((o) => o.label).join(",")}`;
      if (rowKey === key) return;
      rowKey = key;
      clear(row);
      for (const option of q.options) {
        const on = picked.has(option.label);
        const button = btn(option.label, on ? "primary" : "secondary", () => {
          if (!q.multiSelect) {
            next(q.question, option.label, questions.length);
            return;
          }
          if (on) picked.delete(option.label);
          else picked.add(option.label);
          State.notify();
        });
        if (option.description) button.title = option.description;
        row.append(button);
      }
      if (q.multiSelect) {
        const done = btn(tl("Done"), "primary", () => {
          if (picked.size > 0) next(q.question, [...picked], questions.length);
        });
        if (picked.size === 0) done.classList.add("off");
        row.append(done);
      }
    },
  };
}

// ── Error ─────────────────────────────────────────────────────────────────────

function buildError(actions: ViewActions): ViewHost {
  const who = h("div");
  const title = h("div", { class: "title" });
  const detail = h("div", { class: "detail" });
  const row = h("div", { class: "actions" },
    btn(tl("Retry"), "primary", () => actions.setView(State.defaultView())),
    btn(tl("Open in n8n"), "secondary", () => actions.openUrl("")),
  );
  const el = h("div", { class: "view" }, card("red", stack(116, 16, who, title, detail, row)));
  return {
    el,
    sync() {
      const task = State.focusTask;
      clear(who);
      // agentWho already shows an agent's name: its label is just the kind.
      const whoLabel = task?.source === "n8n" ? "n8n" : task?.source === "agent" ? t("Agent") : "Claude Code";
      who.append(agentWho(task, whoLabel));
      title.textContent = task?.source === "n8n" ? t("Workflow stopped.") : t("Session stopped on an error.");
      detail.textContent = (task && lastTextStep(task.steps)) ?? t("No detail available.");
    },
  };
}

// ── Finished ──────────────────────────────────────────────────────────────────

function buildFinished(actions: ViewActions): ViewHost {
  const who = h("div");
  const title = h("div", { class: "title one-line" });
  const open = btn(tl("Open terminal"), "primary", () => actions.openTerminal());
  const row = h("div", { class: "actions" },
    open,
    btn(tl("OK"), "secondary", () => actions.collapse()),
  );
  const el = h("div", { class: "view" }, card("green", stack(116, 16, who, title, row)));
  return {
    el,
    sync() {
      clear(who);
      const agent = State.focusTask?.source === "agent";
      who.append(agentWho(State.focusTask, agent ? t("finished") : t("Claude Code finished")));
      // The final message, else the last step that is not a diff (FinishedView).
      const task = State.focusTask;
      title.textContent = task?.finalLine || (task && lastTextStep(task.steps)) || t("Session finished");
      // Sessions from the Claude desktop app live there, not in a terminal.
      const label = task?.id === "agent_claude-desktop" ? t("Open Claude") : t("Open terminal");
      const span = open.firstElementChild as HTMLElement;
      if (span.textContent !== label) span.textContent = label;
    },
  };
}

// ── Confused ──────────────────────────────────────────────────────────────────

function buildConfused(): ViewHost {
  const body = h(
    "div",
    { class: "stack", style: "padding:0 18px 0 128px" },
    h("div", { class: "title", text: tl("Too many hits at once.") }),
    h("div", { class: "sub", text: tl("Give me a sec — back to work in three seconds.") }),
  );
  return { el: h("div", { class: "view" }, card("pink", body)), sync() {} };
}

// ── Note ──────────────────────────────────────────────────────────────────────

function buildNote(): ViewHost {
  const title = h("div", { class: "title" });
  const el = h("div", { class: "view" }, card(null, h("div", { class: "stack", style: "padding:0 18px 0 98px" }, title)));
  return {
    el,
    sync() {
      title.textContent = State.noteMessage ?? "";
    },
  };
}

// ── In-island settings ────────────────────────────────────────────────────────

function buildSettings(actions: ViewActions): ViewHost {
  const soundSwitch = h("button", { class: "switch", onclick: () => actions.toggleSound() });
  const volume = h("input", {
    type: "range", min: "0", max: "0.2", step: "0.005",
    oninput: (e: Event) => actions.setVolume(Number((e.target as HTMLInputElement).value)),
  }) as HTMLInputElement;
  const autoLabel = h("span", {});
  const segButtons = [10, 15, 30].map((s) =>
    h("button", { onclick: () => actions.setAutoClose(s) }, `${s}s`),
  );
  const sizePresets: { id: "compact" | "comfortable" | "large"; label: string }[] = [
    { id: "compact", label: "Compact" },
    { id: "comfortable", label: "Comfortable" },
    { id: "large", label: "Large" },
  ];
  const sizeButtons = sizePresets.map((p) =>
    h("button", { onclick: () => actions.setIslandSize(p.id) }, p.label),
  );
  const claudeBadge = h("span", { class: "status-badge" });
  const apiBadge = h("span", { class: "status-badge" });

  const rows = h(
    "div",
    { class: "settings-rows" },
    h("div", { class: "settings-row" }, soundSwitch, h("span", { text: tl("Sound") }), volume),
    h(
      "div",
      { class: "settings-row" },
      svg(ICONS.timer, 12),
      autoLabel,
      h("div", { class: "seg" }, ...segButtons),
    ),
    h(
      "div",
      { class: "settings-row" },
      h("span", { text: "Island size" }),
      h("div", { class: "seg" }, ...sizeButtons),
    ),
    h(
      "div",
      { class: "settings-row", style: "gap:14px" },
      claudeBadge,
      apiBadge,
      h("div", { class: "grow" }),
      h("button", {
        class: "link-btn",
        style: "color:#8e939c;font-size:11.5px",
        text: tl("Settings…"),
        onclick: () => actions.openSettingsWindow(),
      }),
    ),
  );

  const el = h("div", { class: "view" },
    card(null, h("div", { class: "stack", style: "padding:14px 16px 14px var(--card-left-pad, 122px)" }, rows)));

  return {
    el,
    sync() {
      const s = State.settings;
      soundSwitch.classList.toggle("on", s.soundEnabled);
      volume.value = String(s.soundVolume);
      volume.style.opacity = s.soundEnabled ? "1" : "0.4";
      autoLabel.textContent = t("Auto-close · {seconds}s", { seconds: Math.round(s.autoCloseInterval) });
      segButtons.forEach((b, i) => b.classList.toggle("on", s.autoCloseInterval === [10, 15, 30][i]));
      const activeSize = (s.islandSize || "comfortable").toLowerCase();
      sizeButtons.forEach((b, i) => b.classList.toggle("on", activeSize === sizePresets[i].id));
      clear(claudeBadge);
      claudeBadge.append(
        dot(s.hooksInstalled ? "#22C55E" : "#F4505E", 6),
        h("span", { text: "Claude Code" }),
      );
      clear(apiBadge);
      apiBadge.append(dot("#F4505E", 6), h("span", { text: "API" }));
    },
  };
}

// ── Placeholders filled in later stages ───────────────────────────────────────

function buildPlaceholder(title: Msg, sub: string): ViewHost {
  const body = h(
    "div",
    { class: "stack", style: "padding:0 18px 0 118px" },
    h("div", { class: "title", text: title }),
    h("div", { class: "sub", text: sub }),
  );
  return { el: h("div", { class: "view" }, card(null, body)), sync() {} };
}

// ── Registry ──────────────────────────────────────────────────────────────────

export function buildViews(
  actions: ViewActions,
  onChatHeightChange: () => void,
): Map<IslandViewName, ViewHost> {
  const map = new Map<IslandViewName, ViewHost>();
  map.set("overview", buildOverview(actions));
  map.set("empty", buildEmpty(actions));
  map.set("approval", buildApproval(actions));
  map.set("question", buildQuestion(actions));
  map.set("error", buildError(actions));
  map.set("finished", buildFinished(actions));
  map.set("confused", buildConfused());
  map.set("note", buildNote());
  map.set("settings", buildSettings(actions));
  map.set("prompt", buildPrompt(onChatHeightChange));
  map.set("upload", buildUpload(actions));
  map.set("uploading", buildUploading());
  map.set("choose", buildChoose(actions));
  map.set("recap", buildRecap(actions));
  map.set("wardrobe", buildWardrobe(actions));
  // Not in the Windows v1: sending a file by email, window attach + web result.
  map.set("mail", buildPlaceholder(tl("Sending by email isn't in this version."), ""));
  map.set("searching", buildPlaceholder(tl("Claude is searching…"), ""));
  map.set("result", buildPlaceholder(tl("Result"), ""));
  return map;
}
