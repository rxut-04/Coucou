// Phase 3 Validation Script:
// Tests and benchmarks all Phase 3 actions on Linux KDE Plasma (Wayland) over DBus:
// 1. Apps & Windows (focus, open, close)
// 2. Browser & YouTube (web search, yt-dlp direct video resolution, media controls, volume setting)
// 3. Coucou features (start focus, pause focus, cancel focus, tasks, daily goals, stats, open chat)
// 4. System basics (brightness controls, volume, mute, screenshot, lock screen)

import { execFileSync } from "node:child_process";

const testQueries = [
  // ── 1. Apps & Windows ──────────────────────────────────────────────────
  { category: "Apps & Windows", lang: "English", query: "focus vscode", expectedIntent: "FocusWindow" },
  { category: "Apps & Windows", lang: "Hindi", query: "terminal kholo", expectedIntent: "FocusWindow" },
  { category: "Apps & Windows", lang: "French", query: "ouvre vscode", expectedIntent: "FocusWindow" },
  { category: "Apps & Windows", lang: "English", query: "close konsole", expectedIntent: "CloseWindow" },
  { category: "Apps & Windows", lang: "Hindi", query: "window band karo", expectedIntent: "CloseWindow" },
  { category: "Apps & Windows", lang: "French", query: "ferme la fenetre", expectedIntent: "CloseWindow" },

  // ── 2. Browser & YouTube & Media ───────────────────────────────────────
  { category: "Browser & Search", lang: "English", query: "search for tauri v2 on wayland", expectedIntent: "SearchWeb" },
  { category: "Browser & Search", lang: "Hindi", query: "internet par dhundo rust tokio guide", expectedIntent: "SearchWeb" },
  { category: "Browser & Search", lang: "French", query: "cherche documentation tauri", expectedIntent: "SearchWeb" },
  { category: "YouTube Resolution", lang: "English", query: "play lo-fi on youtube", expectedIntent: "PlayYouTube" },
  { category: "YouTube Resolution", lang: "Hindi", query: "youtube pe lofi chalao", expectedIntent: "PlayYouTube" },
  { category: "Media & Volume", lang: "English", query: "pause music", expectedIntent: "MediaControl" },
  { category: "Media & Volume", lang: "English", query: "next song", expectedIntent: "MediaControl" },
  { category: "Media & Volume", lang: "English", query: "set volume to 80%", expectedIntent: "MediaControl" },
  { category: "Media & Volume", lang: "Hindi", query: "aawaz 50 karo", expectedIntent: "MediaControl" },
  { category: "Media & Volume", lang: "English", query: "mute", expectedIntent: "MediaControl" },

  // ── 3. Coucou Features ─────────────────────────────────────────────────
  { category: "Coucou Focus", lang: "English", query: "start focus session", expectedIntent: "StartFocus" },
  { category: "Coucou Focus", lang: "English", query: "pause focus", expectedIntent: "PauseFocus" },
  { category: "Coucou Focus", lang: "Hindi", query: "focus pause karo", expectedIntent: "PauseFocus" },
  { category: "Coucou Focus", lang: "English", query: "cancel focus", expectedIntent: "CancelFocus" },
  { category: "Coucou Tasks", lang: "English", query: "add task implement phase 3 voice assistant", expectedIntent: "AddTask" },
  { category: "Coucou Tasks", lang: "Hindi", query: "task complete ho gaya", expectedIntent: "CompleteTask" },
  { category: "Coucou Goals", lang: "English", query: "set goal complete phase 3 actions", expectedIntent: "SetGoal" },
  { category: "Coucou Goals", lang: "Hindi", query: "goal banao deliver voice assistant", expectedIntent: "SetGoal" },
  { category: "Coucou Stats", lang: "English", query: "show my score", expectedIntent: "ShowScore" },
  { category: "Coucou Stats", lang: "Hindi", query: "mera score dikhao", expectedIntent: "ShowScore" },
  { category: "Coucou Chat", lang: "English", query: "open chat", expectedIntent: "OpenChat" },
  { category: "Coucou Chat", lang: "Hindi", query: "chat kholo", expectedIntent: "OpenChat" },
  { category: "Coucou Chat", lang: "French", query: "ouvre le chat", expectedIntent: "OpenChat" },

  // ── 4. System Basics ───────────────────────────────────────────────────
  { category: "System Controls", lang: "English", query: "brightness up", expectedIntent: "SystemControl" },
  { category: "System Controls", lang: "Hindi", query: "brightness badhao", expectedIntent: "SystemControl" },
  { category: "System Controls", lang: "English", query: "brightness down", expectedIntent: "SystemControl" },
  { category: "System Controls", lang: "English", query: "brightness 70%", expectedIntent: "SystemControl" },
  { category: "System Controls", lang: "English", query: "take screenshot", expectedIntent: "SystemControl" },
];

console.log("==================================================================");
console.log("          Coucou Phase 3: Actions & System Controls Benchmarks    ");
console.log("==================================================================\n");

const results = [];

for (const t of testQueries) {
  const t0 = performance.now();
  try {
    const rawOutput = execFileSync(
      "busctl",
      [
        "--user",
        "call",
        "fr.louisraille.coucou.Voice",
        "/fr/louisraille/coucou/Voice",
        "fr.louisraille.coucou.Voice",
        "ProcessQuery",
        "s",
        t.query,
      ],
      { encoding: "utf8", timeout: 8000 }
    );

    const busctlDuration = performance.now() - t0;
    const trimmed = rawOutput.trim();
    let s = trimmed.startsWith("s ") ? trimmed.slice(2).trim() : trimmed;
    s = s.replace(/\\([0-7]{3})/g, (_, oct) => Buffer.from([parseInt(oct, 8)]).toString("binary"));
    s = Buffer.from(s, "binary").toString("utf8");
    s = s.replace(/\\'/g, "'");
    const inner = JSON.parse(s);
    const data = typeof inner === "string" ? JSON.parse(inner) : inner;

    const matchedIntent = data.intent.startsWith(t.expectedIntent);
    const status = matchedIntent ? "PASS" : "FAIL";
    const icon = data.isFastPath ? "⚡ Fast" : "🧠 Smart";

    console.log(`[${status}] [${icon}] [${t.category}] (${t.lang}) "${t.query}"`);
    console.log(`       Intent: ${data.intent} | Success: ${data.success}`);
    console.log(`       Feedback: "${data.feedback}"`);
    console.log(`       Decision: ${data.decisionLatencyMs} ms | Total: ${data.totalLatencyMs} ms | DBus RTT: ${busctlDuration.toFixed(1)} ms\n`);

    results.push({ ...t, ...data, busctlDuration, matchedIntent });
  } catch (err) {
    console.error(`Error processing "${t.query}":`, err.message);
  }
}

// Summary statistics
const passed = results.filter((r) => r.matchedIntent).length;
const total = results.length;
const fastCount = results.filter((r) => r.isFastPath).length;
const avgDecision = (results.reduce((acc, r) => acc + r.decisionLatencyMs, 0) / total).toFixed(2);
const avgTotal = (results.reduce((acc, r) => acc + r.totalLatencyMs, 0) / total).toFixed(2);
const avgDbus = (results.reduce((acc, r) => acc + r.busctlDuration, 0) / total).toFixed(2);

console.log("==================================================================");
console.log(`Results: ${passed}/${total} PASS (${((passed / total) * 100).toFixed(1)}%)`);
console.log(`Fast Path Count: ${fastCount}/${total}`);
console.log(`Avg Decision Latency: ${avgDecision} ms`);
console.log(`Avg Action Latency:   ${avgTotal} ms`);
console.log(`Avg DBus Roundtrip:   ${avgDbus} ms`);
console.log("==================================================================");
