// The island: DOM shell, sizing animation, Mochi placement, mouse handling.
// Mirrors IslandRootView.swift + IslandWindowController.swift.

import { Tracked, Spring, clamp } from "../core/anim";
import { Bridge, IS_TAURI, onDragDrop } from "../core/bridge";
import {
  EXPANDED_CORNER, EXPANDED_W, NOTCH_W, PANEL_H, PANEL_W,
  ROUNDED_CORNER, VIEW_LAYOUTS, botGlowColor, botGlowOpacity, botPosition, chatPromptHeight,
  islandSize,
  QUESTION_PICKER_H,
  type BotEmoteName, type IslandMode, type IslandViewName,
} from "../core/layout";
import {
  applyIslandSizeCssVars,
  getIslandSizeConfig,
  resolveIslandSizePreset,
  type IslandSizePreset,
} from "../core/size-config";
import { Sound } from "../core/sound";
import { State } from "../core/state";
import { BotEngine, hexToRGB } from "../mochi/engine";
import { Greeting } from "../mochi/greeting";
import { createMiniBot, pruneMiniBots, syncMiniBotStates, tickMiniBots } from "../mochi/minibots";
import { SeasonCache, parseOutfit } from "../mochi/wardrobe";
import { UploadCanvas } from "../upload/canvas";
import { USC, UploadSeq } from "../upload/sequence";
import { closePlanCard, openPlanColor, planCardOpen } from "../views/usage";
import { buildHeader, buildViews, type ViewActions, type ViewHost } from "../views/views";
import { h } from "../views/dom";
import { IslandStateMachine } from "./fsm";
import { refreshHookPills } from "./integrations";
import { DesktopLink } from "./desktop";
import { DRAG_THRESHOLD } from "../mochi/desktop-logic";
import type { WorkflowNotification } from "../core/workflow";

const BOT_OVERHANG = 40;
const CLAUDE_DESKTOP_ID = "agent_claude-desktop";
/** Extra canvas on each side of Mochi, for the witch hat's brim and the Santa hat's tip. */
const BOT_SIDE = 24;
/** Same margin as the Rust hit test (src-tauri/src/island.rs). */
const HIT_MARGIN = 14;

/** The three views the drop sequence owns; leaving them stops the engine. */
const UPLOAD_VIEWS: ReadonlySet<IslandViewName> = new Set(["upload", "uploading", "choose"]);

/** Seconds between the drop and the moment the progress bar starts filling. */
const PRE_PROGRESS = USC.T_PROG_START - USC.T_DROP;

const modeOrder = (m: IslandMode) => (m === "hidden" ? 0 : m === "compact" ? 1 : 2);

export class Island {
  readonly fsm = new IslandStateMachine();
  /** Mochi on the desktop: his life cycle and the drag out of the island. */
  readonly desktop: DesktopLink;

  private root: HTMLElement;
  private islandEl!: HTMLElement;
  private clipEl!: HTMLElement;
  private contentEl!: HTMLElement;
  private viewsEl!: HTMLElement;
  private botCanvas!: HTMLCanvasElement;
  private botGlow!: HTMLElement;
  private greetingCanvas!: HTMLCanvasElement;
  private miniGrid!: HTMLElement;
  private countdown!: HTMLElement;
  private wakeStrip!: HTMLElement;

  private header!: ViewHost;
  private views!: Map<IslandViewName, ViewHost>;
  private uploadCanvas!: UploadCanvas;

  private width = new Tracked(NOTCH_W);
  private height = new Tracked(0);
  private radius = new Tracked(ROUNDED_CORNER);
  private botCx = new Spring(46, 0.22, 1.0);
  private botCy = new Spring(27, 0.22, 1.0);
  private botSize = new Spring(10, 0.22, 1.0);

  private engine = new BotEngine();
  private greeting = new Greeting();
  private greetingShown = false;
  private seasons = new SeasonCache();

  private running = false;
  private lastFrame = 0;
  private dirty = true;
  private canvasPx = 0;

  // Rust starts the window at full size so the launch greeting has room.
  private collapsed = false;
  private collapseTimer: number | null = null;
  private wasInIsland = false;
  /** Last shape handed to Rust for the click-through test. */
  private pushedRect = { x: -1, y: -1, w: -1, h: -1 };
  /** Current logical size of the native Tauri window. */
  private windowSize = { w: PANEL_W, h: PANEL_H };
  private lastTargetSize = { w: NOTCH_W, h: 0 };
  private windowResizeTimer: number | null = null;
  private resizeRafId: number | null = null;

  // Bot hover → love (IslandWindowController.botHoverIn)
  private botHovering = false;
  private botHoverTimer: number | null = null;
  private lastLoveTime = 0;
  private botHoverStart = { x: 0, y: 0 };

  private confusedRecovery: number | null = null;
  private prevViewBeforeConfused: IslandViewName = "overview";
  private lastSyncedView: IslandViewName | null = null;

  /** The launch greeting ended, or the island came out of hidden — two of the
   *  moments the Monday recap may open (see src/recap/recap.ts). */
  onGreetingDone: (() => void) | null = null;
  onWake: (() => void) | null = null;

  /** Where a press on Mochi started: moving past DRAG_THRESHOLD drags him out. */
  private botPress: { x: number; y: number } | null = null;

  /** Drop sequence bookkeeping: last tick played, and whether the ✓ has fired. */
  private uploadTens = 0;
  private uploadDone = false;
  
  /** The workflow card currently on screen (object identity, or null). */
  private shownWorkflowNote: WorkflowNotification | null = null;
  /** True while the island is open only because a workflow card opened it. */
  private workflowNoteOpened = false;
  private lastLayoutStressMode = "off";
  private lastSizePreset: IslandSizePreset = "comfortable";

  constructor(root: HTMLElement) {
    this.root = root;
    this.desktop = new DesktopLink({
      reveal: () => this.reveal(),
      wardrobeFromDesktop: () => this.wardrobeFromDesktop(),
      dizzyFromDesktop: () => this.handleDizzy(),
    });
    this.build();
    this.wireFsm();
    this.wireInput();
    this.engine.onDizzy = () => this.handleDizzy();
    this.greeting.onComplete = () => {
      this.fsm.greetComplete();
      this.onGreetingDone?.();
    };
    State.subscribe(() => {
      this.dirty = true;
      this.ensureRunning();

      const preset = resolveIslandSizePreset(State.settings.islandSize);
      if (preset !== this.lastSizePreset) {
        const prev = this.lastSizePreset;
        this.lastSizePreset = preset;
        applyIslandSizeCssVars(preset);
        const shrunk =
          getIslandSizeConfig(preset).minExpandedWidth <
          getIslandSizeConfig(prev).minExpandedWidth;
        this.animateGeometry(shrunk);
      }

      if (State.layoutStressMode !== this.lastLayoutStressMode) {
        this.lastLayoutStressMode = State.layoutStressMode;
        if (
          State.layoutStressMode === "on" ||
          State.layoutStressMode === "recap" ||
          State.layoutStressMode === "agent" ||
          State.layoutStressMode === "few"
        ) {
          this.fsm.pinned = true;
          State.isPinned = true;
          this.fsm.forceHome();
          this.setView("overview");
        } else if (State.layoutStressMode === "compact") {
          this.fsm.pinned = true;
          State.isPinned = true;
          this.fsm.forcePetit();
        } else if (State.layoutStressMode === "off") {
          this.fsm.pinned = false;
          State.isPinned = false;
        }
      }
      
      // A workflow notification appeared (the engine already gave it a unique
      // id) — or the active one expired. Both are edges on object identity: the
      // engine nulls State.workflowNotification when the card's time is up.
      const note = State.workflowNotification;
      if (note !== this.shownWorkflowNote) {
        this.shownWorkflowNote = note;
        if (note) this.showWorkflowNotification(note);
        else this.hideWorkflowNotification();
      }
    });
  }

  /**
   * A motivational quote / focus nudge / finished-task alert. Mochi drops the
   * island down and displays the quote inside the main left-hand pill card; it
   * is never allowed to steal the screen from a permission card or greeting.
   */
  private showWorkflowNotification(_notification: WorkflowNotification) {
    // An approval waiting for an answer or the launch greeting always wins.
    if (State.pendingApproval) return;
    if (State.mode === "expanded" && State.view === "greeting") return;
    // Already open on the overview: the overview sync renders it inside leftBody.
    if (State.mode === "expanded" && (State.view === "overview" || State.view === "empty")) {
      this.setView("overview");
      return;
    }
    // Closed (hidden OR compact): drop the island down so the quote in the main
    // left pill can be read, and pin it while the quote is active.
    if (State.mode === "hidden" || State.mode === "compact") {
      this.workflowNoteOpened = true;
      this.fsm.pinned = true;
      State.isPinned = true;
      this.fsm.forceHome();
    }
  }

  /** The card's time is up. Fold the island away again if we opened it. */
  private hideWorkflowNotification() {
    if (!this.workflowNoteOpened) return;
    this.workflowNoteOpened = false;
    State.isPinned = false;
    this.fsm.pinned = false;
    // Never collapse a card the user opened, or while the mouse is inside.
    if (State.pendingApproval || this.wasInIsland) return;
    if (State.mode === "expanded" && (State.view === "overview" || State.view === "empty")) {
      this.fsm.forcePetit();
    }
  }

  /** The request has its answer: the card goes and the session carries on. */
  private closeApproval() {
    State.endApproval();
    this.fsm.pinned = false;
    this.setView(State.defaultView());
  }

  /**
   * Folds a card that is waiting for an answer down to the compact island,
   * without answering it (Mac #290). Nothing is decided: the request keeps
   * waiting, the island stays on screen, and opening it shows the card again.
   */
  foldApproval() {
    if (!State.pendingApproval || State.mode !== "expanded") return;
    State.isPinned = true;
    this.fsm.pinned = true;
    this.fsm.forcePetit();
  }

  // ── DOM ─────────────────────────────────────────────────────────────────────

  private build() {
    const actions: ViewActions = {
      setView: (v) => this.setView(v),
      cancelDrop: () => this.discardDrop(),
      collapse: () => this.collapse(),
      foldApproval: () => this.foldApproval(),
      setFocus: (id) => {
        State.setFocus(id);
        Sound.play("blip");
        // A pill with a waiting request opens on its card: going back to it
        // after looking at another pill brings the card up again.
        const req = State.pendingApproval;
        if (req?.pillId === id) this.setView(req.questions ? "question" : "approval");
      },
      openTerminal: () => {
        const task = State.focusTask;
        // Sessions from the Claude desktop app live there, not in a terminal.
        if (task?.id === CLAUDE_DESKTOP_ID) void Bridge.openClaudeDesktop();
        else void Bridge.openSession(task?.sessionId ?? null, task?.sessionCwd ?? null);
      },
      // The ↗ button — same targets as openAgentTarget() on macOS.
      openTarget: () => {
        const task = State.focusTask;
        if (!task) return;
        if (task.windowPid !== undefined || task.windowClass !== undefined || task.windowCaption !== undefined) {
          Sound.play("blip");
          void Bridge.focusTrackerActivateWindow(
            task.id,
            task.windowPid ?? 0,
            task.windowClass ?? task.name,
          );
          return;
        }
        const urls: Record<string, string> = {
          integration_resend: "https://resend.com/emails",
          integration_vercel: "https://vercel.com/dashboard",
          integration_github: "https://github.com/pulls",
          integration_stripe: "https://dashboard.stripe.com/payments",
          integration_notion: "https://notion.so",
          integration_calcom: "https://app.cal.com/bookings",
        };
        if (task.id === CLAUDE_DESKTOP_ID) void Bridge.openClaudeDesktop();
        else if (task.id === "integration_claude" || task.sessionId) {
          void Bridge.openSession(task.sessionId ?? null, task.sessionCwd ?? null);
        } else if (task.id === "integration_n8n") void Bridge.openN8n();
        else if (urls[task.id]) void Bridge.openUrl(urls[task.id]);
      },
      openUrl: (url) => {
        if (url) void Bridge.openUrl(url);
      },
      decide: (d) => {
        const req = State.pendingApproval;
        void Bridge.log(`decide ${d} req=${req?.requestId ?? "none"}`);
        if (!req) return;
        Sound.play(d === "deny" ? "blip" : "approve");
        void Bridge.approvalDecision(req.requestId, d);
        this.closeApproval();
      },
      answer: (answers) => {
        const req = State.pendingApproval;
        if (!req) return;
        Sound.play("approve");
        void Bridge.approvalAnswer(req.requestId, answers);
        this.closeApproval();
      },
      answerInTerminal: () => {
        const req = State.pendingApproval;
        if (!req) return;
        Sound.play("blip");
        void Bridge.approvalDecline(req.requestId);
        this.closeApproval();
      },
      toggleSound: () => {
        State.settings.soundEnabled = !State.settings.soundEnabled;
        Sound.setEnabled(State.settings.soundEnabled);
        void Bridge.saveSettings(State.settings);
        State.notify();
      },
      setVolume: (v) => {
        State.settings.soundVolume = v;
        Sound.setVolume(v);
        void Bridge.saveSettings(State.settings);
        State.notify();
      },
      setAutoClose: (s) => {
        State.settings.autoCloseInterval = s;
        this.fsm.homeToPetitDelay = s;
        void Bridge.saveSettings(State.settings);
        State.notify();
      },
      setIslandSize: (preset) => {
        const next = resolveIslandSizePreset(preset);
        const prev = resolveIslandSizePreset(State.settings.islandSize);
        State.settings.islandSize = next;
        this.lastSizePreset = next;
        applyIslandSizeCssVars(next);
        void Bridge.saveSettings(State.settings);
        const shrunk =
          getIslandSizeConfig(next).minExpandedWidth <
          getIslandSizeConfig(prev).minExpandedWidth;
        this.animateGeometry(shrunk);
        State.notify();
      },
      openSettingsWindow: () => void Bridge.openSettingsWindow(),
      blip: () => Sound.play("blip"),
      chooseOutfit: (selection) => {
        if (parseOutfit(State.settings.mochiOutfit) === selection) return;
        State.settings.mochiOutfit = selection;
        void Bridge.saveSettings(State.settings);
        Sound.play("pop");
        this.engine.triggerEmote("proud");
        State.notify();
      },
      previewOutfit: (outfit) => {
        State.wardrobePreview = outfit;
        State.notify();
      },
    };

    applyIslandSizeCssVars(State.settings.islandSize);
    this.lastSizePreset = resolveIslandSizePreset(State.settings.islandSize);

    this.wakeStrip = h("div", { id: "wake-strip" });
    this.botGlow = h("div", { id: "bot-glow" });
    this.botCanvas = h("canvas", { id: "bot-canvas" });
    this.greetingCanvas = h("canvas", { id: "greeting-canvas" });
    this.miniGrid = h("div", { id: "mini-grid" });
    this.countdown = h("div", { id: "countdown" });

    this.header = buildHeader(actions);
    this.views = buildViews(actions, () => this.animateGeometry(false));
    this.viewsEl = h("div", { id: "views" });
    for (const v of this.views.values()) this.viewsEl.append(v.el);
    this.contentEl = h("div", { id: "content" }, this.header.el, this.viewsEl);

    // The drop sequence draws the card, the bar and its own Mochi. It sits under
    // the header, which stays visible on top of it exactly as on macOS.
    this.uploadCanvas = new UploadCanvas({
      ask: () => {
        State.promptContext = State.droppedFile
          ? { kind: "file", name: State.droppedFile.name, path: State.droppedFile.path }
          : null;
        this.setView("prompt");
      },
      cancel: () => this.discardDrop(),
    });

    this.clipEl = h(
      "div",
      { id: "island-clip" },
      this.greetingCanvas,
      this.uploadCanvas.el,
      this.contentEl,
    );
    this.islandEl = h(
      "div",
      { id: "island" },
      this.clipEl,
      this.botGlow,
      this.botCanvas,
      this.miniGrid,
      this.countdown,
    );

    const dpr = Math.min(2, window.devicePixelRatio || 1);
    this.greetingCanvas.width = Math.round(EXPANDED_W * dpr);
    this.greetingCanvas.height = Math.round(150 * dpr);
    this.greetingCanvas.style.width = `${EXPANDED_W}px`;
    this.greetingCanvas.style.height = "150px";

    this.root.append(this.wakeStrip, this.islandEl);
    if (typeof ResizeObserver !== "undefined") {
      const ro = new ResizeObserver(() => {
        if (State.mode !== "expanded") return;
        if (this.resizeRafId != null) return;
        this.resizeRafId = requestAnimationFrame(() => {
          this.resizeRafId = null;
          if (State.mode !== "expanded") return;
          const hh = this.currentHeaderHeight();
          const { w, h } = this.targetSize();
          const headerChanged = Math.abs(hh - this.lastHeaderHeight) >= 1;
          const sizeChanged =
            Math.abs(w - this.lastTargetSize.w) >= 2 ||
            Math.abs(h - this.lastTargetSize.h) >= 2;
          if (headerChanged || sizeChanged) {
            const shrunk = h < this.lastTargetSize.h && w <= this.lastTargetSize.w;
            this.lastHeaderHeight = hh;
            this.animateGeometry(shrunk);
          }
        });
      });
      ro.observe(this.header.el);
      ro.observe(this.viewsEl);
    }
    this.applyGeometry();
  }

  // ── FSM ─────────────────────────────────────────────────────────────────────

  private wireFsm() {
    this.fsm.homeToPetitDelay = State.settings.autoCloseInterval;
    this.fsm.petitToHiddenDelay = Number.POSITIVE_INFINITY;
    this.fsm.onTransition = (from, to) => {
      // The greeting is over, however it ended: back to his desktop spot.
      if (from === "coucou" && to !== "coucou") this.desktop.launch();
      switch (to) {
        case "hidden":
          this.setMode("hidden");
          break;
        case "petit":
          if (from === "coucou") this.greeting.interrupt();
          else if (from === "hidden") Sound.play("peek");
          this.setMode("compact");
          if (from === "coucou") State.view = State.defaultView();
          if (!this.wasInIsland) this.fsm.mouseLeft();
          break;
        case "home":
          this.expand(State.defaultView());
          if (!this.wasInIsland) this.fsm.mouseLeft();
          // Hooks may have been installed in a terminal since: the idle cards
          // say so on the next open, without polling while the island is shut.
          void refreshHookPills();
          break;
        case "coucou":
          this.expand("greeting");
          this.greeting.start();
          break;
      }
      State.notify();
      if (from === "hidden") this.onWake?.();
    };
  }

  launch() {
    this.fsm.launch();
  }

  // ── Mode / view ─────────────────────────────────────────────────────────────

  private syncActiveViewDom() {
    if (State.mode !== "expanded") return;
    this.header?.sync();
    if (this.views) {
      for (const [name, view] of this.views) {
        const on = name === State.view;
        view.el.classList.toggle("on", on);
        if (on) view.sync();
      }
    }
  }

  private setMode(mode: IslandMode) {
    const prev = State.mode;
    if (mode === prev) return;
    State.mode = mode;
    if (mode === "expanded") {
      Sound.play("open");
      this.syncActiveViewDom();
    }
    if (prev === "expanded") {
      Sound.play("close");
      // A folded card is still waiting: it keeps the island pinned.
      if (!State.pendingApproval) State.isPinned = false;
      void Bridge.focusWindow(false);
    }
    if (mode !== "expanded") {
      closePlanCard();
      this.engine.resetMorph();
      // Nothing can be seen of the sequence once the island is shut, and leaving
      // it running would keep the frame loop awake — the island must cost
      // nothing while hidden.
      UploadSeq.deactivate();
    }
    this.updateWindowCollapsed();
    this.animateGeometry(modeOrder(mode) < modeOrder(prev));
    State.notify();
  }

  /** True while the drop sequence owns the island body. */
  private get uploadActive(): boolean {
    return State.mode === "expanded" && UploadSeq.isActive && UPLOAD_VIEWS.has(State.view);
  }

  /** Navigating out of the drop flow ends the sequence, as on macOS. */
  private stopSequenceIfLeaving(view: IslandViewName) {
    if (UploadSeq.isActive && !UPLOAD_VIEWS.has(view)) UploadSeq.deactivate();
  }

  expand(view: IslandViewName) {
    this.stopSequenceIfLeaving(view);
    if (view !== "overview") closePlanCard();
    State.view = view;
    if (State.mode !== "expanded") {
      this.setMode("expanded");
    } else {
      this.syncActiveViewDom();
      this.animateGeometry(false);
    }
    State.lastActivity = performance.now();
    State.notify();
  }

  setView(view: IslandViewName) {
    this.stopSequenceIfLeaving(view);
    if (view !== "overview") closePlanCard();
    if (State.mode !== "expanded") {
      this.fsm.forceHome();
      State.view = view;
      this.syncActiveViewDom();
      this.animateGeometry(false);
      State.notify();
      return;
    }
    const grew = VIEW_LAYOUTS[view].height >= VIEW_LAYOUTS[State.view].height;
    State.view = view;
    State.lastActivity = performance.now();
    this.syncActiveViewDom();
    this.animateGeometry(!grew);
    State.notify();
  }

  collapse() {
    // A waiting card is only ever folded, never dropped by a close.
    if (State.pendingApproval) {
      this.foldApproval();
      return;
    }
    State.isPinned = false;
    this.fsm.pinned = false;
    // Drive the state machine rather than the mode: setting the mode behind its
    // back left it thinking the island was still open, and a click on the compact
    // island then did nothing — the island could never be reopened.
    this.fsm.forcePetit();
  }

  /** Alert from the hook server: open on this view. Pinned alerts never auto-close. */
  alert(view: IslandViewName) {
    this.fsm.pinned = State.isPinned;
    this.fsm.forceHome();
    this.expand(view);
  }

  reveal() {
    this.fsm.reveal();
  }

  /** Right-click on Mochi: wardrobe open ↔ back to the usual view. */
  toggleWardrobe() {
    if (State.paused || State.mode === "hidden") return;
    // The greeting and the drop sequence draw a Mochi of their own.
    if (State.mode === "expanded" && (State.view === "greeting" || this.uploadActive)) return;
    if (State.mode === "expanded" && State.view === "wardrobe") this.setView(State.defaultView());
    else this.setView("wardrobe");
  }

  /**
   * Right-click on the desktop Mochi (macOS openWardrobeFromDesktop): opens the
   * wardrobe from any state, or goes back if it is already open.
   */
  wardrobeFromDesktop() {
    this.wardrobeAnywhere();
  }

  /**
   * The wardrobe from any state — compact or hidden island included — or back
   * to the usual view if it is already open. The desktop Mochi's right-click
   * and the wardrobe shortcut (`open-wardrobe`) both land here.
   */
  wardrobeAnywhere() {
    if (State.mode === "expanded" && State.view === "wardrobe") {
      this.setView(State.defaultView());
      return;
    }
    if (State.paused) return;
    this.alert("wardrobe");
  }

  /** An alert stopped waiting for an answer: let the island auto-close again. */
  dropPin() {
    this.fsm.pinned = false;
    // The countdown the pin held back starts now, if the mouse is elsewhere.
    if (!this.wasInIsland) this.fsm.mouseLeft();
  }

  // ── Keyboard shortcuts (island/shortcuts.ts) ────────────────────────────────

  emote(name: BotEmoteName) {
    this.engine.triggerEmote(name);
    this.ensureRunning();
  }

  /** Ctrl+P: keep the open island from folding away, or let it fold again. */
  setPinned(on: boolean) {
    State.isPinned = on;
    this.fsm.pinned = on;
    if (on) {
      // The countdown bar reads the state machine's deadline, cleared with it.
      this.fsm.cancelTimers();
    } else if (!this.wasInIsland && this.fsm.state === "home") {
      this.fsm.mouseLeft();
    }
    State.notify();
  }

  /** The island takes the keyboard, so its own shortcuts work (Mac: makeKey).
   *  It gives it back when it closes, or when the chat is left. */
  takeKeyboard() {
    void Bridge.focusWindow(true);
  }

  // ── File drop ───────────────────────────────────────────────────────────────

  private onDragDrop(e: { type: string; paths?: string[] }) {
    if (e.type !== "over") void Bridge.log(`drag ${e.type} ${e.paths?.length ?? 0} file(s)`);
    if (State.paused) return;
    switch (e.type) {
      case "enter":
      case "over": {
        if (State.fileDragOver) return;
        State.fileDragOver = true;
        this.engine.animateMorph(1);
        // enterZone must run before the island expands, so the sequence is
        // already active by the time the view becomes `upload`.
        UploadSeq.enterZone(State.mouseInIsland.x, State.mouseInIsland.y);
        this.alert("upload");
        break;
      }
      case "leave": {
        if (!State.fileDragOver) return;
        State.fileDragOver = false;
        this.engine.animateMorph(0);
        // The island deliberately stays open: the drag session is still alive.
        UploadSeq.exitZone();
        State.notify();
        break;
      }
      case "drop": {
        State.fileDragOver = false;
        const path = e.paths?.[0];
        if (!path) {
          this.engine.animateMorph(0);
          this.setView(State.defaultView());
          return;
        }
        this.swallow(path);
        break;
      }
    }
  }

  /**
   * Mochi eats the file. Nothing here waits on the file system: the copy into
   * the inbox runs in the background and swaps the path in when it lands, so a
   * slow disk can never stall the animation — same as FileDropHandler on macOS.
   */
  private swallow(path: string) {
    const name = path.split(/[\\/]/).pop() || "file";
    State.droppedFile = { name, path };
    State.promptContext = { kind: "file", name, path };
    State.chatHistory = [];
    void Bridge.chatReset();

    UploadSeq.performDrop(State.uploadDuration);
    this.uploadTens = 0;
    this.uploadDone = false;

    this.engine.gulp();
    Sound.play("approve");
    this.engine.triggerEmote("happy");
    this.engine.animateMorph(0);

    State.uploadProgress = 0;
    this.setView("uploading");
    this.ensureRunning();

    void Bridge.ingestFile(path)
      .then((file) => {
        // Cancelled (or replaced by another drop) while the copy was running.
        if (State.droppedFile?.path !== path) return;
        State.droppedFile = { name: file.name, path: file.path };
        State.promptContext = { kind: "file", name: file.name, path: file.path };
        State.notify();
      })
      .catch((err) => {
        if (State.droppedFile?.path !== path) return;
        UploadSeq.deactivate();
        State.noteMessage = String(err).replace(/^Error:\s*/, "");
        this.engine.animateMorph(0);
        this.setView("note");
        Sound.play("error");
        window.setTimeout(() => this.setView(State.defaultView()), 2400);
      });
  }

  /** "Cancel" on the dropped file: forget it, so the chat does not pick it up. */
  private discardDrop() {
    State.droppedFile = null;
    State.promptContext = null;
    this.setView(State.defaultView());
  }

  /**
   * Sounds and view changes hung off the canvas timeline: a `tick` every 10 %,
   * the ✓ chime when the bar completes, then `choose` once Mochi has grown back.
   */
  private stepSequence() {
    const since = UploadSeq.sinceDrop();
    if (since == null) return;
    const dur = State.uploadDuration;
    const p = Math.max(0, Math.min(1, (since - PRE_PROGRESS) / dur));

    const tens = Math.floor(p * 10);
    if (tens > this.uploadTens && tens < 10) {
      this.uploadTens = tens;
      Sound.play("tick");
    }

    if (!this.uploadDone && since >= PRE_PROGRESS + dur) {
      this.uploadDone = true;
      Sound.play("approve");
      this.engine.triggerEmote("happy");
    }
    // The extra second is the grow-back, after which the choose card is up.
    if (since >= PRE_PROGRESS + dur + 1 && State.view === "uploading") {
      this.setView("choose");
    }
  }

  // ── Geometry ────────────────────────────────────────────────────────────────

  private lastHeaderHeight = 34;
  private lastMeasuredContentH = 0;
  private lastLeftNaturalH = 0;
  private lastPillsNaturalH = 0;
  private lastIslandPaddingH = 52;
  private lastExpandDurationMs = 240;
  private lastCollapseDurationMs = 200;
  private animStartTimeMs: number | null = null;
  private animWasShrinking = false;

  private currentHeaderHeight(): number {
    if (State.mode !== "expanded" || State.view === "greeting") return 34;
    const measured = this.header?.el?.offsetHeight || 34;
    return Math.max(34, measured);
  }

  private measureOverviewContent(cfg = getIslandSizeConfig(State.settings.islandSize)): {
    extraW: number;
    bodyH: number;
  } {
    const overviewEl = this.views?.get("overview")?.el;
    const minBodyH = cfg.avatarDiameter + 28;
    if (!overviewEl) return { extraW: 0, bodyH: minBodyH };

    const badges = this.header?.el?.querySelectorAll<HTMLElement>(".phase3-bar > .p3-pill");
    const badgeCount = badges ? badges.length : 0;

    const leftBodyEl = overviewEl.querySelector<HTMLElement>(".left-body");
    const leftCardEl = overviewEl.querySelector<HTMLElement>(".left-body > *");
    let leftNaturalH = 0;
    if (leftBodyEl && leftCardEl) {
      const prevBodyH = leftBodyEl.style.height;
      const prevCardFlex = leftCardEl.style.flex;
      const prevCardH = leftCardEl.style.height;
      leftBodyEl.style.height = "auto";
      leftCardEl.style.flex = "0 0 auto";
      leftCardEl.style.height = "auto";
      leftNaturalH = leftCardEl.offsetHeight;
      leftCardEl.style.height = prevCardH;
      leftCardEl.style.flex = prevCardFlex;
      leftBodyEl.style.height = prevBodyH;
    }

    const pillCount = State.otherTasks.length;
    const pillRows = Math.ceil(pillCount / 2);
    const pillsNeededH =
      pillRows > 0 ? pillRows * cfg.pillHeightPx + Math.max(0, pillRows - 1) * 6 + 28 : 0;

    const rawBodyH = Math.max(minBodyH, leftNaturalH, pillsNeededH);
    const bodyH = Math.min(cfg.maxCardContentHeight, rawBodyH);
    const extraW = badgeCount >= 6 || pillCount >= 6 ? 60 : 0;
    this.lastLeftNaturalH = leftNaturalH;
    this.lastPillsNaturalH = pillsNeededH;
    this.lastMeasuredContentH = bodyH;
    return { extraW, bodyH };
  }

  private targetSize(): { w: number; h: number; r: number } {
    const cfg = getIslandSizeConfig(State.settings.islandSize);
    let { w, h } = islandSize(
      State.mode,
      State.view,
      State.chatHistory.length,
      State.settings.islandSize,
    );
    if (State.mode === "expanded") {
      const screenW = Math.max(1024, State.screenSize.width || window.screen.width || 1920);
      const screenH = Math.max(640, State.screenSize.height || window.screen.height || 1080);
      const maxW = Math.floor(screenW * cfg.maxWidthPercent);
      const maxH = Math.floor(screenH * cfg.maxHeightPercent);

      // Compute final expanded width first and lock #content to it so header
      // badge wrapping and left-card text wrapping are measured at the real
      // expanded width immediately, with zero mid-animation reflows.
      if (State.view === "overview") {
        const badges = this.header?.el?.querySelectorAll<HTMLElement>(".phase3-bar > .p3-pill");
        const badgeCount = badges ? badges.length : 0;
        const pillCount = State.otherTasks.length;
        const extraW = badgeCount >= 6 || pillCount >= 6 ? 60 : 0;
        w = Math.max(w, cfg.minExpandedWidth + extraW);
      }
      w = Math.min(maxW, w);
      if (this.contentEl) {
        this.contentEl.style.width = `${w}px`;
      }

      const hh = this.currentHeaderHeight();
      const extraHeader = State.view !== "greeting" ? Math.max(0, hh - 34) : 0;
      const islandPadding = hh + 18;
      this.lastIslandPaddingH = islandPadding;

      if (State.view === "question" && State.pendingApproval?.questions) {
        h = Math.max(h, QUESTION_PICKER_H);
      }
      if (State.view === "overview") {
        const { bodyH } = this.measureOverviewContent(cfg);
        h = bodyH + islandPadding;
      } else if (State.view === "settings") {
        const settingsStack = this.views?.get("settings")?.el?.querySelector<HTMLElement>(".stack");
        const stackH = settingsStack ? settingsStack.scrollHeight + 20 : 168;
        this.lastMeasuredContentH = stackH;
        h = Math.max(h + extraHeader, stackH + islandPadding);
      } else if (State.view !== "greeting") {
        this.lastMeasuredContentH = Math.max(0, h - 52);
        h += extraHeader;
      }

      h = Math.min(maxH, h);
      if (this.contentEl) {
        this.contentEl.style.height = `${h}px`;
      }
    } else if (State.mode === "compact") {
      this.lastMeasuredContentH = cfg.compactNotchHeight;
      this.lastIslandPaddingH = 0;
    }
    const r = State.mode === "expanded" ? EXPANDED_CORNER : ROUNDED_CORNER;
    return { w, h, r };
  }

  private scheduleWindowResize(targetW: number, targetH: number, collapseMs = 200) {
    const desiredW = Math.max(NOTCH_W, Math.ceil(targetW));
    const desiredH = Math.max(32, Math.ceil(targetH));

    if (
      Math.abs(desiredW - this.windowSize.w) < 1 &&
      Math.abs(desiredH - this.windowSize.h) < 1
    ) {
      this.pushLayoutMetrics(targetW, targetH, desiredW, desiredH);
      return;
    }

    const growing = desiredW > this.windowSize.w || desiredH > this.windowSize.h;
    if (this.windowResizeTimer != null) {
      window.clearTimeout(this.windowResizeTimer);
      this.windowResizeTimer = null;
    }
    if (growing) {
      this.windowSize = { w: desiredW, h: desiredH };
      void Bridge.setWindowSize(desiredW, desiredH);
      this.pushLayoutMetrics(targetW, targetH, desiredW, desiredH);
    } else {
      this.windowResizeTimer = window.setTimeout(() => {
        this.windowResizeTimer = null;
        this.windowSize = { w: desiredW, h: desiredH };
        void Bridge.setWindowSize(desiredW, desiredH);
        this.pushLayoutMetrics(targetW, targetH, desiredW, desiredH);
      }, collapseMs + 10);
    }
  }

  private pushLayoutMetrics(
    islandW: number,
    islandH: number,
    nativeW: number,
    nativeH: number,
  ) {
    if (typeof window === "undefined" || typeof document === "undefined") return;
    const htmlBg = window.getComputedStyle(document.documentElement).backgroundColor;
    const bodyBg = window.getComputedStyle(document.body).backgroundColor;
    const isTransparent = (bg: string) =>
      !bg || bg === "transparent" || bg === "rgba(0, 0, 0, 0)";
    const islandOp = parseFloat(window.getComputedStyle(this.islandEl).opacity || "1");
    const botOp = parseFloat(this.botCanvas.style.opacity || "1");
    void Bridge.reportLayoutMetrics({
      mode: State.mode,
      view: State.view,
      sizePreset: State.settings.islandSize,
      measuredContentHeight: this.lastMeasuredContentH,
      leftPanelNaturalHeight: this.lastLeftNaturalH,
      rightPillsNaturalHeight: this.lastPillsNaturalH,
      headerHeight: this.currentHeaderHeight(),
      islandPaddingHeight: this.lastIslandPaddingH,
      islandWidth: Math.round(islandW),
      islandHeight: Math.round(islandH),
      nativeWindowWidth: Math.round(nativeW),
      nativeWindowHeight: Math.round(nativeH),
      windowBackgroundTransparent: isTransparent(htmlBg) && isTransparent(bodyBg),
      islandOpacity: islandOp,
      botCanvasOpacity: botOp,
      botCenterY: Math.round(this.botCy.target),
      expandDurationMs: this.lastExpandDurationMs,
      collapseDurationMs: this.lastCollapseDurationMs,
      updatedAt: Date.now(),
    });
  }

  private animateGeometry(shrinking: boolean) {
    const cfg = getIslandSizeConfig(State.settings.islandSize);
    const { w, h, r } = this.targetSize();
    this.lastTargetSize = { w, h };
    this.animStartTimeMs = performance.now();
    this.animWasShrinking = shrinking;
    this.scheduleWindowResize(w, h, cfg.collapseMs);
    if (shrinking) {
      this.width.curveTowards(w, cfg.collapseMs);
      this.height.curveTowards(h, cfg.collapseMs);
      this.radius.curveTowards(r, cfg.collapseMs);
    } else {
      this.width.springTo(w, cfg.expandMs);
      this.height.springTo(h, cfg.expandMs);
      this.radius.springTo(r, cfg.expandMs);
    }
    this.ensureRunning();
  }

  private applyGeometry() {
    const w = this.width.value;
    const hh = this.height.value;
    const r = this.radius.value;
    this.islandEl.style.width = `${w}px`;
    this.islandEl.style.height = `${hh}px`;
    this.islandEl.style.borderRadius = `0 0 ${r}px ${r}px`;
    this.islandEl.style.transform = `translateX(-50%)`;
    // These follow the island as it resizes, so they belong here rather than in
    // the state-driven DOM sync.
    this.miniGrid.style.left = `${w - 40 - 14.5}px`;
    this.miniGrid.style.top = `${hh / 2 - 14.5}px`;
    this.greetingCanvas.style.left = `${(w - EXPANDED_W) / 2}px`;
    this.uploadCanvas.el.style.left = `${(w - EXPANDED_W) / 2}px`;

    const viewportW = window.innerWidth || this.windowSize.w || PANEL_W;
    const rect = { x: Math.max(0, (viewportW - w) / 2), y: 0, w, h: hh };
    const p = this.pushedRect;
    if (Math.abs(p.x - rect.x) > 0.5 || Math.abs(p.w - rect.w) > 0.5 || Math.abs(p.h - rect.h) > 0.5) {
      this.pushedRect = rect;
      void Bridge.setIslandRect(rect.x, rect.y, rect.w, rect.h);
    }
  }

  /** Island rect in window coordinates (origin top-left of the native window). */
  private islandRect(): { x: number; y: number; w: number; h: number } {
    const w = this.width.value;
    const hh = this.height.value;
    const viewportW = window.innerWidth || this.windowSize.w || PANEL_W;
    return { x: Math.max(0, (viewportW - w) / 2), y: 0, w, h: hh };
  }

  // ── Window collapse (hidden → tiny wake strip, zero polling) ────────────────

  private updateWindowCollapsed() {
    if (this.collapseTimer != null) {
      window.clearTimeout(this.collapseTimer);
      this.collapseTimer = null;
    }
    if (State.mode === "hidden") {
      // Let the island finish retracting, then drop the window to the wake strip:
      // from there the OS delivers no cursor events, so nothing polls at all.
      this.collapseTimer = window.setTimeout(() => {
        this.collapseTimer = null;
        if (State.mode !== "hidden") return;
        this.collapsed = true;
        void Bridge.setCollapsed(true);
      }, 420);
    } else if (this.collapsed) {
      // Grow the window back before the island animates open.
      this.collapsed = false;
      void Bridge.setCollapsed(false);
    }
  }

  // ── Input ───────────────────────────────────────────────────────────────────

  private wireInput() {
    // The wake strip is the only thing the OS can hit while the island is hidden.
    this.wakeStrip.addEventListener("mouseenter", () => {
      Sound.resume();
      if (State.mode === "hidden") this.fsm.mouseEntered();
    });

    this.islandEl.addEventListener("mousedown", (e) => {
      Sound.resume();
      State.lastActivity = performance.now();
      // A press on Mochi may become a drag out to the desktop.
      if (e.button === 0 && this.isBotHit(e.clientX, e.clientY)) {
        this.botPress = { x: e.clientX, y: e.clientY };
      }
      // Right-click on Mochi opens the wardrobe, and closes it again.
      if (e.button === 2 && this.isBotHit(e.clientX, e.clientY)) {
        this.cancelBotHover();
        this.toggleWardrobe();
        return;
      }
      if (State.mode !== "expanded") {
        this.fsm.click();
        return;
      }
      if (this.isBotHit(e.clientX, e.clientY)) {
        this.cancelBotHover();
        this.engine.slap();
      }
    });

    // No browser menu over Mochi: his right-click is the wardrobe. Everywhere
    // else (the chat field) the webview keeps its own menu.
    this.islandEl.addEventListener("contextmenu", (e) => {
      if (this.isBotHit(e.clientX, e.clientY)) e.preventDefault();
    });

    // Dragging Mochi out of the island puts him on the desktop.
    window.addEventListener("mousemove", (e) => {
      if (this.desktop.carrying) {
        this.desktop.carry(e.clientX, e.clientY);
        return;
      }
      const press = this.botPress;
      if (!press) return;
      if (!(e.buttons & 1)) {
        this.botPress = null;
        return;
      }
      if (Math.hypot(e.clientX - press.x, e.clientY - press.y) <= DRAG_THRESHOLD) return;
      this.botPress = null;
      if (!this.canDragOut()) return;
      this.cancelBotHover();
      this.desktop.pickUp(e.clientX, e.clientY);
    });
    window.addEventListener("mouseup", (e) => {
      this.botPress = null;
      if (this.desktop.carrying) this.desktop.carryEnd(e.clientX, e.clientY);
    });

    // Only keys typed into the island itself land here, never Escape typed in
    // a terminal — so it may fold a waiting card away, as Escape in the notch
    // does on macOS.
    window.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && State.mode === "expanded") {
        if (State.pendingApproval) this.foldApproval();
        else if (!State.isPinned) this.collapse();
      }
      State.lastActivity = performance.now();
    });

    void onDragDrop((e) => this.onDragDrop(e));

    // Outside Tauri (plain browser) drive the cursor from DOM events so the
    // island can be inspected with `npm run dev`.
    if (!IS_TAURI) this.followPageCursor();
  }

  /**
   * Takes the cursor from the page's own mouse events instead of Rust's poll.
   * Used where the OS has no global cursor position (Wayland): the events only
   * fire while the pointer is over the island, so leaving the window is
   * reported as a cursor far away, which is what the poll would have said.
   */
  followPageCursor() {
    window.addEventListener("mousemove", (e) => this.onCursor(e.clientX, e.clientY));
    window.addEventListener("mouseout", (e) => {
      if (e.relatedTarget == null) this.onCursor(-10_000, -10_000);
    });
  }

  /**
   * Pointer on/off the island as the compositor sees it (Linux only). Null
   * until the first report, so a platform that never sends it is not gated.
   */
  private pointerInside: boolean | null = null;

  setPointerInside(inside: boolean) {
    this.pointerInside = inside;
  }

  /** Cursor in window-logical coordinates. */
  onCursor(x: number, y: number) {
    // WebKitGTK can deliver a mousemove after the pointer has left the layer
    // surface; trusting it re-enters the island and the auto-close never runs.
    if (this.pointerInside === false) {
      x = -10_000;
      y = -10_000;
    }
    State.mouse = { x, y };
    const rect = this.islandRect();
    State.mouseInIsland = { x: x - rect.x, y: y - rect.y };

    // Windows sends no cursor position with an OLE drag, so the drop sequence is
    // fed from the Win32 cursor poll instead — it runs throughout the drag.
    if (UploadSeq.isActive && !UploadSeq.dropped) {
      UploadSeq.updateCursor(State.mouseInIsland.x, State.mouseInIsland.y);
    }

    const inIsland =
      x >= rect.x - HIT_MARGIN && x <= rect.x + rect.w + HIT_MARGIN &&
      y >= rect.y - HIT_MARGIN && y <= rect.y + rect.h + HIT_MARGIN;

    if (inIsland && !this.wasInIsland) {
      if (this.fsm.state === "coucou") this.greeting.hover();
      this.fsm.mouseEntered();
    }
    if (!inIsland && this.wasInIsland) {
      this.fsm.mouseLeft();
    }
    this.wasInIsland = inIsland;

    // Bot hover → love
    const overBot = State.mode === "expanded" && State.stateOverride == null && this.isBotHit(x, y);
    if (overBot && !this.botHovering) this.botHoverIn(x, y);
    if (!overBot && this.botHovering) this.cancelBotHover();
    this.botHovering = overBot;
    if (this.botHovering) {
      const d = Math.hypot(x - this.botHoverStart.x, y - this.botHoverStart.y);
      if (d > 40) {
        this.botHoverStart = { x, y };
        this.scheduleLove();
      }
    }

    this.ensureRunning();
  }

  /** The greeting and the drop sequence draw a Mochi of their own: not that one. */
  private canDragOut(): boolean {
    if (State.mode === "hidden" || !this.desktop.canPickUp()) return false;
    return !(State.mode === "expanded" && (State.view === "greeting" || this.uploadActive));
  }

  private isBotHit(x: number, y: number): boolean {
    // Out on the desktop, the island's Mochi is invisible: nothing to hit.
    if (State.mochiOnDesktop) return false;
    const rect = this.islandRect();
    const cx = rect.x + this.botCx.value;
    const cy = rect.y + this.botCy.value;
    const radius = this.botSize.value / 2;
    return (x - cx) ** 2 + (y - cy) ** 2 <= radius * radius;
  }

  private botHoverIn(x: number, y: number) {
    if (performance.now() / 1000 - this.lastLoveTime < 6) return;
    this.botHoverStart = { x, y };
    this.engine.blink();
    this.engine.tgEs = 1.08;
    Sound.play("hover");
    this.scheduleLove();
  }

  private scheduleLove() {
    if (this.botHoverTimer != null) window.clearTimeout(this.botHoverTimer);
    this.botHoverTimer = window.setTimeout(() => {
      this.botHoverTimer = null;
      if (!this.botHovering || State.stateOverride != null) return;
      if (performance.now() / 1000 - this.lastLoveTime < 6) return;
      this.lastLoveTime = performance.now() / 1000;
      this.engine.triggerEmote("love");
      Sound.play("love");
    }, 1900);
  }

  private cancelBotHover() {
    if (this.botHoverTimer != null) window.clearTimeout(this.botHoverTimer);
    this.botHoverTimer = null;
    this.engine.tgEs = 1;
  }

  /** Three slaps → dizzy + confused view for 3.3 s, then back. */
  handleDizzy() {
    this.prevViewBeforeConfused = State.view;
    State.stateOverride = "dizzy";
    this.engine.setState("dizzy");
    Sound.play("dizzy");
    this.alert("confused");
    if (this.confusedRecovery != null) window.clearTimeout(this.confusedRecovery);
    this.confusedRecovery = window.setTimeout(() => {
      this.confusedRecovery = null;
      State.stateOverride = null;
      this.engine.setState(State.effectiveState);
      if (State.view === "confused") {
        const fallback = State.defaultView();
        this.setView(this.prevViewBeforeConfused === "confused" ? fallback : this.prevViewBeforeConfused);
      }
      this.engine.triggerEmote("happy");
    }, 3300);
  }

  // ── Frame loop ──────────────────────────────────────────────────────────────

  ensureRunning() {
    if (this.running) return;
    this.running = true;
    this.lastFrame = performance.now();
    requestAnimationFrame(this.frame);
  }

  private frame = (nowMs: number) => {
    const dt = Math.min(0.05, (nowMs - this.lastFrame) / 1000);
    this.lastFrame = nowMs;

    this.width.step(dt, nowMs);
    this.height.step(dt, nowMs);
    this.radius.step(dt, nowMs);
    this.applyGeometry();
    if (this.animStartTimeMs != null && !this.width.animating && !this.height.animating) {
      const elapsed = Math.max(1, Math.round(nowMs - this.animStartTimeMs));
      this.animStartTimeMs = null;
      if (this.animWasShrinking) {
        this.lastCollapseDurationMs = elapsed;
      } else {
        this.lastExpandDurationMs = elapsed;
      }
      this.pushLayoutMetrics(
        this.lastTargetSize.w,
        this.lastTargetSize.h,
        this.windowSize.w,
        this.windowSize.h,
      );
    }

    if (this.dirty) {
      this.dirty = false;
      this.syncDom();
    }

    this.updateBotTargets();
    this.botCx.step(dt);
    this.botCy.step(dt);
    this.botSize.step(dt);

    const greetingActive = State.mode === "expanded" && State.view === "greeting";
    if (greetingActive) {
      const gctx = this.greetingCanvas.getContext("2d");
      if (gctx) {
        const dpr = Math.min(2, window.devicePixelRatio || 1);
        gctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        this.greeting.draw(gctx);
      }
    } else {
      // Kept running even while the drop canvas is up, so the island's own Mochi
      // is already in the right place the moment the canvas fades out.
      this.drawBot(dt);
    }

    const uploadActive = this.uploadActive;
    if (uploadActive) this.uploadCanvas.draw(UploadSeq.frame(), nowMs / 1000);
    this.uploadCanvas.el.classList.toggle("on", uploadActive);
    this.viewsEl.classList.toggle("hidden-by-upload", uploadActive);

    tickMiniBots(dt);
    // A ticker scroll that loses its frames freezes mid-way, rows overlapping.
    const viewAnimating = this.views.get(State.view)?.tick?.(nowMs) === true;
    if (UploadSeq.isActive) this.stepSequence();
    this.updateCountdown(nowMs);

    // Nothing is drawn while the island is hidden, so nothing may keep the loop
    // alive either. This used to read `... || this.engine.busy || State.mode !==
    // "hidden"`, and engine.busy is permanently true for any state with a
    // looping animation — breathing, ratelimit sweat, sleeping z's, the search
    // sweep — so a hidden island went on burning frames in exactly the states it
    // spends most of its life in. Geometry still has to finish retracting.
    const settling =
      this.width.animating || this.height.animating || this.radius.animating;
    const busy = State.mode === "hidden"
      ? settling
      : settling ||
        !this.botCx.settled || !this.botCy.settled || !this.botSize.settled ||
        greetingActive || this.engine.busy || UploadSeq.isActive || viewAnimating;

    if (busy) {
      requestAnimationFrame(this.frame);
    } else {
      this.running = false;
      Sound.idle();
    }
  };

  private updateBotTargets() {
    const p = botPosition(
      State.mode,
      State.view,
      this.height.value,
      State.uploadProgress,
      this.currentHeaderHeight(),
      State.settings.islandSize,
    );
    this.botCx.target = p.cx;
    this.botCy.target = p.cy;
    this.botSize.target = p.diameter / 0.6;

    const greetingActive = State.mode === "expanded" && State.view === "greeting";
    // The drop canvas draws its own Mochi; two of them would overlap. Out on the
    // desktop, he isn't here at all.
    const away = State.mochiOnDesktop;
    const visible = p.opacity > 0 && !greetingActive && !this.uploadActive && !away;
    this.botCanvas.style.opacity = visible ? "1" : "0";

    if (State.mode === "expanded" && State.view !== "uploading" && !greetingActive && !this.uploadActive && !away) {
      const d = p.diameter;
      const color = botGlowColor(State.effectiveState);
      this.botGlow.style.display = "block";
      this.botGlow.style.width = `${d * 2.2}px`;
      this.botGlow.style.height = `${d * 2.2}px`;
      this.botGlow.style.left = `${this.botCx.value - d * 1.1}px`;
      this.botGlow.style.top = `${this.botCy.value - d * 1.1}px`;
      this.botGlow.style.background = `radial-gradient(circle, ${color} 0%, transparent 62%)`;
      this.botGlow.style.opacity = String(botGlowOpacity(State.effectiveState));
    } else {
      this.botGlow.style.display = "none";
    }
  }

  private drawBot(dt: number) {
    const size = this.botSize.value;
    const w = Math.max(1, size);
    const hCss = w + BOT_OVERHANG;
    const wCss = w + BOT_SIDE * 2;
    const canvasW = 180;
    const canvasH = 180;
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    if (this.canvasPx !== dpr) {
      this.canvasPx = dpr;
      this.botCanvas.width = Math.round(canvasW * dpr);
      this.botCanvas.height = Math.round(canvasH * dpr);
      this.botCanvas.style.width = `${canvasW}px`;
      this.botCanvas.style.height = `${canvasH}px`;
    }
    const leftOffset = (canvasW - wCss) / 2;
    const topOffset = (canvasH - hCss) / 2;
    this.botCanvas.style.left = `${this.botCx.value - wCss / 2 - leftOffset}px`;
    this.botCanvas.style.top = `${this.botCy.value - BOT_OVERHANG / 2 - hCss / 2 - topOffset}px`;

    const ctx = this.botCanvas.getContext("2d");
    if (!ctx) return;

    const focus = State.focusTask;
    // While a plan card is open Mochi wears the plan's colour, like its pill.
    // All other pills keep bodyColor = null so Mochi always renders with his
    // full 3D gradient, specular highlight, and bright state tint.
    this.engine.bodyColor = planCardOpen() ? hexToRGB(openPlanColor()) : null;
    this.engine.particleOverhang = BOT_OVERHANG;
    this.engine.lookX = this.lookX();
    this.engine.lookY = this.lookY();
    if (this.engine.morph > 0.3) {
      this.engine.slotHTarget = State.fileDragOver ? 0.2 : 0;
    } else {
      this.engine.slotHTarget = 0;
      if (this.engine.morph < 0.05) {
        this.engine.slotH = 0;
        this.engine.slotHVel = 0;
      }
    }
    // Only the main Mochi is dressed — the one of the main tool's pill (Settings →
    // Active pills) or the active desktop window: a focused service integration
    // pill shows its own colours, unless the wardrobe is open (BotCanvasView.showOutfit, macOS).
    // In the wardrobe the hovered outfit swaps in at once, without the drop-in.
    const inWardrobe = State.mode === "expanded" && State.view === "wardrobe";
    const mainFocused = State.focusId == null || State.focusId === State.mainPillId || !focus?.isIntegration;
    const showOutfit = mainFocused || State.mode !== "expanded" || inWardrobe;
    const outfit = State.wardrobePreview ?? this.seasons.get(parseOutfit(State.settings.mochiOutfit));
    this.engine.setOutfit(showOutfit ? outfit : "none", !inWardrobe);

    this.engine.update(dt);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.globalAlpha = 1.0;
    ctx.globalCompositeOperation = "source-over";
    ctx.clearRect(0, 0, canvasW, canvasH);
    ctx.save();
    ctx.translate(leftOffset + BOT_SIDE, topOffset);
    this.engine.applyDance(ctx, w, hCss);
    this.engine.draw(ctx, w, hCss);
    ctx.restore();
  }

  /** BotCanvasView.lookX / lookY — tanh of the distance to the bot. */
  private lookX(): number {
    const rect = this.islandRect();
    const botScreenX = rect.x + this.botCx.value;
    return Math.tanh((State.mouse.x - botScreenX) / 260);
  }

  private lookY(): number {
    return -Math.tanh((State.mouse.y - this.botCy.value) / 200);
  }

  private updateCountdown(nowMs: number) {
    // The state machine's own deadline, so the bar follows an auto-close delay
    // edited while the countdown runs.
    const dueAt = this.fsm.homeCollapseDueAt;
    if (State.mode !== "expanded" || State.isPinned || dueAt == null) {
      this.countdown.style.width = "0px";
      return;
    }
    const autoClose = this.fsm.homeToPetitDelay;
    const windowS = Math.min(10, autoClose * 0.6);
    const remaining = (dueAt - nowMs) / 1000;
    this.countdown.style.width =
      remaining < windowS ? `${Math.max(0, clamp(remaining / windowS, 0, 1) * 160)}px` : "0px";
  }

  // ── DOM sync ────────────────────────────────────────────────────────────────

  private syncDom() {
    const expanded = State.mode === "expanded";
    const greetingActive = expanded && State.view === "greeting";

    const live = expanded && !greetingActive;
    this.contentEl.style.opacity = live ? "1" : "0";
    // While the drop sequence owns the body its buttons are painted on the canvas
    // underneath, so only the header may keep taking clicks up here.
    this.contentEl.style.pointerEvents = live && !this.uploadActive ? "auto" : "none";
    this.header.el.style.pointerEvents = live ? "auto" : "none";
    this.greetingCanvas.style.display = greetingActive ? "block" : "none";

    // Leaving the greeting, however it ends, lets its sound fade out.
    if (this.greetingShown && !greetingActive) this.greeting.leave();
    this.greetingShown = greetingActive;
    // A wardrobe try-on never outlives the wardrobe.
    if (State.wardrobePreview && !(expanded && State.view === "wardrobe")) State.wardrobePreview = null;

    this.header.sync();
    for (const [name, view] of this.views) {
      const on = name === State.view;
      view.el.classList.toggle("on", on);
      if (on) view.sync();
    }

    if (expanded) {
      const hh = this.currentHeaderHeight();
      const { w, h } = this.targetSize();
      const headerChanged = Math.abs(hh - this.lastHeaderHeight) >= 1;
      const sizeChanged =
        Math.abs(w - this.lastTargetSize.w) >= 2 ||
        Math.abs(h - this.lastTargetSize.h) >= 2;
      if (headerChanged || sizeChanged) {
        const shrunk = h < this.lastTargetSize.h && w <= this.lastTargetSize.w;
        this.lastHeaderHeight = hh;
        this.animateGeometry(shrunk);
      }
    }

    // The chat is the only view with a text field, so it is the only time the
    // island is allowed to take keyboard focus.
    if (this.lastSyncedView !== State.view) {
      const wasChat = this.lastSyncedView === "prompt";
      this.lastSyncedView = State.view;
      if (State.view === "prompt") {
        void Bridge.focusWindow(true);
        window.setTimeout(() => this.views.get("prompt")?.focus?.(), 120);
      } else if (wasChat) {
        void Bridge.focusWindow(false);
      }
    }

    // Compact mini grid
    const showGrid = State.mode === "compact";
    this.miniGrid.style.opacity = showGrid ? "1" : "0";
    if (showGrid) {
      const others = State.otherTasks.slice(0, 4);
      const key = others.map((t) => t.id).join("|");
      if (this.miniGrid.dataset.key !== key) {
        this.miniGrid.dataset.key = key;
        this.miniGrid.replaceChildren();
        for (const t of others) {
          this.miniGrid.append(createMiniBot(t, 13));
        }
        pruneMiniBots();
      }
    }

    const activeTasks = [
      ...(State.focusTask ? [State.focusTask] : []),
      ...State.otherTasks,
      ...State.tasks,
    ];
    syncMiniBotStates(activeTasks);
    this.engine.setState(State.effectiveState);
  }

  /** Applies settings coming from Rust at boot. */
  applySettings() {
    Sound.setEnabled(State.settings.soundEnabled);
    Sound.setVolume(State.settings.soundVolume);
    this.fsm.homeToPetitDelay = State.settings.autoCloseInterval;
    applyIslandSizeCssVars(State.settings.islandSize);
    this.lastSizePreset = resolveIslandSizePreset(State.settings.islandSize);
    this.animateGeometry(false);
    State.notify();
  }

  get panelSize() {
    return { w: PANEL_W, h: PANEL_H };
  }

  get chatHeight() {
    return chatPromptHeight(State.chatHistory.length);
  }
}
