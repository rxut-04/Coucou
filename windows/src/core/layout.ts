// Island geometry — ported from IslandTypes.swift + IslandWindowController.islandSize
// + IslandRootView.botPosition. All values are logical pixels, identical to the
// macOS app's points.

export type IslandMode = "hidden" | "compact" | "expanded";

export type IslandViewName =
  | "overview"
  | "empty"
  | "approval"
  | "question"
  | "error"
  | "finished"
  | "confused"
  | "upload"
  | "uploading"
  | "choose"
  | "mail"
  | "prompt"
  | "searching"
  | "result"
  | "note"
  | "settings"
  | "greeting"
  | "recap"
  | "wardrobe";

export type BotStateName =
  | "idle"
  | "working"
  | "thinking"
  | "searching"
  | "approval"
  | "question"
  | "error"
  | "finished"
  | "ratelimit"
  | "sleeping"
  | "dizzy";

export type BotEmoteName = "love" | "surprised" | "proud" | "wink" | "yawn" | "happy" | "annoyed";

export type AgentLayoutMode = "none" | "grid" | "pills" | "column";

export interface ViewLayout {
  height: number;
  botX: number;
  botY: number | null; // null = auto-centred
  botDiameter: number;
  agentMode: AgentLayoutMode;
}

import { getIslandSizeConfig } from "./size-config";

// Default panel dimensions for Comfortable preset; grown dynamically via ResizeObserver.
export const PANEL_W = 1040;
export const PANEL_H = 420;

// No notch on a PC: these are the hidden/compact sizes from docs/SPEC.md.
export const NOTCH_W = 220;
export const NOTCH_H = 44;
export const COMPACT_W = 390;
export const EXPANDED_W = 1040;

export const ROUNDED_CORNER = 14; // hidden / compact
export const EXPANDED_CORNER = 22;

/** Invisible hover strip that wakes the island when hidden. */
export const WAKE_STRIP_W = 240;
export const WAKE_STRIP_H = 6;

export const VIEW_LAYOUTS: Record<IslandViewName, ViewLayout> = {
  overview: { height: 132, botX: 68, botY: null, botDiameter: 56, agentMode: "pills" },
  empty: { height: 160, botX: 68, botY: null, botDiameter: 58, agentMode: "none" },
  approval: { height: 170, botX: 64, botY: null, botDiameter: 56, agentMode: "column" },
  question: { height: 170, botX: 64, botY: null, botDiameter: 56, agentMode: "column" },
  error: { height: 170, botX: 64, botY: null, botDiameter: 58, agentMode: "column" },
  finished: { height: 170, botX: 64, botY: null, botDiameter: 58, agentMode: "column" },
  confused: { height: 170, botX: 74, botY: null, botDiameter: 64, agentMode: "column" },
  upload: { height: 176, botX: 140, botY: 104, botDiameter: 62, agentMode: "column" },
  // botY 103 = bar top (42 + 58) + 3, so the dot really rides the bar. The Swift
  // layout says 118 while its own comment says 103; the comment matches the spec.
  uploading: { height: 176, botX: 46, botY: 103, botDiameter: 20, agentMode: "none" },
  choose: { height: 176, botX: 66, botY: 101, botDiameter: 56, agentMode: "column" },
  mail: { height: 240, botX: 62, botY: null, botDiameter: 50, agentMode: "column" },
  prompt: { height: 204, botX: 58, botY: null, botDiameter: 50, agentMode: "column" },
  searching: { height: 170, botX: 58, botY: null, botDiameter: 50, agentMode: "column" },
  result: { height: 170, botX: 58, botY: null, botDiameter: 50, agentMode: "column" },
  note: { height: 170, botX: 64, botY: null, botDiameter: 54, agentMode: "column" },
  settings: { height: 210, botX: 60, botY: null, botDiameter: 52, agentMode: "none" },
  greeting: { height: 150, botX: 320, botY: 90, botDiameter: 0, agentMode: "none" },
  // Mac: 160. The extra 24 hold the two lines with top agent, project, busiest
  // day, longest session, permissions and questions, which the Mac card leaves
  // to the shared image.
  recap: { height: 196, botX: 68, botY: null, botDiameter: 58, agentMode: "column" },
  wardrobe: { height: 176, botX: 68, botY: null, botDiameter: 58, agentMode: "none" },
};

// The upload views above are only the fallback geometry. Once a file is actually
// dropped the whole sequence — Mochi included — is drawn by src/upload, which
// owns its own constants (USC) straight from UploadSequenceEngine.swift.

/** The question view with options to pick from: room for two rows of them. */
export const QUESTION_PICKER_H = 220;

/** Chat view grows with the conversation — IslandContainer.chatPromptHeight. */
export function chatPromptHeight(messageCount: number): number {
  return Math.min(340, 250 + messageCount * 40);
}

export function islandSize(
  mode: IslandMode,
  view: IslandViewName,
  chatCount = 0,
  sizePreset = "comfortable",
): { w: number; h: number } {
  const cfg = getIslandSizeConfig(sizePreset);
  switch (mode) {
    case "hidden":
      // No notch to hide inside on a PC: the island retracts to zero height and
      // slides into the top edge of the screen instead of sitting there as a bar.
      return { w: NOTCH_W, h: 0 };
    case "compact":
      return { w: cfg.compactNotchWidth, h: cfg.compactNotchHeight };
    case "expanded": {
      if (view === "greeting") {
        return { w: 640, h: VIEW_LAYOUTS.greeting.height };
      }
      const baseH =
        view === "prompt"
          ? chatPromptHeight(chatCount)
          : view === "overview"
            ? cfg.minOverviewHeight
            : Math.max(VIEW_LAYOUTS[view].height, cfg.minOverviewHeight);
      return { w: cfg.minExpandedWidth, h: baseH };
    }
  }
}

export interface BotPlacement {
  cx: number;
  cy: number;
  diameter: number;
  opacity: number;
}

/** IslandRootView.botPosition — cy is measured from the island's top edge. */
export function botPosition(
  mode: IslandMode,
  view: IslandViewName,
  islandH: number,
  uploadProgress = 0,
  headerH = 34,
  sizePreset = "comfortable",
): BotPlacement {
  const cfg = getIslandSizeConfig(sizePreset);
  switch (mode) {
    case "hidden":
      return { cx: 46, cy: 16, diameter: 6, opacity: 0 };
    case "compact": {
      const compactDiameter = Math.round(20 * (cfg.compactNotchHeight / 44));
      return {
        cx: 42,
        cy: Math.round(cfg.compactNotchHeight * 0.62),
        diameter: compactDiameter,
        opacity: 1,
      };
    }
    case "expanded": {
      const layout = VIEW_LAYOUTS[view];
      const extraHeader = Math.max(0, headerH - 34);
      const botX = view === "overview" || view === "empty" || view === "wardrobe" ? cfg.botX : layout.botX;
      const botDiameter =
        view === "overview" || view === "empty" || view === "wardrobe"
          ? cfg.avatarDiameter
          : Math.round(layout.botDiameter * cfg.baseFontScale);
      if (view === "uploading") {
        return {
          cx: 36 + uploadProgress * 526,
          cy: (layout.botY ?? 103) + extraHeader,
          diameter: layout.botDiameter,
          opacity: 1,
        };
      }
      if (layout.botY != null) {
        return { cx: botX, cy: layout.botY + extraHeader, diameter: botDiameter, opacity: 1 };
      }
      // Centre of the card area below the dynamic header (8 pt top inset + headerH),
      // nudged slightly lower (56%) so Mochi + his seasonal hat fit evenly inside a tight card.
      const headerBottom = 8 + Math.max(34, headerH);
      const bodySpace = Math.max(76, islandH - headerBottom - 10);
      const cy = headerBottom + bodySpace * 0.56;
      return { cx: botX, cy, diameter: botDiameter, opacity: 1 };
    }
  }
}

export function botGlowColor(s: BotStateName): string {
  switch (s) {
    case "working":
      return "#3B9EFF";
    case "thinking":
      return "#A78BFA";
    case "searching":
      return "#6366F1";
    case "approval":
      return "#F5A524";
    case "error":
      return "#F4505E";
    case "finished":
      return "#34D399";
    case "ratelimit":
      return "#F59E0B";
    default:
      return "#FFFFFF";
  }
}

export function botGlowOpacity(s: BotStateName): number {
  switch (s) {
    case "idle":
    case "sleeping":
      return 0.15;
    case "dizzy":
      return 0;
    default:
      return 0.65;
  }
}

// Project colours (IslandConst.projectColors)
const PROJECT_COLORS: Record<string, string> = {
  korus: "#FF5A4E",
  "sbe hub": "#2EC4A0",
  "morning ai brief": "#F29B38",
  "publication ig": "#7C5CFF",
  "ig post": "#7C5CFF",
  "louisraille.fr": "#38BDF8",
  louisraille: "#38BDF8",
  "notch buddy": "#EC4899",
  "notch-buddy": "#EC4899",
  notchbuddy: "#EC4899",
};

const FALLBACK_COLORS = ["#22C55E", "#EAB308", "#60A5FA", "#E879F9"];

export function colorForProject(name: string): string {
  const key = name.toLowerCase().trim();
  const exact = PROJECT_COLORS[key];
  if (exact) return exact;
  for (const [k, c] of Object.entries(PROJECT_COLORS)) {
    if (key.startsWith(k) || key.includes(k)) return c;
  }
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = (hash * 31 + name.charCodeAt(i)) | 0;
  return FALLBACK_COLORS[Math.abs(hash) % FALLBACK_COLORS.length];
}

// Card wash colours (CardBackground.washColor)
export type Wash = "red" | "green" | "pink" | "amber" | "cyan" | "indigo" | "soft" | null;

export function washRGBA(wash: Wash): string {
  switch (wash) {
    case "red":
      return "rgba(244,80,94,0.55)";
    case "green":
      return "rgba(52,211,153,0.5)";
    case "pink":
      return "rgba(244,114,182,0.55)";
    case "amber":
      return "rgba(245,165,36,0.42)";
    case "cyan":
      return "rgba(34,211,238,0.38)";
    case "indigo":
      return "rgba(99,102,241,0.5)";
    case "soft":
      return "rgba(255,255,255,0.08)";
    default:
      return "rgba(0,0,0,0)";
  }
}
