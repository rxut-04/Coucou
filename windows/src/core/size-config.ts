// Central sizing configuration for Coucou (Phases D & E).
// Keeps all island & window dimensions, screen percentage caps, font scales,
// avatar sizes, and badge/pill dimensions in one place.

export type IslandSizePreset = "compact" | "comfortable" | "large";

export const DEFAULT_ISLAND_SIZE_PRESET: IslandSizePreset = "comfortable";

export interface IslandSizeConfig {
  preset: IslandSizePreset;
  label: string;
  /** Minimum expanded island width in logical pixels. */
  minExpandedWidth: number;
  /** Maximum fraction of screen width the island/window may grow to (default 0.92). */
  maxWidthPercent: number;
  /** Maximum fraction of screen height the island/window may grow to (default 0.80). */
  maxHeightPercent: number;
  /** Base font multiplier applied to card titles, body text, and pills. */
  baseFontScale: number;
  /** Top status badge height in pixels. */
  badgeHeightPx: number;
  /** Top status badge font size in pixels. */
  badgeFontPx: number;
  /** Mochi avatar diameter in expanded overview/cards. */
  avatarDiameter: number;
  /** Mochi avatar horizontal center (cx) from left edge of island. */
  botX: number;
  /** Left padding on main card body to clear Mochi's avatar. */
  cardLeftPadPx: number;
  /** Right-panel window pill height in pixels. */
  pillHeightPx: number;
  /** Right-panel window pill font size in pixels. */
  pillFontPx: number;
  /** Minimum expanded island height for the overview view. */
  minOverviewHeight: number;
  /** Maximum height of card content before vertical scrolling engages. */
  maxCardContentHeight: number;
  /** Compact (folded) island width. */
  compactNotchWidth: number;
  /** Compact (folded) island height. */
  compactNotchHeight: number;
  /** Expand animation duration in milliseconds (fast ease-out). */
  expandMs: number;
  /** Collapse animation duration in milliseconds (fast ease-out). */
  collapseMs: number;
}

export const ISLAND_SIZE_CONFIGS: Record<IslandSizePreset, IslandSizeConfig> = {
  compact: {
    preset: "compact",
    label: "Compact",
    minExpandedWidth: 920,
    maxWidthPercent: 0.92,
    maxHeightPercent: 0.80,
    baseFontScale: 1.16,
    badgeHeightPx: 26,
    badgeFontPx: 12.5,
    avatarDiameter: 54,
    botX: 64,
    cardLeftPadPx: 112,
    pillHeightPx: 32,
    pillFontPx: 12,
    minOverviewHeight: 142,
    maxCardContentHeight: 460,
    compactNotchWidth: 360,
    compactNotchHeight: 44,
    expandMs: 500,
    collapseMs: 340,
  },
  comfortable: {
    preset: "comfortable",
    label: "Comfortable",
    minExpandedWidth: 1040,
    maxWidthPercent: 0.92,
    maxHeightPercent: 0.80,
    baseFontScale: 1.28,
    badgeHeightPx: 29,
    badgeFontPx: 13.5,
    avatarDiameter: 60,
    botX: 70,
    cardLeftPadPx: 122,
    pillHeightPx: 36,
    pillFontPx: 13,
    minOverviewHeight: 154,
    maxCardContentHeight: 520,
    compactNotchWidth: 390,
    compactNotchHeight: 48,
    expandMs: 500,
    collapseMs: 340,
  },
  large: {
    preset: "large",
    label: "Large",
    minExpandedWidth: 1180,
    maxWidthPercent: 0.94,
    maxHeightPercent: 0.82,
    baseFontScale: 1.40,
    badgeHeightPx: 32,
    badgeFontPx: 14.5,
    avatarDiameter: 64,
    botX: 76,
    cardLeftPadPx: 132,
    pillHeightPx: 40,
    pillFontPx: 14,
    minOverviewHeight: 168,
    maxCardContentHeight: 580,
    compactNotchWidth: 420,
    compactNotchHeight: 52,
    expandMs: 500,
    collapseMs: 340,
  },
};

export const ISLAND_SIZE_PRESETS: readonly IslandSizePreset[] = [
  "compact",
  "comfortable",
  "large",
];

export function resolveIslandSizePreset(raw?: string | null): IslandSizePreset {
  const clean = (raw ?? "").trim().toLowerCase();
  if (clean === "compact" || clean === "comfortable" || clean === "large") {
    return clean;
  }
  return DEFAULT_ISLAND_SIZE_PRESET;
}

export function getIslandSizeConfig(raw?: string | null): IslandSizeConfig {
  return ISLAND_SIZE_CONFIGS[resolveIslandSizePreset(raw)];
}

export function applyIslandSizeCssVars(raw?: string | null): IslandSizeConfig {
  const cfg = getIslandSizeConfig(raw);
  if (typeof document !== "undefined") {
    const root = document.documentElement;
    root.dataset.islandSize = cfg.preset;
    root.style.setProperty("--size-font-scale", String(cfg.baseFontScale));
    root.style.setProperty("--badge-h", `${cfg.badgeHeightPx}px`);
    root.style.setProperty("--badge-font", `${cfg.badgeFontPx}px`);
    root.style.setProperty("--card-left-pad", `${cfg.cardLeftPadPx}px`);
    root.style.setProperty("--pill-h", `${cfg.pillHeightPx}px`);
    root.style.setProperty("--pill-font", `${cfg.pillFontPx}px`);
    root.style.setProperty("--card-max-h", `${cfg.maxCardContentHeight}px`);
  }
  return cfg;
}
