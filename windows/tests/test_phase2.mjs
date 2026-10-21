// Phase 2 Validation Script:
// Exercises Coucou's Voice Intent Engine via DBus on Linux KDE Wayland.
// Measures decision latency and total execution time for:
// - Fast path queries (English, Hindi, French, Spanish, code-switching, compound)
// - Memory follow-up queries ("play another one", "open the second one")
// - Ambiguous queries requesting clarification ("open it", "start it")
// - Smart path queries with live KDE context (active window, task, failed command)

import { execFileSync } from "node:child_process";

const testQueries = [
  // Fast Path: YouTube & Media
  { lang: "English", query: "play lo-fi on youtube", expectedFast: true },
  { lang: "Hindi", query: "youtube pe lofi chalao", expectedFast: true },
  { lang: "French", query: "joue du lofi sur youtube", expectedFast: true },
  { lang: "Spanish", query: "reproducir lofi en youtube", expectedFast: true },
  { lang: "English", query: "pause music", expectedFast: true },
  { lang: "Hindi", query: "gaana roko", expectedFast: true },
  { lang: "French", query: "pause la musique", expectedFast: true },
  { lang: "Spanish", query: "pausar musica", expectedFast: true },

  // Fast Path: Focus & Tasks & Stats
  { lang: "English", query: "start focus session", expectedFast: true },
  { lang: "English", query: "focus for 30 minutes", expectedFast: true },
  { lang: "Hindi", query: "focus session start karo", expectedFast: true },
  { lang: "French", query: "commence une session de focus", expectedFast: true },
  { lang: "English", query: "stop focus", expectedFast: true },
  { lang: "English", query: "add task test voice phase 2", expectedFast: true },
  { lang: "Hindi", query: "task complete ho gaya", expectedFast: true },
  { lang: "English", query: "show my score", expectedFast: true },
  { lang: "Hindi", query: "mera score dikhao", expectedFast: true },
  { lang: "French", query: "montre mon score", expectedFast: true },

  // Fast Path: App & Window Focus
  { lang: "English", query: "focus vscode", expectedFast: true },
  { lang: "Hindi", query: "code kholo", expectedFast: true },
  { lang: "French", query: "ouvre vscode", expectedFast: true },

  // Compound & Code-Switching
  { lang: "French/English", query: "ouvre VS Code et commence un focus session", expectedFast: true },
  { lang: "Hindi/English", query: "terminal kholo aur focus start karo", expectedFast: true },

  // Clarifications
  { lang: "Clarify", query: "open it", expectedFast: true, expectClarify: true },
  { lang: "Clarify", query: "start it", expectedFast: true, expectClarify: true },

  // Follow-up via Conversation Memory
  { lang: "FollowUp", query: "play another one", expectedFast: true },

  // Smart Path (OpenCode AI Reasoning Engine + Context)
  { lang: "Smart (English)", query: "How can I refactor my authentication service?", expectedFast: false },
];

console.log("==================================================================");
console.log("       Coucou Phase 2: Multilingual Voice Intent Benchmarks       ");
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

      const status = (t.expectedFast ? data.isFastPath : !data.isFastPath) ? "PASS" : "FAIL";
      const icon = data.isFastPath ? "⚡ Fast" : "🧠 Smart";
      
      console.log(`[${status}] [${icon}] (${t.lang}) "${t.query}"`);
      console.log(`       Feedback: "${data.feedback}"`);
      console.log(`       Decision: ${data.decisionLatencyMs} ms | Total: ${data.totalLatencyMs} ms | DBus RTT: ${busctlDuration.toFixed(1)} ms\n`);

      results.push({ ...t, ...data, busctlDuration });
  } catch (err) {
    console.error(`Error processing "${t.query}":`, err.message);
  }
}

// Summary
const fastCount = results.filter((r) => r.isFastPath).length;
const avgDecision = results.reduce((acc, r) => acc + r.decisionLatencyMs, 0) / (results.length || 1);
const avgTotal = results.reduce((acc, r) => acc + r.totalLatencyMs, 0) / (results.length || 1);

console.log("------------------------------------------------------------------");
console.log(`Total tests run: ${results.length}`);
console.log(`Fast-path recognized: ${fastCount}/${results.length} (${((fastCount / (results.length || 1)) * 100).toFixed(0)}%)`);
console.log(`Average Decision Latency: ${avgDecision.toFixed(2)} ms (Target: < 150 ms)`);
console.log(`Average Total Latency: ${avgTotal.toFixed(2)} ms`);
console.log("------------------------------------------------------------------");
