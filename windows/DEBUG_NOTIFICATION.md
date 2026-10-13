# Notification System Debugging Guide

**Date:** 2026-10-09  
**Issue:** Notification system not working  
**Fix Applied:** Timer values changed to 2 seconds + comprehensive logging added

---

## What Was Wrong

The notification system WAS working, but with **15-minute intervals** instead of 2-second debug intervals!

### Original Values:
```typescript
quotesIntervalSeconds: 900      // 15 minutes
workNotificationSeconds: 1200    // 20 minutes
```

### Fixed Values (DEBUG MODE):
```typescript
quotesIntervalSeconds: 2         // 2 seconds for testing
workNotificationSeconds: 2       // 2 seconds for testing
```

---

## How to Verify It's Working

### 1. Open Browser Dev Tools
Since Coucou uses WebView, you need to:
1. Right-click anywhere on the Coucou island
2. Select **"Inspect Element"** or **"Inspect"**
3. Go to the **Console** tab

### 2. Watch for These Logs

You should see logs every second like this:

```
[workflow] WorkflowEngine initialized
[workflow] focusTrackerWindowInfo returned: {pid: 12345, name: "code", title: "...", ...}
[workflow] Quote check: elapsed=1.2s, threshold=2s, app=vscode
[workflow] Quote check: elapsed=2.1s, threshold=2s, app=vscode
[workflow] Quote timer elapsed, checking active window
[workflow] focusTrackerWindows returned: [{active: true, resourceClass: "code", ...}]
[workflow] Active window: {active: true, resourceClass: "code", ...}
[workflow] Showing motivational quote for code: "Great progress! Keep that momentum going 🚀"
[workflow] Showing notification: {message: "Great progress! Keep that momentum going 🚀"}
```

### 3. Visual Confirmation

**Expected behavior (every 2 seconds):**
1. **Sound plays** (blip sound effect)
2. **Island expands** from compact mode
3. **Speech bubble appears** with motivational quote
4. **After 6 seconds**, notification disappears
5. **Island collapses** back to compact mode

---

## Troubleshooting

### Issue: No Console Logs at All

**Possible causes:**
1. WorkflowEngine not initialized (check for `[workflow] WorkflowEngine initialized`)
2. Side-effect import missing in main.ts

**Solution:**
```bash
# Check if import exists
grep "workflow" "/run/media/om-raut/New Volume/COUCOU/coucou_old/windows/src/main.ts"

# Should show:
# import "./core/workflow";
```

### Issue: Logs Show "Current app is 'other'"

**Meaning:** WorkflowEngine doesn't recognize your active window as productive app

**Debug:**
Look for log: `[workflow] focusTrackerWindowInfo returned: {...}`

If it shows `null` or `{name: "unknown", ...}`, the focus tracker isn't working.

**Solution:**
```bash
# Check if focus tracker is running
ps aux | grep focus

# Check Rust backend logs
tail -f ~/.local/share/coucou/coucou.log | grep focus
```

### Issue: Logs Show "Quotes disabled in settings"

**Meaning:** Quotes are turned off in localStorage

**Solution:**
In browser console, run:
```javascript
localStorage.removeItem('coucou-workflow-settings');
location.reload();
```

Or enable in Settings UI (if available).

### Issue: Timer Never Reaches 2 Seconds

**Look for:** Logs show `elapsed=0.5s` then resets to `elapsed=0.1s`

**Meaning:** App is changing too frequently, timer keeps resetting

**Check:** Look for `[workflow] App changed: X -> Y` logs

**Solution:** Stay in one app (VS Code, Terminal, or Browser) for at least 2 seconds.

### Issue: focusTrackerWindowInfo Returns Null

**Meaning:** Backend can't detect active window (Linux-specific issue)

**Possible causes:**
1. KWin script not running (if on KDE)
2. DBus permissions issue
3. Focus tracker not implemented for your window manager

**Solution:**
```bash
# Check which window manager you're using
echo $XDG_CURRENT_DESKTOP
echo $XDG_SESSION_TYPE

# Check backend logs for focus tracker errors
tail -f ~/.local/share/coucou/coucou.log
```

**Temporary workaround:**
Edit `workflow.ts` to force a specific app type:
```typescript
private detectActiveApp() {
  // DEBUG: Force VS Code for testing
  if (this.currentApp !== "vscode") {
    console.log("[workflow] FORCED: app -> vscode");
    this.currentApp = "vscode";
    this.appStartTime = Date.now();
  }
}
```

---

## Testing Different Scenarios

### Test 1: VS Code Quotes
```bash
# 1. Open VS Code
# 2. Wait 2 seconds
# 3. Check console for: "Showing motivational quote for code"
# Expected quote: One of 15 VS Code motivational quotes
```

### Test 2: Terminal Quotes
```bash
# 1. Open Terminal
# 2. Wait 2 seconds
# 3. Check console for: "Showing motivational quote for konsole" (or your terminal)
# Expected quote: One of 15 Terminal quotes
```

### Test 3: Browser Quotes
```bash
# 1. Open Browser
# 2. Wait 2 seconds
# 3. Check console for: "Showing motivational quote for firefox" (or chrome)
# Expected quote: One of 10 Browser quotes
```

### Test 4: Work Notifications
```bash
# 1. Stay in VS Code for 2+ seconds
# 2. Check console for: "Showing work notification"
# Expected message: "Great focus! You've been coding for X seconds! 🎯"
```

### Test 5: Multiple Cycles
```bash
# 1. Keep Coucou running for 30 seconds
# 2. Should see ~15 notifications (every 2 seconds)
# 3. Verify island expands/collapses each time
# 4. Check main pill stays vibrant (canvas opacity issue test)
```

---

## Key Console Commands

### Check Current Settings
```javascript
// In browser console:
JSON.parse(localStorage.getItem('coucou-workflow-settings'))
```

### Force Reset to Defaults
```javascript
// In browser console:
localStorage.removeItem('coucou-workflow-settings');
location.reload();
```

### Manually Trigger Notification
```javascript
// In browser console (if State is accessible):
State.workflowNotification = {
  message: "Test notification",
  detail: "This is a test"
};
State.notify();
```

### Check if WorkflowEngine Exists
```javascript
// In browser console:
// This won't work directly (module scope), but you can check logs
console.log("Check for: [workflow] WorkflowEngine initialized");
```

---

## Production Settings

When you're done testing and want normal intervals:

**Edit:** `/run/media/om-raut/New Volume/COUCOU/coucou_old/windows/src/core/workflow.ts`

```typescript
export const DEFAULT_WORKFLOW_SETTINGS: WorkflowSettings = {
  quotesEnabled: true,
  quotesIntervalSeconds: 900,    // 15 minutes (was 2 for debug)

  workNotificationEnabled: true,
  workNotificationSeconds: 1200,  // 20 minutes (was 2 for debug)
  
  // ...rest of settings...
};
```

Then rebuild:
```bash
cd "/run/media/om-raut/New Volume/COUCOU/coucou_old/windows"
npm run build
cargo tauri build --bundles deb
sudo dpkg -i "target/release/bundle/deb/Coucou_0.2.0_amd64.deb"
```

---

## Quick Reference: Log Meanings

| Log Message | Meaning | Status |
|-------------|---------|--------|
| `WorkflowEngine initialized` | System started | ✅ Good |
| `Quote check: elapsed=X` | Timer running | ✅ Good |
| `Quotes disabled in settings` | Feature turned off | ⚠️ Enable it |
| `Current app is 'other'` | Non-productive app | ⚠️ Switch to VS Code/Terminal/Browser |
| `focusTrackerWindowInfo returned: null` | Can't detect window | ❌ Focus tracker issue |
| `Quote timer elapsed` | Ready to show notification | ✅ Good |
| `Showing motivational quote` | Notification triggered | ✅ Good |
| `Showing notification:` | Sent to island | ✅ Good |
| `[Bot Visibility]` | Island responding | ✅ Good |
| `[Canvas Fix]` | Canvas being recreated | ✅ Good |

---

## Summary

**Changes made:**
1. ✅ Changed timer from 900s → 2s for quotes
2. ✅ Changed timer from 1200s → 2s for work notifications
3. ✅ Added comprehensive console logging
4. ✅ Added fallback if focusTrackerWindows returns empty
5. ✅ Added error handling with .catch()

**Expected result:**
- Notification every 2 seconds while in VS Code, Terminal, or Browser
- Clear console logs showing exactly what's happening
- Easy to debug if something breaks

**If still not working:**
1. Check console logs (most important!)
2. Verify you're in a productive app (not "other")
3. Check if quotes are enabled in settings
4. Verify focus tracker is working (backend logs)

---

**Built:** 2026-10-09  
**Version:** Coucou 0.2.0 (debug build)
