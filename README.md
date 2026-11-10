<!-- ===================== HERO BANNER ===================== -->
<div align="center">
  <img src="https://capsule-render.vercel.app/api?type=waving&color=gradient&customColorList=6,2,12&height=220&section=header&text=Coucou+✨&fontSize=52&fontColor=ffffff&animation=twinkling&fontAlignY=38&desc=A+Next-Gen+Dynamic+Island+%26+Mochi+AI+Companion+for+Linux+%26+Windows&descFontSize=18&descAlignY=62" width="100%" alt="Coucou Hero" />
</div>

<div align="center">
  <a href="https://github.com/rxut-04/Coucou/stargazers">
    <img src="https://img.shields.io/github/stars/rxut-04/Coucou?style=for-the-badge&logo=starship&logoColor=white&labelColor=1a1a1a&color=FF69B4" alt="Stars" />
  </a>
  <a href="https://github.com/rxut-04/Coucou/network/members">
    <img src="https://img.shields.io/github/forks/rxut-04/Coucou?style=for-the-badge&logo=git&logoColor=white&labelColor=1a1a1a&color=FF69B4" alt="Forks" />
  </a>
  <a href="https://github.com/rxut-04/Coucou/issues">
    <img src="https://img.shields.io/github/issues/rxut-04/Coucou?style=for-the-badge&logo=github&logoColor=white&labelColor=1a1a1a&color=FF69B4" alt="Issues" />
  </a>
  <img src="https://img.shields.io/badge/License-MIT-FF69B4?style=for-the-badge&logo=open-source-initiative&logoColor=white&labelColor=1a1a1a" alt="MIT License" />
</div>

<br>

<!-- ===================== THREE TILES ===================== -->
<div align="center">
  <a href="#-key-custom-features">
    <img src="https://img.shields.io/badge/⚡_Proactive_Island-Dynamic_Widgets-1a1a1a?style=for-the-badge&logo=sparkles&logoColor=FF69B4" width="31%" alt="Proactive Island" />
  </a>
  <a href="#-media-visualizer">
    <img src="https://img.shields.io/badge/🎵_Now_Playing-MPRIS_Visualizer-1a1a1a?style=for-the-badge&logo=spotify&logoColor=FF69B4" width="31%" alt="Media Visualizer" />
  </a>
  <a href="#-voice--intelligence">
    <img src="https://img.shields.io/badge/🎙️_Voice_Agent-ElevenLabs_STT-1a1a1a?style=for-the-badge&logo=openai&logoColor=FF69B4" width="31%" alt="Voice Agent" />
  </a>
</div>

<br>

---

<!-- ===================== ABOUT ===================== -->
<h2 align="center">✨ Meet Coucou & Mochi</h2>

<p align="center">
  <b>Coucou</b> is an open-source, ultra-fluid <b>Dynamic Island & AI Companion</b> built with <b>Tauri 2</b>, <b>Rust</b>, and <b>TypeScript</b>.<br>
  Anchored seamlessly to the top of your screen on <b>Linux (Wayland & X11)</b> and <b>Windows</b>, Mochi observes your workflow, manages AI coding sessions, provides instant one-click OS actions, and reacts with charming physics-driven animations.
</p>

<div align="center">
  <img src="docs/media/demo.gif" width="760" alt="Coucou in action" style="border-radius: 12px; box-shadow: 0 8px 24px rgba(0,0,0,0.4);" />
</div>

<br>

---

<!-- ===================== KEY FEATURES ===================== -->
<h2 align="center">🚀 Key Custom Features</h2>

<table align="center" width="100%" border="0">
  <tr>
    <td width="50%" valign="top">
      <h3>🎵 Now Playing Media Visualizer</h3>
      <ul>
        <li><b>Dynamic Island Transformation</b>: When playing audio or video in <b>Brave, Chrome, Spotify, or VLC</b>, the compact top bar morphs into an Apple-style media visualizer.</li>
        <li><b>Animated Dancing Wave</b>: Real-time 4-bar dancing equalizer (<code>@keyframes eq-bounce</code>) reacting to playback.</li>
        <li><b>Hover Controls</b>: Instant hover reveals <code>⏮</code> Prev, <code>⏯</code> Play/Pause, <code>⏭</code> Next, and a clickable seeking scrubber.</li>
      </ul>
    </td>
    <td width="50%" valign="top">
      <h3>⚡ Proactive Contextual Pills</h3>
      <ul>
        <li>🌿 <b>Git Uncommitted Changes (<code>🌿 X modified · Commit</code>)</b>: Live git status tracker with 1-click instant AI commit checkpoints.</li>
        <li>❌ <b>Test Failure Auto-Fix (<code>❌ Test Failed</code>)</b>: Real-time failure detection in VS Code / terminal with instant AI root-cause resolution.</li>
        <li>📄 <b>Browser Article Reader (<code>📄 TL;DR</code>)</b>: Detects documentation or article reading; click for instant AI summaries.</li>
      </ul>
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <h3>🛡️ Hardware & Resource Sentinel</h3>
      <ul>
        <li><b>Runaway Sentinel</b>: Continuously monitors CPU, RAM, and thermal sensors (<code>/sys/class/thermal</code>).</li>
        <li><b>Runaway Process Detection</b>: Warns when processes (e.g. Docker, Node) consume excessive memory (>30%) or CPU (>80%).</li>
        <li><b>1-Click Termination</b>: Kill runaway PIDs (<code>kill -15</code> / <code>kill -9</code>) straight from the island without opening a task manager.</li>
      </ul>
    </td>
    <td width="50%" valign="top">
      <h3>🎙️ Voice AI & Morning Recap</h3>
      <ul>
        <li><b>Voice Pipeline</b>: Speech-to-text (Wispr Flow / Whisper STT) + natural ElevenLabs voice agent for conversational OS control.</li>
        <li><b>Morning Recap (Where did I leave off)</b>: Automatically summarizes your git branch, last modified file, and incomplete tasks.</li>
        <li><b>500+ Motivational Quotes</b>: Context-aware quotes for VS Code, Antigravity, Terminal, and Browser sessions.</li>
      </ul>
    </td>
  </tr>
</table>

<br>

---

<!-- ===================== TECH STACK ===================== -->
<h2 align="center">💻 Tech Stack & Architecture</h2>

<table align="center" width="100%" border="0">
  <tr>
    <td width="50%" align="center" valign="top">
      <h3>⚙️ Core Engine & Backend</h3>
      <img src="https://img.shields.io/badge/Rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
      <img src="https://img.shields.io/badge/Tauri_2-%2324C8D8.svg?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri 2" />
      <img src="https://img.shields.io/badge/Wayland_Layer_Shell-%23FCC624.svg?style=for-the-badge&logo=linux&logoColor=black" alt="Wayland" />
      <img src="https://img.shields.io/badge/SQLite-%2307405E.svg?style=for-the-badge&logo=sqlite&logoColor=white" alt="SQLite" />
      <img src="https://img.shields.io/badge/DBus_%2F_MPRIS-%23E95420.svg?style=for-the-badge&logo=gnome&logoColor=white" alt="DBus" />
    </td>
    <td width="50%" align="center" valign="top">
      <h3>🌐 Frontend & UI Layer</h3>
      <img src="https://img.shields.io/badge/TypeScript-%23007ACC.svg?style=for-the-badge&logo=typescript&logoColor=white" alt="TypeScript" />
      <img src="https://img.shields.io/badge/Vite-%23646CFF.svg?style=for-the-badge&logo=vite&logoColor=white" alt="Vite" />
      <img src="https://img.shields.io/badge/HTML5_Canvas_2D-%23E34F26.svg?style=for-the-badge&logo=html5&logoColor=white" alt="Canvas 2D" />
      <img src="https://img.shields.io/badge/CSS3_Spring_Animations-%231572B6.svg?style=for-the-badge&logo=css3&logoColor=white" alt="CSS3" />
    </td>
  </tr>
  <tr>
    <td width="50%" align="center" valign="top">
      <h3>🤖 AI, Speech & LLM Integrations</h3>
      <img src="https://img.shields.io/badge/ElevenLabs-Voice_Synthesis-black?style=for-the-badge&logo=elevenlabs&logoColor=white" alt="ElevenLabs" />
      <img src="https://img.shields.io/badge/Whisper_%2F_WisprFlow-STT-FF69B4?style=for-the-badge&logo=openai&logoColor=white" alt="Whisper" />
      <img src="https://img.shields.io/badge/Claude_Code-%23D97706.svg?style=for-the-badge&logo=anthropic&logoColor=white" alt="Claude Code" />
      <img src="https://img.shields.io/badge/Antigravity_%2F_Gemini-4285F4?style=for-the-badge&logo=google&logoColor=white" alt="Gemini" />
    </td>
    <td width="50%" align="center" valign="top">
      <h3>🧰 Environment & Operating Systems</h3>
      <img src="https://img.shields.io/badge/Linux_KDE_Plasma-1D99F3?style=for-the-badge&logo=kde&logoColor=white" alt="KDE Plasma" />
      <img src="https://img.shields.io/badge/Linux_GNOME_%2F_Hyprland-FCC624?style=for-the-badge&logo=linux&logoColor=black" alt="Linux" />
      <img src="https://img.shields.io/badge/Windows_10_%2F_11-0078D4?style=for-the-badge&logo=windows&logoColor=white" alt="Windows" />
      <img src="https://img.shields.io/badge/VS_Code-0078d7.svg?style=for-the-badge&logo=visual-studio-code&logoColor=white" alt="VS Code" />
    </td>
  </tr>
</table>

<br>

---

<!-- ===================== QUICK START ===================== -->
<h2 align="center">🛠️ Quick Start & Build Instructions</h2>

### 1. Prerequisites
Ensure you have the following installed on your system:
- **Rust toolchain** (`cargo`, `rustc`)
- **Node.js** (v18+ recommended) & `npm`
- **Linux Packages** (for Wayland layer-shell & GTK):
  ```bash
  sudo apt install libgtk-3-dev libgtk-layer-shell-dev playerctl  # Debian/Ubuntu
  sudo pacman -S gtk3 gtk-layer-shell playerctl                  # Arch/Manjaro
  ```

### 2. Build & Run
```bash
# Clone the repository
git clone https://github.com/rxut-04/Coucou.git
cd Coucou/windows

# Install frontend dependencies
npm install

# Build release binary with Tauri
npm run build
npx tauri build --no-bundle --config src-tauri/tauri.linux.conf.json

# Run Coucou
./target/release/coucou
```

---

<!-- ===================== SHORTCUTS & INTERACTION ===================== -->
<h2 align="center">🎮 Keyboard Shortcuts & Interaction</h2>

<div align="center">

| Shortcut / Action | Functionality |
| :--- | :--- |
| **Move cursor to top-center edge** | Mochi peeks out and greets you 👋 |
| **Click the compact island** | Smoothly expands the full Dynamic Island card |
| **Click Mochi** | Squishes body + displays playful reaction |
| **Click Mochi 3× fast** | 😵‍💫 Puts Mochi into dizzy state |
| **Drag Mochi out of island** | Releases Mochi to float freely on your desktop |
| `Ctrl+Alt+Space` | Opens AI chat assistant from any application |
| `Ctrl+Alt+A` | Jumps to pending Claude / agent approval |
| `Ctrl+Alt+T` | Brings active session terminal window to front |
| `Ctrl+Alt+S` | Toggles island sound effects |
| `Esc` | Collapses the island back to compact / hidden |

</div>

<br>

---

<!-- ===================== AUTHOR & CONNECT ===================== -->
<h2 align="center">🤝 Let's Connect</h2>

<p align="center">
  <b>Om Raut</b> — B.E. Information Technology @ SVIT Nashik<br>
  Building full-stack systems, computer vision models, and AI desktop companions.
</p>

<div align="center">
  <a href="https://github.com/rxut-04">
    <img src="https://img.shields.io/badge/GitHub-FF69B4?style=for-the-badge&logo=github&logoColor=white" alt="GitHub" />
  </a>
  <a href="https://linkedin.com/in/">
    <img src="https://img.shields.io/badge/LinkedIn-FF69B4?style=for-the-badge&logo=linkedin&logoColor=white" alt="LinkedIn" />
  </a>
  <a href="mailto:oraut9025@gmail.com">
    <img src="https://img.shields.io/badge/Email-FF69B4?style=for-the-badge&logo=gmail&logoColor=white" alt="Email" />
  </a>
</div>

<br>

<!-- ===================== FOOTER ===================== -->
<div align="center">
  <img src="https://capsule-render.vercel.app/api?type=waving&color=gradient&customColorList=6,2,12&height=160&section=footer&text=Mochi+is+always+watching+over+your+code!&fontSize=24&fontColor=ffffff&animation=twinkling&fontAlignY=68" width="100%" alt="Footer" />
</div>
