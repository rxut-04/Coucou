import { COUCOU_TOKEN } from "./config.js";

const domainEl = document.getElementById("domain");
const categoryEl = document.getElementById("category");
const statusEl = document.getElementById("status");
const tokenInput = document.getElementById("token");
const saveBtn = document.getElementById("save");

async function refreshPopup() {
  const data = await chrome.storage.local.get([
    "lastDomain",
    "lastCategory",
    "lastStatus",
    "coucouToken",
  ]);
  domainEl.textContent = data.lastDomain || "—";
  categoryEl.textContent = data.lastCategory || "neutral";
  statusEl.textContent = data.lastStatus || "Ready";
  tokenInput.value = data.coucouToken || COUCOU_TOKEN;
}

saveBtn.addEventListener("click", async () => {
  const val = tokenInput.value.trim();
  if (val) {
    await chrome.storage.local.set({ coucouToken: val });
  }
  chrome.runtime.sendMessage({ type: "SYNC_NOW" }, () => {
    setTimeout(refreshPopup, 200);
  });
});

void refreshPopup();
