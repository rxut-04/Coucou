import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const STORAGE_KEY = "coucou_scratchpad";

async function readBackendNotes(): Promise<string> {
  try {
    const val = await invoke<string>("read_scratchpad");
    if (val !== undefined && val !== null && val.length > 0) return val;
  } catch {}
  return localStorage.getItem(STORAGE_KEY) ?? "";
}

async function writeBackendNotes(content: string): Promise<void> {
  try {
    localStorage.setItem(STORAGE_KEY, content);
  } catch {}
  try {
    await invoke("write_scratchpad", { content });
  } catch {}
}

async function initScratchpad() {
  const root = document.getElementById("scratchpad-root");
  if (!root) return;

  const initialText = await readBackendNotes();

  // Header
  const header = document.createElement("div");
  header.className = "sp-header";

  const titleArea = document.createElement("div");
  titleArea.className = "sp-title-area";

  const icon = document.createElement("span");
  icon.className = "sp-icon";
  icon.textContent = "📝";

  const title = document.createElement("span");
  title.className = "sp-title";
  title.textContent = "Quick Scratchpad";

  const badge = document.createElement("span");
  badge.className = "sp-badge";
  badge.textContent = "● Synced & Saved";

  titleArea.append(icon, title, badge);

  // Actions
  const actions = document.createElement("div");
  actions.className = "sp-actions";

  const copyBtn = document.createElement("button");
  copyBtn.className = "sp-btn sp-btn-copy";
  copyBtn.textContent = "📋 Copy All";
  copyBtn.onclick = async () => {
    try {
      await navigator.clipboard.writeText(textarea.value);
      copyBtn.textContent = "✓ Copied!";
      setTimeout(() => { copyBtn.textContent = "📋 Copy All"; }, 1500);
    } catch {
      copyBtn.textContent = "Failed";
    }
  };

  const clearBtn = document.createElement("button");
  clearBtn.className = "sp-btn sp-btn-clear";
  clearBtn.textContent = "🗑️ Clear";
  clearBtn.onclick = async () => {
    if (textarea.value.trim() && confirm("Clear all content from the scratchpad?")) {
      textarea.value = "";
      await writeBackendNotes("");
      updateStats();
    }
  };

  actions.append(copyBtn, clearBtn);
  header.append(titleArea, actions);

  // Textarea Wrapper
  const wrapper = document.createElement("div");
  wrapper.className = "sp-textarea-wrapper";

  const textarea = document.createElement("textarea");
  textarea.className = "sp-textarea";
  textarea.placeholder = "Write your thoughts, ideas, plans, code snippets, or commands here...\n\nEverything you type here is auto-saved and synced across Coucou seamlessly.";
  textarea.value = initialText;

  // Handle Tab key for indenting
  textarea.addEventListener("keydown", (e: KeyboardEvent) => {
    if (e.key === "Tab") {
      e.preventDefault();
      const start = textarea.selectionStart;
      const end = textarea.selectionEnd;
      textarea.value = textarea.value.substring(0, start) + "  " + textarea.value.substring(end);
      textarea.selectionStart = textarea.selectionEnd = start + 2;
      void writeBackendNotes(textarea.value);
      updateStats();
    }
  });

  textarea.addEventListener("input", () => {
    void writeBackendNotes(textarea.value);
    updateStats();
  });

  wrapper.appendChild(textarea);

  // Footer
  const footer = document.createElement("div");
  footer.className = "sp-footer";

  const tip = document.createElement("span");
  tip.className = "sp-tip";
  tip.textContent = "Press Tab to indent · Auto-saves instantly";

  const stats = document.createElement("span");
  stats.className = "sp-stats";

  function updateStats() {
    const text = textarea.value;
    const chars = text.length;
    const lines = text ? text.split("\n").length : 0;
    const words = text.trim() ? text.trim().split(/\s+/).length : 0;
    stats.textContent = `${lines} line${lines === 1 ? "" : "s"} · ${words} words · ${chars} chars`;
  }
  updateStats();

  footer.append(tip, stats);

  root.append(header, wrapper, footer);

  // Listen for sync from island or file
  try {
    await listen<string>("scratchpad-updated", (event) => {
      if (document.activeElement !== textarea && event.payload !== textarea.value) {
        textarea.value = event.payload;
        try { localStorage.setItem(STORAGE_KEY, event.payload); } catch {}
        updateStats();
      }
    });
  } catch {}

  // Also listen for window storage events
  window.addEventListener("storage", (e) => {
    if (e.key === STORAGE_KEY && e.newValue !== null && document.activeElement !== textarea) {
      textarea.value = e.newValue;
      updateStats();
    }
  });

  textarea.focus();
}

window.addEventListener("DOMContentLoaded", () => {
  void initScratchpad();
});
