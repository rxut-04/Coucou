# Phase D Quick Test Guide

## Install & Start

```bash
# Stop old instance
pkill -x coucou

# Install new build
sudo dpkg -i "/run/media/om-raut/New Volume/COUCOU/coucou/windows/target/release/bundle/deb/Coucou_0.2.0_amd64.deb"

# Start Coucou
/usr/bin/coucou &

# Check logs (in another terminal)
journalctl --user -f | grep -E "WINDOWS|coucou"
```

## Quick Verification Commands

### See all current windows
```bash
busctl --user call com.coucou.FocusTracker /com/coucou/FocusTracker com.coucou.FocusTracker ListWindows | jq -r '.data[0]' | jq .
```

### Monitor window events
```bash
busctl --user monitor com.coucou.FocusTracker
```

### Check script loaded
```bash
busctl --user call org.kde.KWin /Scripting org.kde.kwin.Scripting isScriptLoaded s coucou-focus
```

## Key Tests

### ✅ Test 1: Two VS Code Windows
1. Open VS Code window #1
2. Open VS Code window #2  
**PASS:** Both appear as separate pills

### ✅ Test 2: Minimized Window
1. Minimize one window  
**PASS:** Window appears dimmed (50% opacity)

### ✅ Test 3: Overflow Chip
1. Open 6+ windows  
**PASS:** See 4 pills + `+N` chip (e.g., `+2`)

### ✅ Test 4: Sorting
Open: Dolphin, Konsole, Browser, VS Code  
**PASS:** Right pills in order: VS Code, Browser, Konsole, Dolphin

### ✅ Test 5: Focus Switch
Click a right-side pill  
**PASS:** Window moves to left card, becomes active

## Expected Logs

```
Upgrading package from file: /usr/lib/Coucou/kwin/coucou-focus
Successfully upgraded /home/om-raut/.local/share/kwin/scripts/coucou-focus/
WINDOWS count=14
WINDOWS count=15  (after opening window)
WINDOWS count=14  (after closing window)
```

## What to Report

For each test, report:
1. ✅/❌ PASS/FAIL
2. Screenshot of island (if failure)
3. Output of `ListWindows` command
4. Any console errors

## Known Good Baseline

Phase C confirmed working:
- `ListWindows` returns 14 distinct KWin UUIDs
- Real windows: Dolphin, VS Code (2x), Claude, Brave, Konsole (1 minimized), ONLYOFFICE, ChatGPT, App Center, Kiro, Coucou
- Active window detection working
- Blank-caption plasmashell entries handled correctly
