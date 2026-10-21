// Phase 4 Test Suite: Permission Tiers, Scoped File Search, Note Creation, Summarization, Undo, and Confirmation
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";

const homeDir = os.homedir();
const notesDir = path.join(homeDir, "Documents", "CoucouNotes");
fs.mkdirSync(notesDir, { recursive: true });

function callDbusProcessQuery(query) {
  const t0 = performance.now();
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
      query,
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
  return { ...data, busctlDuration };
}

console.log("================================================================================");
console.log("   COUCOU VOICE ASSISTANT — PHASE 4 VERIFICATION (SAFETY, FILES & TIERS)");
console.log("================================================================================\n");

let passed = 0;
let total = 0;

function assertTest(title, condition, details = "") {
  total++;
  if (condition) {
    passed++;
    console.log(`[PASS] ${title}`);
    if (details) console.log(`       ${details}`);
  } else {
    console.error(`[FAIL] ${title}`);
    if (details) console.error(`       ${details}`);
  }
}

// ── Test 1: File Search (Tier 1) ──────────────────────────────────────────────
console.log("--- 1. Scoped File Search (Tier 1: Immediate) ---");
const searchRes = callDbusProcessQuery("search files api");
assertTest(
  "Search files returns results in allowed scope",
  searchRes.success && searchRes.tier.includes("Tier 1"),
  `Tier: ${searchRes.tier} | Feedback: "${searchRes.feedback}" | Latency: ${searchRes.decisionLatencyMs} ms`
);

// ── Test 2: Find & Open File (Tier 2) ─────────────────────────────────────────
console.log("\n--- 2. Find and Open File (Tier 2: Benign File Access) ---");
const openRes = callDbusProcessQuery("find and open api.js");
assertTest(
  "Find and open triggers Tier 2 file access",
  openRes.tier.includes("Tier 2"),
  `Tier: ${openRes.tier} | Feedback: "${openRes.feedback}" | Latency: ${openRes.decisionLatencyMs} ms`
);

// ── Test 3: Summarize File (Tier 2) ───────────────────────────────────────────
console.log("\n--- 3. File Summarization (Tier 2) ---");
const dummySummaryFile = path.join(notesDir, "test_summary.txt");
fs.writeFileSync(dummySummaryFile, "Phase 4 adds permission tiers and secure scoped file access to Coucou voice assistant.\nEverything runs 100% locally with high performance and zero privacy leakage.");

const summaryRes = callDbusProcessQuery("summarize test_summary.txt");
assertTest(
  "Summarize reads and previews allowed file safely",
  summaryRes.success && summaryRes.feedback.includes("test_summary.txt") && summaryRes.tier.includes("Tier 2"),
  `Feedback: "${summaryRes.feedback}" | Decision: ${summaryRes.decisionLatencyMs} ms`
);
fs.unlinkSync(dummySummaryFile);

// ── Test 4: Take a Note & Undo (Tier 2) ───────────────────────────────────────
console.log("\n--- 4. Take a Note & Undo Reversal (Tier 2) ---");
const noteRes = callDbusProcessQuery("take a note about testing coucou voice undo feature");
assertTest(
  "Take note creates file in ~/Documents/CoucouNotes and flags canUndo",
  noteRes.success && noteRes.canUndo && noteRes.tier.includes("Tier 2"),
  `Feedback: "${noteRes.feedback}" | canUndo: ${noteRes.canUndo}`
);

// Verify file was written to disk
const noteFilesBefore = fs.readdirSync(notesDir).filter(f => f.toLowerCase().startsWith("note_"));
assertTest("Note file exists on disk", noteFilesBefore.length > 0, `Found: ${noteFilesBefore.join(", ")}`);

// Now execute Undo
const undoRes = callDbusProcessQuery("undo that");
assertTest(
  "Undo reverts the note creation",
  undoRes.success && (undoRes.feedback.toLowerCase().includes("removed") || undoRes.feedback.toLowerCase().includes("reverted")),
  `Feedback: "${undoRes.feedback}" | Total: ${undoRes.totalLatencyMs} ms`
);

// ── Test 5: Tier 3 Deletion with Voice Confirmation ('yes') ───────────────────
console.log("\n--- 5. Sensitive File Deletion with Voice Confirmation (Tier 3) ---");
const deleteTarget = path.join(notesDir, "temp_to_delete.txt");
fs.writeFileSync(deleteTarget, "This file will be deleted under Tier 3 confirmation.");

const delPromptRes = callDbusProcessQuery("delete file temp_to_delete.txt");
assertTest(
  "Delete queues confirmation prompt and returns requiresConfirmation: true",
  delPromptRes.requiresConfirmation && delPromptRes.tier.includes("Tier 3"),
  `Feedback: "${delPromptRes.feedback}" | requiresConfirmation: ${delPromptRes.requiresConfirmation}`
);

assertTest("File is NOT deleted before user confirms", fs.existsSync(deleteTarget));

// Voice confirmation: "yes"
const confirmRes = callDbusProcessQuery("yes");
assertTest(
  "Confirming with 'yes' performs deletion",
  confirmRes.success && confirmRes.feedback.toLowerCase().includes("deleted"),
  `Feedback: "${confirmRes.feedback}"`
);
assertTest("File is now safely deleted from disk", !fs.existsSync(deleteTarget));

// ── Test 6: Tier 3 Deletion Cancellation ('no') ───────────────────────────────
console.log("\n--- 6. Sensitive File Deletion Cancellation (Tier 3) ---");
const keepTarget = path.join(notesDir, "important_doc_to_keep.txt");
fs.writeFileSync(keepTarget, "This file must NOT be deleted.");

const delCancelPromptRes = callDbusProcessQuery("delete file important_doc_to_keep.txt");
assertTest(
  "Delete queues confirmation for second file",
  delCancelPromptRes.requiresConfirmation && delCancelPromptRes.tier.includes("Tier 3"),
  `Feedback: "${delCancelPromptRes.feedback}"`
);

// User cancels: "no"
const cancelRes = callDbusProcessQuery("no");
assertTest(
  "Declining with 'no' cancels action",
  cancelRes.success && cancelRes.feedback.includes("Cancelled"),
  `Feedback: "${cancelRes.feedback}"`
);
assertTest("File was preserved and still exists", fs.existsSync(keepTarget));
fs.unlinkSync(keepTarget);

console.log("\n================================================================================");
console.log(`PHASE 4 TEST SUMMARY: ${passed}/${total} TESTS PASSED (${((passed/total)*100).toFixed(0)}%)`);
console.log("================================================================================");

if (passed !== total) {
  process.exit(1);
}
