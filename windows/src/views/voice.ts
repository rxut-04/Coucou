// Voice Assistant view in the Dynamic Island:
// - Live microphone indicator with pulsing glow (satisfying strict privacy visibility)
// - Real-time animated audio wave reacting to live voice meter
// - Streaming partial transcript as you speak
// - Measured transcription latency display
// - Full keyboard control: [Esc] cancels, [Enter] completes immediately, or auto-stops on silence

import { h, svg } from "./dom";
import { ICONS } from "./icons";
import { Bridge, onEvent, type VoiceActionResult, type VoiceMeterPayload, type VoiceStatePayload, type VoiceTranscriptPayload } from "../core/bridge";
import type { ViewActions, ViewHost } from "./views";

export function buildVoiceView(actions: ViewActions): ViewHost {
  const container = h("div", { class: "view voice-view" });

  // ── Header bar ────────────────────────────────────────────────────────────
  const micIconSvg = svg(ICONS.mic || "M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z M19 10v2a7 7 0 0 1-14 0v-2 M12 19v4 M8 23h8", 18);
  const micBadge = h("div", { class: "voice-mic-badge listening" }, micIconSvg);
  
  const statusLabel = h("span", { class: "voice-status-label", text: "Listening…" });
  const latencyBadge = h("span", { class: "voice-latency-badge hidden", text: "" });

  const statusStack = h(
    "div",
    { class: "voice-status-group" },
    micBadge,
    statusLabel,
    latencyBadge
  );

  const closeBtn = h(
    "button",
    {
      class: "btn-icon voice-close-btn",
      title: "Cancel (Esc)",
      onClick: () => {
        void Bridge.voiceCancel();
        actions.collapse();
      },
    },
    svg(ICONS.close, 14)
  );

  const header = h("div", { class: "voice-header" }, statusStack, closeBtn);

  // ── Live audio wave visualizer ─────────────────────────────────────────────
  const waveBars: HTMLElement[] = [];
  const waveContainer = h("div", { class: "voice-wave-container" });
  for (let i = 0; i < 24; i++) {
    const bar = h("div", { class: "voice-wave-bar" });
    waveBars.push(bar);
    waveContainer.append(bar);
  }

  // ── Live Transcript Area ──────────────────────────────────────────────────
  const transcriptText = h("div", {
    class: "voice-transcript-text placeholder",
    text: "Say something in any language…",
  });
  const transcriptContainer = h("div", { class: "voice-transcript-container" }, transcriptText);

  // ── Action Result Card (Phase 2 & Phase 4) ──────────────────────────────────
  const actionIcon = h("span", { class: "voice-action-icon", text: "⚡" });
  const actionBadge = h("span", { class: "voice-action-badge fast", text: "⚡ Fast Path" });
  const tierBadge = h("span", { class: "voice-tier-badge hidden", text: "" });
  const actionTiming = h("span", { class: "voice-action-timing", text: "" });
  const actionHeader = h("div", { class: "voice-action-header" }, actionIcon, actionBadge, tierBadge, actionTiming);
  const actionFeedback = h("div", { class: "voice-action-feedback", text: "" });

  // Interactive controls for Confirmation (Tier 3) and Undo (Tier 2)
  const actionControls = h("div", { class: "voice-action-controls hidden" });
  const confirmBtn = h("button", {
    class: "voice-ctrl-btn voice-confirm-btn",
    text: "✓ Confirm ('yes')",
    onClick: () => {
      void Bridge.voiceProcessQuery("yes");
    },
  });
  const cancelBtn = h("button", {
    class: "voice-ctrl-btn voice-cancel-btn",
    text: "✕ Cancel ('no')",
    onClick: () => {
      void Bridge.voiceProcessQuery("no");
    },
  });
  const undoBtn = h("button", {
    class: "voice-ctrl-btn voice-undo-btn",
    text: "↺ Undo ('undo that')",
    onClick: () => {
      void Bridge.voiceProcessQuery("undo that");
    },
  });
  actionControls.append(confirmBtn, cancelBtn, undoBtn);

  const actionCard = h("div", { class: "voice-action-card hidden" }, actionHeader, actionFeedback, actionControls);

  // ── Footer hint bar ───────────────────────────────────────────────────────
  const hints = h(
    "div",
    { class: "voice-hints-bar" },
    h("span", { class: "voice-hint", text: "Auto-stops on silence" }),
    h("span", { class: "voice-hint-dot", text: "·" }),
    h("span", { class: "voice-hint", text: "Esc to cancel" }),
    h("span", { class: "voice-hint-dot", text: "·" }),
    h("span", { class: "voice-hint", text: "Enter to finish" })
  );

  const card = h("div", { class: "card voice-card" }, header, waveContainer, transcriptContainer, actionCard, hints);
  container.append(card);

  // ── State & Animation ─────────────────────────────────────────────────────
  let smoothedLevel = 0;
  let hasTranscribed = false;

  const updateStateUI = (st: "idle" | "listening" | "speaking" | "transcribing", error?: string | null) => {
    micBadge.className = `voice-mic-badge ${st}`;

    if (error) {
      statusLabel.textContent = error;
      statusLabel.style.color = "#F4505E";
      return;
    }
    statusLabel.style.color = "";

    switch (st) {
      case "listening":
        statusLabel.textContent = "Listening…";
        latencyBadge.classList.add("hidden");
        break;
      case "speaking":
        statusLabel.textContent = "Speaking…";
        break;
      case "transcribing":
        statusLabel.textContent = "Transcribing…";
        break;
      case "idle":
        if (hasTranscribed) {
          statusLabel.textContent = "Ready";
        } else {
          statusLabel.textContent = "Idle";
        }
        break;
    }
  };

  // Register real-time Tauri event listeners
  void onEvent<VoiceStatePayload>("voice-state", (payload) => {
    updateStateUI(payload.state, payload.error);
    if (payload.state === "listening") {
      hasTranscribed = false;
      transcriptText.className = "voice-transcript-text placeholder";
      transcriptText.textContent = "Say something in any language…";
      latencyBadge.classList.add("hidden");
      actionCard.classList.add("hidden");
      actionControls.classList.add("hidden");
      tierBadge.classList.add("hidden");
      if (collapseTimeout) {
        clearTimeout(collapseTimeout);
        collapseTimeout = null;
      }
    }
  });

  void onEvent<VoiceMeterPayload>("voice-meter", (payload) => {
    smoothedLevel = smoothedLevel * 0.4 + payload.level * 0.6;
    // Animate wave bars based on meter
    const heightFactor = Math.min(100, smoothedLevel * 1.8);
    for (let i = 0; i < waveBars.length; i++) {
      // Wave shape distribution: highest in the middle
      const centerDist = Math.abs(i - waveBars.length / 2) / (waveBars.length / 2);
      const curve = Math.cos(centerDist * Math.PI * 0.5);
      const barH = Math.max(4, Math.min(42, heightFactor * curve * (0.6 + 0.4 * Math.sin(i * 0.8 + performance.now() * 0.01))));
      waveBars[i].style.height = `${barH}px`;
    }
  });

  void onEvent<VoiceTranscriptPayload>("voice-partial-transcript", (payload) => {
    if (payload.text) {
      transcriptText.className = "voice-transcript-text partial";
      transcriptText.textContent = payload.text;
    }
  });

  void onEvent<VoiceTranscriptPayload>("voice-final-transcript", (payload) => {
    hasTranscribed = true;
    transcriptText.className = "voice-transcript-text final";
    transcriptText.textContent = payload.text || "(No speech detected)";

    if (payload.latencyMs != null) {
      latencyBadge.textContent = `⚡ ${payload.latencyMs} ms`;
      latencyBadge.classList.remove("hidden");
    }
  });

  let collapseTimeout: number | null = null;

  void onEvent<VoiceActionResult>("voice-action-result", (res) => {
    actionCard.classList.remove("hidden");
    actionFeedback.textContent = res.feedback;

    if (res.isFastPath) {
      actionBadge.textContent = "⚡ Fast Path";
      actionBadge.className = "voice-action-badge fast";
      actionTiming.textContent = `${res.decisionLatencyMs} ms · total ${res.totalLatencyMs} ms`;
    } else {
      actionBadge.textContent = "🧠 Smart Path";
      actionBadge.className = "voice-action-badge smart";
      actionTiming.textContent = `${(res.decisionLatencyMs / 1000).toFixed(1)}s`;
    }

    if (res.tier) {
      tierBadge.classList.remove("hidden");
      if (res.tier.includes("Tier 3")) {
        tierBadge.textContent = "🛡️ Tier 3";
        tierBadge.className = "voice-tier-badge tier-3";
      } else if (res.tier.includes("Tier 2")) {
        tierBadge.textContent = "📝 Tier 2";
        tierBadge.className = "voice-tier-badge tier-2";
      } else {
        tierBadge.textContent = "⚡ Tier 1";
        tierBadge.className = "voice-tier-badge tier-1";
      }
    } else {
      tierBadge.classList.add("hidden");
    }

    // Interactive confirmation & undo buttons
    if (res.requiresConfirmation) {
      actionControls.classList.remove("hidden");
      confirmBtn.style.display = "inline-flex";
      cancelBtn.style.display = "inline-flex";
      undoBtn.style.display = "none";
    } else if (res.canUndo) {
      actionControls.classList.remove("hidden");
      confirmBtn.style.display = "none";
      cancelBtn.style.display = "none";
      undoBtn.style.display = "inline-flex";
    } else {
      actionControls.classList.add("hidden");
      confirmBtn.style.display = "none";
      cancelBtn.style.display = "none";
      undoBtn.style.display = "none";
    }

    if (res.clarification || res.requiresConfirmation) {
      actionIcon.textContent = res.requiresConfirmation ? "🛡️" : "❓";
      actionCard.className = `voice-action-card ${res.requiresConfirmation ? "sensitive" : "clarify"}`;
      statusLabel.textContent = res.requiresConfirmation ? "Action Confirmation" : "Clarification";
      if (collapseTimeout) {
        clearTimeout(collapseTimeout);
        collapseTimeout = null;
      }
    } else {
      actionIcon.textContent = res.success ? "✨" : "⚠️";
      actionCard.className = `voice-action-card ${res.success ? "success" : "failure"}`;
      statusLabel.textContent = res.success ? "Executed" : "Failed";

      if (collapseTimeout) clearTimeout(collapseTimeout);
      collapseTimeout = window.setTimeout(() => {
        actions.collapse();
      }, res.canUndo ? 6000 : 3500);
    }
  });

  // Keyboard navigation inside voice view
  window.addEventListener("keydown", (e) => {
    if (container.parentElement && container.style.display !== "none") {
      if (e.key === "Escape") {
        e.preventDefault();
        void Bridge.voiceCancel();
        actions.collapse();
      } else if (e.key === "Enter") {
        e.preventDefault();
        void Bridge.voiceStop();
      }
    }
  });

  return {
    el: container,
    sync() {
      // Refresh status on view show
      void Bridge.voiceStatus().then((status) => {
        if (status) {
          if (status.listening) {
            updateStateUI(status.speaking ? "speaking" : "listening");
          }
          if (status.lastTranscript && !hasTranscribed) {
            transcriptText.className = "voice-transcript-text final";
            transcriptText.textContent = status.lastTranscript;
            if (status.latencyMs) {
              latencyBadge.textContent = `⚡ ${status.latencyMs} ms`;
              latencyBadge.classList.remove("hidden");
            }
          }
        }
      });
    },
    focus() {
      // Trigger voice listening when view opens
      void Bridge.voiceStart();
    },
  };
}
