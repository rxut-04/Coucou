# Notification Position & Canvas Opacity Fix

**Date:** 2026-10-09  
**Build:** Coucou v0.2.0

## Issues Fixed

### 1. Notification Speech Bubble Position ✅
**Problem:** Notification appeared ABOVE the pills instead of inside the expanded island content area, near the main Mochi pill.

**Root Cause:** The `#workflow-note` element was a direct child of `#island` container, making it a sibling to both `#bot-canvas` (main Mochi pill) and `#mini-grid` (the pills). This meant it was positioned relative to the entire island, not the content area.

**Solution:**
- **Moved `#workflow-note` inside `#clipEl`** (island.ts line ~330)
  - Now part of the content hierarchy: `#island → #island-clip → #workflow-note`
  - This places it in the same coordinate space as the expanded content
  
- **Updated CSS positioning** (style.css):
  ```css
  #workflow-note {
    position: absolute;
    top: 60px;        /* Below header, in content area */
    left: 100px;      /* Next to main Mochi pill */
    /* Removed: transform: translateY(-50%) */
  }
  ```

- **Fixed speech bubble arrow** to point at Mochi from the left:
  ```css
  #workflow-note::before, ::after {
    top: 20px;  /* Fixed position instead of 50% translateY */
  }
  ```

- **Updated animation** to slide from left instead of from top:
  ```css
  @keyframes wf-note-in {
    from { opacity: 0; transform: translateX(-4px); }
    to { opacity: 1; transform: translateX(0); }
  }
  ```

### 2. Main Pill Dullness Investigation 🔍
**Problem:** After island expand→collapse cycle, the main Mochi pill becomes dull/faded and looks like it's "going outside the window."

**Diagnostic Changes Added:**
1. **Canvas recreation logging** (island.ts `recreateBotCanvas()`):
   - Logs when canvas is recreated
   - Tracks old vs new opacity values
   - Confirms mode transitions

2. **Bot visibility logging** (island.ts `updateBotTargets()`):
   - Logs mode, view, visibility state
   - Tracks opacity target from `botPosition()`
   - Monitors when canvas opacity is set

3. **Explicit opacity management**:
   - Canvas created with explicit `style.opacity = "1"`
   - Canvas recreation now called after notification dismissal

**Expected Behavior (per layout.ts):**
```typescript
case "compact":
  return { cx: 40, cy: 16, diameter: 20, opacity: 1 };
```

## Files Modified

### TypeScript/DOM
- `/src/island/island.ts`:
  - Moved `#workflow-note` creation before `#clipEl`
  - Added it as child of `#clipEl` instead of `#island`
  - Added console.log debugging in `recreateBotCanvas()` and `updateBotTargets()`
  - Explicit opacity: "1" on canvas recreation

### CSS
- `/src/style.css`:
  - Changed `#workflow-note` positioning from centered to fixed top/left
  - Updated speech bubble arrow from centered to fixed position
  - Modified entrance animation from vertical to horizontal slide

## Testing Checklist

With the app running, watch the browser console for logs:

1. **Notification Position:**
   - [ ] Wait for notification (every 2 seconds)
   - [ ] Island should expand down
   - [ ] Speech bubble should appear INSIDE expanded area
   - [ ] Bubble should be to the RIGHT of main Mochi pill (not above pills)
   - [ ] Arrow should point left toward Mochi

2. **Canvas Opacity (check console logs):**
   - [ ] After notification shows: Look for `[Bot Visibility]` logs
   - [ ] When island expands: Check Mode=expanded, Visible=true, Opacity target=1
   - [ ] After island collapses: Check Mode=compact, Visible=true, Opacity target=1
   - [ ] Look for `[Canvas Fix] Recreating bot canvas` when collapsing
   - [ ] Verify "New canvas created with opacity: 1"
   - [ ] Visual check: Main pill should stay vibrant and properly positioned

3. **Multiple Cycles:**
   - [ ] Wait for 3-4 notification cycles
   - [ ] Verify pill stays vibrant after each collapse
   - [ ] Check logs for consistent opacity values
   - [ ] Confirm no "opacity: 0" states in compact mode

## Console Commands for Debugging

Open browser dev tools and watch for:
```
[Bot Visibility] Mode: compact View: overview Visible: true Opacity target: 1
[Canvas Fix] Recreating bot canvas - Mode: compact
[Canvas Fix] Old canvas opacity: 1
[Canvas Fix] New canvas created with opacity: 1
```

## Known Issues

If pill still appears dull after these changes, investigate:
1. **CSS !important rules** in `#bot-canvas` might be interfering
2. **Canvas context state** - check if `ctx.globalAlpha` is being reset properly
3. **Hardware acceleration** - the `transform: translateZ(0)` might cause compositor issues
4. **Timing** - canvas might be drawn before opacity is fully applied

## Next Steps

If pill dullness persists:
1. Check console logs to confirm `recreateBotCanvas()` is actually being called
2. Verify opacity stays at "1" throughout compact mode lifecycle
3. Consider removing CSS `opacity: 1 !important` and rely on JS control
4. Test if issue is visual (rendering) vs logical (opacity value)
5. Compare canvas context flags between working/broken states

## Original Source Reference

- Original macOS source: `/run/media/om-raut/New Volume/COUCOU/coucou_original/`
- Original does NOT have workflow notification system
- Original canvas setup: `getContext("2d")` without `desynchronized` flag (already fixed)
- Original draw sequence: `ctx.save() → applyDance() → draw() → ctx.restore()` (already fixed)
