// Coucou Browser Intent — MV3 Service Worker
// Extracts ONLY the active tab's hostname (never path, query, or fragment)
// and sends it to Coucou's localhost-only, token-protected endpoint.

import { COUCOU_ENDPOINT, COUCOU_TOKEN } from "./config.js";

let lastReportedDomain = "";
let lastReportedAt = 0;

function extractDomainOnly(rawUrl) {
  if (!rawUrl || typeof rawUrl !== "string") return "";
  try {
    const parsed = new URL(rawUrl);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return "";
    }
    return parsed.hostname.replace(/^www\./i, "").toLowerCase();
  } catch {
    return "";
  }
}

async function getActiveToken() {
  try {
    const stored = await chrome.storage.local.get(["coucouToken"]);
    if (stored && typeof stored.coucouToken === "string" && stored.coucouToken.trim()) {
      return stored.coucouToken.trim();
    }
  } catch {
    // Fallback to auto-synced config.js token
  }
  return COUCOU_TOKEN;
}

async function reportDomain(domain, force = false) {
  if (!domain) return null;
  const now = Date.now();
  if (!force && domain === lastReportedDomain && now - lastReportedAt < 3000) {
    return null;
  }
  lastReportedDomain = domain;
  lastReportedAt = now;

  const token = await getActiveToken();
  try {
    const res = await fetch(COUCOU_ENDPOINT, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Coucou-Token": token,
      },
      body: JSON.stringify({ domain }),
    });
    const data = await res.json();
    await chrome.storage.local.set({
      lastDomain: domain,
      lastCategory: data.category || "neutral",
      lastStatus: res.ok ? "connected" : `error (${res.status})`,
      lastUpdatedAt: now,
    });
    return data;
  } catch (err) {
    await chrome.storage.local.set({
      lastDomain: domain,
      lastStatus: "offline",
      lastUpdatedAt: now,
    });
    return null;
  }
}

async function checkActiveTab(force = false) {
  try {
    const tabs = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
    const tab = tabs && tabs[0];
    if (!tab || !tab.url) return;
    const domain = extractDomainOnly(tab.url);
    if (domain) {
      await reportDomain(domain, force);
    }
  } catch {
    // Ignore restricted tabs
  }
}

chrome.tabs.onActivated.addListener(() => {
  void checkActiveTab(true);
});

chrome.tabs.onUpdated.addListener((_tabId, changeInfo, tab) => {
  if (tab.active && (changeInfo.url || changeInfo.status === "complete")) {
    void checkActiveTab(false);
  }
});

chrome.windows.onFocusChanged.addListener((windowId) => {
  if (windowId !== chrome.windows.WINDOW_ID_NONE) {
    void checkActiveTab(true);
  }
});

chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (msg && msg.type === "SYNC_NOW") {
    checkActiveTab(true).then((res) => sendResponse({ ok: true, res }));
    return true;
  }
  return false;
});
