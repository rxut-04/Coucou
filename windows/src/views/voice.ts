// Voice Assistant view in the Dynamic Island:
// - Live microphone indicator with pulsing glow (satisfying strict privacy visibility)
// - Real-time animated audio wave reacting to live voice meter
// - Streaming partial transcript as you speak
// - Measured transcription latency display
// - Full keyboard control: [Esc] cancels, [Enter] completes immediately, or auto-stops on silence

import { h, svg } from "./dom";
import { ICONS } from "./icons";
import { Bridge, onEvent, type VoiceMeterPayload, type VoiceStatePayload, type VoiceTranscriptPayload } from "../core/bridge";
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

  const card = h("div", { class: "card voice-card" }, header, waveContainer, transcriptContainer, hints);
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
