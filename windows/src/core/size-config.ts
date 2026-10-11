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
}

export const ISLAND_SIZE_CONFIGS: Record<IslandSizePreset, IslandSizeConfig> = {
  compact: {
    preset: "compact",
    label: "Compact",
    minExpandedWidth: 800,
    maxWidthPercent: 0.92,
    maxHeightPercent: 0.80,
    baseFontScale: 0.96,
    badgeHeightPx: 22,
    badgeFontPx: 10.5,
    avatarDiameter: 50,
    botX: 60,
    cardLeftPadPx: 104,
    pillHeightPx: 28,
    pillFontPx: 10.5,
    minOverviewHeight: 124,
    maxCardContentHeight: 420,
    compactNotchWidth: 340,
    compactNotchHeight: 40,
  },
  comfortable: {
    preset: "comfortable",
    label: "Comfortable",
    minExpandedWidth: 960,
    maxWidthPercent: 0.92,
    maxHeightPercent: 0.80,
    baseFontScale: 1.08,
    badgeHeightPx: 25,
    badgeFontPx: 11.5,
    avatarDiameter: 56,
    botX: 68,
    cardLeftPadPx: 116,
    pillHeightPx: 32,
    pillFontPx: 11.5,
    minOverviewHeight: 132,
    maxCardContentHeight: 480,
    compactNotchWidth: 370,
    compactNotchHeight: 44,
  },
  large: {
    preset: "large",
    label: "Large",
    minExpandedWidth: 1120,
    maxWidthPercent: 0.92,
    maxHeightPercent: 0.80,
    baseFontScale: 1.18,
    badgeHeightPx: 28,
    badgeFontPx: 12.5,
    avatarDiameter: 62,
    botX: 74,
    cardLeftPadPx: 128,
    pillHeightPx: 36,
    pillFontPx: 12.5,
    minOverviewHeight: 142,
    maxCardContentHeight: 540,
    compactNotchWidth: 410,
    compactNotchHeight: 48,
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
