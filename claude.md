# Coucou: project context for Kiro

## Role and rules
You are a senior Rust + TypeScript developer (Tauri, tokio, zbus, DBus, KDE KWin scripting). You are customizing the open-source app Coucou (upstream: github.com/Louis-CFM/coucou) for one user, Om, on Linux KDE Plasma 6 (Wayland).
1. READ BEFORE WRITING. Never invent file names, functions, events or APIs; grep for them.
2. Work in phases. After each phase STOP, show runtime evidence, and wait for "go".
3. Maximum 2 attempts per problem, then stop and report the blocker. Never loop.
4. Verification commands for Om: ONE per step, never piped with "|". Use `busctl --user`, never `qdbus`.
5. A successful build is NOT proof. Only runtime evidence counts (DBus data, logs, visible UI).
6. Additive changes only. Keep existing WindowChanged and ListWindows DBus methods and existing Tauri events working.
7. PROTECTED: never edit windows/src-tauri/src/engine.rs (finished, 14 tests pass, deliberately has no LastFocus/FocusState). Never delete or overwrite files outside the repo (including ~/.local/share/kwin/scripts) without asking Om first.
8. Version-dependent API (Tauri v1/v2, zbus macros, KWin 5/6 scripting)? Check Cargo.toml and the installed versions, and say what you VERIFIED versus ASSUMED.

## Environment (verified)
- Ubuntu, KDE Plasma 6, WAYLAND session. xdotool/xprop/wmctrl cannot see native Wayland windows (placeholder id 0x200000, no PID/class): never use them.
- Shell bash. Browser Brave. Terminal Konsole. Editor VS Code.
- Real KWin resourceClass values: code_code (VS Code), brave (browser), org.kde.konsole (terminal), org.kde.plasmashell (other), com.anthropic.claude (other).
- Repo: Tauri app under windows/. Known files:
  - Rust, windows/src-tauri/src: lib.rs (Tauri setup + generate_handler!), focus_tracker.rs (zbus DBus service com.coucou.FocusTracker, WindowEntry, windows_snapshot, focus_tracker_windows, install_kwin_script stub), classify.rs, engine.rs (PROTECTED), pipe.rs (unix socket listener), platform/linux.rs
  - KWin package in repo: windows/src-tauri/kwin/coucou-focus/ (metadata.json, contents/code/main.js)
  - TypeScript, windows/src: main.ts, core/bridge.ts, core/state.ts, views/views.ts, island/hooks.ts
  - Docs: coucou/WINDOW_PILLS_INVESTIGATION.md (earlier baseline; parts are stale) and the handoff notes in the chat.
  Verify these by reading the tree; some names above come from earlier reports.

## Architecture
1. FOCUS SENSOR (works): a KWin script calls DBus com.coucou.FocusTracker /com/coucou/FocusTracker method WindowChanged(sss)=(class, title, pid) on focus changes. The app owns that bus name via zbus 5.x. LESSON: the zbus Connection must be kept alive for the whole app lifetime or the name vanishes. All DBus args are strings because KWin can send numbers as doubles.
2. WINDOW LIST: the KWin script reads workspace.windowList() and sends ONE JSON string via DBus method WindowsSnapshot(s). focus_tracker.rs parses it into Vec<WindowEntry>, stores it, emits Tauri event "windows-changed", and exposes ListWindows() on DBus and the Tauri command focus_tracker_windows. Frontend path: core/bridge.ts focusTrackerWindows() -> main.ts (initial fetch + "windows-changed" listener) -> core/state.ts (desktopWindows -> active task and other tasks, falls back to settings-selected provider pills when the list is empty) -> views/views.ts (focus task on the LEFT, up to 4 other pills on the RIGHT).
3. CLASSIFICATION: classify.rs maps class -> AppType {VsCode, Browser, Terminal, Other}.
4. TERMINAL SENSOR (unverified end to end): ~/.local/share/coucou/scripts/terminal-hook.sh (bash DEBUG trap + PROMPT_COMMAND) -> upstream binary ~/.local/share/coucou/bin/coucou-hook -> unix socket $XDG_RUNTIME_DIR/coucou.sock, listened on by pipe.rs. Never add a second listener on that socket.
5. TIMER ENGINE: engine.rs, pure logic, API Config::from_env() (COUCOU_FAST=1 divides timings by 60), Engine::new, on_focus, on_tick, on_terminal_finished, on_lock_changed, debug_summary. Rules: VS Code 25 min -> "5 minutes break time", browser 30 min -> "Go back to VS Code" (repeat every 10 min), terminal >= 30s -> "Task is finished!". Whether it is wired into the runtime is UNVERIFIED.

## Hard-won KWin / DBus facts (verified on Om's machine)
- kpackagetool6 installs KWin scripts to ~/.local/share/kwin/scripts/<id>. KWin 6 does NOT search ~/.config/kwin/scripts.
- org.kde.KWin /Scripting exposes isScriptLoaded(s), loadScript(s) and loadScript(ss), unloadScript(s), start(). There is NO run() method.
- org.kde.KWin /KWin reconfigure takes no args; with busctl the call needs an empty signature argument: busctl --user call org.kde.KWin /KWin org.kde.KWin reconfigure ""
- plasmashell --replace does NOT reload KWin scripts.
- KWin 6 metadata.json needs "KPackageStructure": "KWin/Script", "X-Plasma-API": "javascript", "X-Plasma-MainScript": "code/main.js".
- Coucou has a single-instance guard (fr.louisraille.coucou.SingleInstance). ALWAYS run `pkill coucou` before testing a rebuilt app, or a new launch hands off to the old process and tests stale code.
- KWin payloads use camelCase keys (resourceClass); the Rust struct needs serde rename_all = "camelCase". A parse failure must NEVER overwrite the last good window list with an empty one.
- The KWin Window object's internalId is a QUuid; use internalId.toString() as the unique window id.

## Output style
Lead with findings, then actions. Show file paths and diffs. Mark anything unverified.


block 2:
Read .kiro/steering/coucou.md first (or the rules pasted above if steering is unavailable). Do the phases below in order. STOP at the end of each phase and wait for my "go". No file edits until I approve your Phase 0 report.

PHASE 0 - READ-ONLY STUDY
Read in full: every .rs file under windows/src-tauri/src (including platform/), Cargo.toml, tauri.conf.json, windows/src-tauri/kwin/coucou-focus/metadata.json and contents/code/main.js, and under windows/src: main.ts, core/bridge.ts, core/state.ts, views/views.ts, island/hooks.ts, plus coucou/WINDOW_PILLS_INVESTIGATION.md and package.json. Then report:
 a) Tauri/zbus/tokio versions, how setup() starts services, every Tauri command and event, and whether focus_tracker_windows is in generate_handler!.
 b) Exact data path of window data from KWin to the island, with the file and function at each hop, and where an empty list can be produced silently.
 c) Current state of WindowEntry serde naming, the windows_snapshot error handling, and install_kwin_script().
 d) Whether engine.rs is wired into the runtime; whether the pipe.rs terminal tap exists; any remaining xdotool/wmctrl/xprop code.
 e) What state.ts, main.ts and views.ts currently do with desktopWindows: minimized filtering, the 8-item cap, dedupe keys, task id scheme, the 4-pill cap, and the settings-pill fallback.
 f) Risks and the exact files you plan to touch per phase below.
Run, one command each, and show the output: cargo test (from windows/src-tauri) and npm run build (from windows).
Then STOP.

KNOWN BLOCKER (do not act on it yet)
The installed KWin package at ~/.local/share/kwin/scripts/coucou-focus is stale/malformed (it has no WindowsSnapshot sender and its metadata is rejected as "KWin/Script"), so both kpackagetool6 --upgrade and --install fail. Om removes or moves aside that directory and its duplicate at ~/.config/kwin/scripts/coucou-focus BY HAND. Never delete anything yourself; if you need a file removed, ask.

MY ACTUAL GOAL (everything above exists to serve this)
On KDE Plasma 6 Wayland, the expanded Coucou island must reflect the desktop's real open windows, live:
- the currently ACTIVE window is the big item on the LEFT;
- every other open window (Brave, Konsole, VS Code, Dolphin, ...) is a pill on the RIGHT, ordered VsCode, Browser, Terminal, Other;
- max 4 visible pills plus a "+N" overflow chip; minimized windows stay visible but dimmed;
- it updates within about 1 second as windows open, close, minimize, restore, change title or gain focus;
- two windows of the same app stay distinct (unique per-window id, not pid/class);
- the settings-selected integration pills (Cursor, Anthropic, Google AI, OpenAI...) must NOT substitute for the live list; they are only a fallback if the window list has never loaded.
(Browser tabs are not visible on Wayland, only windows.)

PHASE C - DELIVERY RELIABILITY (after my "go")
1. Bundle the KWin package with the app. Implement an idempotent install: kpackagetool6 --upgrade, fall back to --install, then kwriteconfig6 --file kwinrc --group Plugins --key coucou-focusEnabled true, then busctl reconfigure with the empty "" argument. If the target exists but is malformed, report and stop.
2. Run the install/reload only AFTER com.coucou.FocusTracker is owned, so the first snapshot is not lost. Verify on this machine which DBus calls actually load the script; do not assume reconfigure alone does.
3. Frontend: poll focus_tracker_windows about every 5s in addition to the "windows-changed" event; log invoke failures and keep the last good list.
RUNTIME GATE (I run these and paste output): pkill coucou, start the app with windows already open; isScriptLoaded must be true; busctl --user monitor com.coucou.FocusTracker must show WindowsSnapshot calls; ListWindows must be non-empty within 10 seconds. Show those three results SEPARATELY.

PHASE D - ISLAND UI (after my "go")
Implement the goal above in state.ts, main.ts and views.ts: unique window id per task, keep minimized windows (dimmed), remove the 8-item cap, exactly one active window on the left, ordered right pills, 4 + "+N". Reuse the existing pill CSS/components. Report what the island SHOWS and what ListWindows RETURNED, separately.

PHASE E - DEV LOOP (after my "go")
From package.json, give me the fastest way to run the full Tauri app (Rust + DBus, not just Vite) without building a .deb each time, including the pkill coucou step.

START NOW with Phase 0. No edits.