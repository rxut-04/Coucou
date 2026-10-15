import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { highlightCode } from "./highlighter";

interface FileItem {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
  ext: string;
}

interface DirListing {
  currentPath: string;
  parentPath: string | null;
  items: FileItem[];
}

const STORAGE_KEY = "coucou_scratchpad";
const THEME_KEY = "coucou_scratchpad_theme";
const SIDEBAR_WIDTH_KEY = "coucou_scratchpad_sidebar_w";

let currentFilePath: string | null = null;
let currentFileExt: string = "txt";
let currentDir: string | null = null;
let isUnsaved: boolean = false;
let isFullscreen: boolean = false;

window.addEventListener("beforeunload", (e) => {
  if (isUnsaved) {
    e.preventDefault();
  }
});

async function init() {
  const root = document.getElementById("scratchpad-root");
  if (!root) return;

  // 1. Theme Setup
  const savedTheme = localStorage.getItem(THEME_KEY) || "dark";
  if (savedTheme === "light") {
    document.body.classList.add("light-theme");
  }

  // 2. Build Layout Structure
  root.innerHTML = `
    <header class="studio-topbar">
      <div class="topbar-left">
        <div class="studio-brand">
          <span class="brand-icon">⚡</span>
          <span>Code Studio</span>
        </div>
        <div class="breadcrumb-path" id="breadcrumb" title="Current location">~ / notes</div>
        <span class="save-pill" id="save-pill">● Synced</span>
      </div>
      <div class="topbar-right">
        <button class="tool-btn" id="open-folder-btn" title="Open Directory on Local Computer">📂 Open Folder</button>
        <button class="tool-btn" id="open-file-btn" title="Open Existing File">📄 Open File</button>
        <button class="tool-btn btn-accent" id="save-file-btn" title="Save File (Ctrl+S)">💾 Save</button>
        <button class="tool-btn" id="theme-btn" title="Toggle Light / Dark Theme">🌓 Theme</button>
        <button class="tool-btn" id="fullscreen-btn" title="Toggle Fullscreen">⛶ Fullscreen</button>
      </div>
    </header>

    <div class="studio-workspace">
      <!-- Collapsible & Stretchable Directory Sidebar -->
      <aside class="studio-sidebar" id="sidebar">
        <div class="sidebar-header">
          <span class="sidebar-title" id="folder-title">EXPLORER</span>
          <div class="sidebar-actions">
            <button class="icon-btn" id="new-file-btn" title="New File (+)">➕</button>
            <button class="icon-btn" id="new-dir-btn" title="New Folder">📁</button>
            <button class="icon-btn" id="refresh-dir-btn" title="Refresh Directory">🔄</button>
            <button class="icon-btn" id="collapse-sidebar-btn" title="Collapse Sidebar (Stretch or click)">◀</button>
          </div>
        </div>
        <div class="sidebar-content" id="file-list"></div>
        <div class="sidebar-resizer" id="sidebar-resizer"></div>
      </aside>

      <!-- Code Editor Pane -->
      <main class="studio-editor-pane">
        <div class="editor-tabs-bar">
          <div class="tabs-list" id="tabs-list">
            <div class="tab-pill active" id="main-tab">scratchpad.txt</div>
          </div>
          <div class="editor-actions-strip">
            <button class="tool-btn" id="toggle-sidebar-btn" title="Toggle File Explorer Sidebar">☰ Explorer</button>
            <button class="tool-btn" id="copy-btn" title="Copy Content">📋 Copy</button>
          </div>
        </div>

        <div class="editor-code-container">
          <div class="editor-gutter" id="editor-gutter">1</div>
          <div class="editor-code-wrapper">
            <div class="editor-highlight" id="editor-highlight"></div>
            <textarea class="editor-input" id="editor-input" spellcheck="false" placeholder="Write notes, code, or commands here..."></textarea>
          </div>
        </div>
      </main>
    </div>

    <footer class="studio-statusbar">
      <div class="statusbar-left">
        <div class="status-item" id="file-info-status">Plain Text</div>
        <div class="status-item" id="file-path-status">Default Scratchpad</div>
      </div>
      <div class="statusbar-right">
        <div class="status-item" id="lines-status">Ln 1, Col 1</div>
        <div class="status-item" id="words-status">0 words</div>
        <div class="status-item" id="chars-status">0 chars</div>
        <div class="status-item">UTF-8</div>
      </div>
    </footer>
  `;

  // Element handles
  const editorInput = document.getElementById("editor-input") as HTMLTextAreaElement;
  const editorHighlight = document.getElementById("editor-highlight") as HTMLDivElement;
  const editorGutter = document.getElementById("editor-gutter") as HTMLDivElement;
  const savePill = document.getElementById("save-pill") as HTMLElement;
  const breadcrumb = document.getElementById("breadcrumb") as HTMLElement;
  const fileList = document.getElementById("file-list") as HTMLElement;
  const folderTitle = document.getElementById("folder-title") as HTMLElement;
  const mainTab = document.getElementById("main-tab") as HTMLElement;
  const sidebar = document.getElementById("sidebar") as HTMLElement;
  const sidebarResizer = document.getElementById("sidebar-resizer") as HTMLElement;
  const linesStatus = document.getElementById("lines-status") as HTMLElement;
  const wordsStatus = document.getElementById("words-status") as HTMLElement;
  const charsStatus = document.getElementById("chars-status") as HTMLElement;
  const fileInfoStatus = document.getElementById("file-info-status") as HTMLElement;
  const filePathStatus = document.getElementById("file-path-status") as HTMLElement;

  // Restore saved sidebar width
  const savedWidth = localStorage.getItem(SIDEBAR_WIDTH_KEY);
  if (savedWidth) {
    sidebar.style.width = `${savedWidth}px`;
  }

  // ── Sync & Render Highlighting ──────────────────────────────────────────────
  function updateHighlight() {
    const code = editorInput.value;
    editorHighlight.innerHTML = highlightCode(code, currentFileExt) + "\n";
    syncGutters();
    updateStatusMetrics();
  }

  function syncGutters() {
    const text = editorInput.value;
    const lines = text ? text.split("\n").length : 1;
    let gutterText = "";
    for (let i = 1; i <= lines; i++) {
      gutterText += `${i}\n`;
    }
    editorGutter.textContent = gutterText;
  }

  function updateStatusMetrics() {
    const text = editorInput.value;
    const chars = text.length;
    const words = text.trim() ? text.trim().split(/\s+/).length : 0;
    const lines = text ? text.split("\n").length : 1;

    // Cursor position
    const pos = editorInput.selectionStart;
    const linesUpToPos = text.substring(0, pos).split("\n");
    const currentLine = linesUpToPos.length;
    const currentCol = linesUpToPos[linesUpToPos.length - 1].length + 1;

    linesStatus.textContent = `Ln ${currentLine}, Col ${currentCol} (${lines} total)`;
    wordsStatus.textContent = `${words} words`;
    charsStatus.textContent = `${chars} chars`;
  }

  // Synchronize scrolling between textarea and highlight/gutter
  editorInput.addEventListener("scroll", () => {
    editorHighlight.scrollTop = editorInput.scrollTop;
    editorHighlight.scrollLeft = editorInput.scrollLeft;
    editorGutter.scrollTop = editorInput.scrollTop;
  });

  editorInput.addEventListener("input", () => {
    isUnsaved = true;
    savePill.textContent = "● Unsaved";
    savePill.className = "save-pill unsaved";
    updateHighlight();

    if (!currentFilePath) {
      // Sync default scratchpad with Island & disk
      void saveScratchpadDefault(editorInput.value);
    }
  });

  editorInput.addEventListener("click", updateStatusMetrics);
  editorInput.addEventListener("keyup", updateStatusMetrics);

  // Tab key indentation
  editorInput.addEventListener("keydown", (e: KeyboardEvent) => {
    if (e.key === "Tab") {
      e.preventDefault();
      const start = editorInput.selectionStart;
      const end = editorInput.selectionEnd;
      editorInput.value = editorInput.value.substring(0, start) + "  " + editorInput.value.substring(end);
      editorInput.selectionStart = editorInput.selectionEnd = start + 2;
      updateHighlight();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      void saveCurrentFile();
    }
  });

  async function saveScratchpadDefault(text: string) {
    try {
      localStorage.setItem(STORAGE_KEY, text);
      await invoke("write_scratchpad", { content: text });
      isUnsaved = false;
      savePill.textContent = "● Synced";
      savePill.className = "save-pill";
    } catch {}
  }

  async function saveCurrentFile() {
    const content = editorInput.value;
    if (currentFilePath) {
      try {
        await invoke("write_text_file", { path: currentFilePath, content });
        isUnsaved = false;
        savePill.textContent = "● Saved";
        savePill.className = "save-pill";
      } catch (err) {
        alert("Failed to save file: " + err);
      }
    } else {
      await saveScratchpadDefault(content);
    }
  }

  // ── Load Directory Explorer ────────────────────────────────────────────────
  async function loadDirectory(path: string | null = null) {
    try {
      const listing = await invoke<DirListing>("list_dir", { path });
      currentDir = listing.currentPath;
      breadcrumb.textContent = listing.currentPath;
      breadcrumb.title = listing.currentPath;

      const parts = listing.currentPath.split(/[/\\]/);
      folderTitle.textContent = parts[parts.length - 1] || "EXPLORER";

      renderFiles(listing);
    } catch (err) {
      console.error("Failed to list dir:", err);
    }
  }

  function getFileIcon(item: FileItem): string {
    if (item.isDir) return "📁";
    const ext = item.ext.toLowerCase();
    switch (ext) {
      case "js":
      case "mjs":
      case "cjs":
        return "🟨";
      case "ts":
      case "tsx":
        return "🟦";
      case "jsx":
        return "⚛️";
      case "json":
        return "📦";
      case "html":
        return "🌐";
      case "css":
        return "🎨";
      case "rs":
        return "🦀";
      case "py":
        return "🐍";
      case "sh":
      case "bash":
        return "🐚";
      case "md":
        return "📝";
      default:
        return "📄";
    }
  }

  function renderFiles(listing: DirListing) {
    fileList.innerHTML = "";

    // Always include a shortcut to default scratchpad at the top
    const scratchpadItem = document.createElement("div");
    scratchpadItem.className = `file-item ${currentFilePath === null ? "active" : ""}`;
    scratchpadItem.innerHTML = `
      <span class="file-icon">⚡</span>
      <span class="file-name">scratchpad.txt (Main)</span>
    `;
    scratchpadItem.onclick = () => void openScratchpadDefault();
    fileList.appendChild(scratchpadItem);

    // Parent directory link if available
    if (listing.parentPath) {
      const upItem = document.createElement("div");
      upItem.className = "file-item";
      upItem.innerHTML = `
        <span class="file-icon">⬆️</span>
        <span class="file-name">.. (Parent Directory)</span>
      `;
      upItem.onclick = () => void loadDirectory(listing.parentPath);
      fileList.appendChild(upItem);
    }

    for (const item of listing.items) {
      const el = document.createElement("div");
      el.className = `file-item ${currentFilePath === item.path ? "active" : ""}`;
      el.title = item.path;

      const icon = getFileIcon(item);
      el.innerHTML = `
        <span class="file-icon">${icon}</span>
        <span class="file-name">${item.name}</span>
        <button class="delete-file-btn" title="Delete">✕</button>
      `;

      el.onclick = (e) => {
        const target = e.target as HTMLElement;
        if (target.classList.contains("delete-file-btn")) {
          e.stopPropagation();
          if (confirm(`Delete ${item.name}?`)) {
            void invoke("delete_entry", { path: item.path }).then(() => {
              if (currentFilePath === item.path) {
                void openScratchpadDefault();
              }
              void loadDirectory(currentDir);
            });
          }
          return;
        }

        if (item.isDir) {
          void loadDirectory(item.path);
        } else {
          void openFile(item.path, item.name, item.ext);
        }
      };

      fileList.appendChild(el);
    }
  }

  async function openFile(path: string, name: string, ext: string) {
    try {
      const content = await invoke<string>("read_text_file", { path });
      currentFilePath = path;
      currentFileExt = ext;
      editorInput.value = content;
      isUnsaved = false;

      savePill.textContent = "● Saved";
      savePill.className = "save-pill";
      mainTab.textContent = name;
      mainTab.title = path;
      fileInfoStatus.textContent = ext.toUpperCase() || "Text";
      filePathStatus.textContent = path;

      updateHighlight();
      if (currentDir) void loadDirectory(currentDir);
    } catch (err) {
      alert("Error reading file: " + err);
    }
  }

  async function openScratchpadDefault() {
    currentFilePath = null;
    currentFileExt = "txt";
    let text = "";
    try {
      text = (await invoke<string>("read_scratchpad")) || localStorage.getItem(STORAGE_KEY) || "";
    } catch {}
    editorInput.value = text;
    isUnsaved = false;

    savePill.textContent = "● Synced";
    savePill.className = "save-pill";
    mainTab.textContent = "scratchpad.txt";
    mainTab.title = "Default scratchpad";
    fileInfoStatus.textContent = "Plain Text";
    filePathStatus.textContent = "Default Scratchpad";

    updateHighlight();
    if (currentDir) void loadDirectory(currentDir);
  }

  // ── Buttons & Action Listeners ─────────────────────────────────────────────
  document.getElementById("open-folder-btn")?.addEventListener("click", async () => {
    try {
      const picked = await invoke<string | null>("pick_folder_dialog");
      if (picked) void loadDirectory(picked);
    } catch (e) {
      console.error(e);
    }
  });

  document.getElementById("open-file-btn")?.addEventListener("click", async () => {
    try {
      const picked = await invoke<string | null>("pick_file_dialog");
      if (picked) {
        const parts = picked.split(/[/\\]/);
        const name = parts[parts.length - 1];
        const ext = name.includes(".") ? name.split(".").pop() || "" : "";
        void openFile(picked, name, ext);
      }
    } catch (e) {
      console.error(e);
    }
  });

  document.getElementById("save-file-btn")?.addEventListener("click", () => {
    void saveCurrentFile();
  });

  document.getElementById("copy-btn")?.addEventListener("click", async () => {
    const btn = document.getElementById("copy-btn");
    try {
      await navigator.clipboard.writeText(editorInput.value);
      if (btn) {
        const prev = btn.textContent;
        btn.textContent = "✓ Copied!";
        setTimeout(() => { btn.textContent = prev; }, 1400);
      }
    } catch {}
  });

  // Theme Toggle (Light / Dark)
  document.getElementById("theme-btn")?.addEventListener("click", () => {
    const isLight = document.body.classList.toggle("light-theme");
    localStorage.setItem(THEME_KEY, isLight ? "light" : "dark");
  });

  // Fullscreen Toggle
  document.getElementById("fullscreen-btn")?.addEventListener("click", async () => {
    isFullscreen = !isFullscreen;
    try {
      await invoke("set_scratchpad_fullscreen", { fullscreen: isFullscreen });
    } catch {}
  });

  // New File in Current Directory
  document.getElementById("new-file-btn")?.addEventListener("click", async () => {
    if (!currentDir) return;
    const name = prompt("Enter new file name (e.g. index.js, notes.txt, style.css):");
    if (!name || !name.trim()) return;

    try {
      const createdPath = await invoke<string>("create_file", { parentDir: currentDir, name: name.trim() });
      const ext = name.includes(".") ? name.split(".").pop() || "" : "";
      void loadDirectory(currentDir);
      void openFile(createdPath, name.trim(), ext);
    } catch (err) {
      alert("Could not create file: " + err);
    }
  });

  // New Folder in Current Directory
  document.getElementById("new-dir-btn")?.addEventListener("click", async () => {
    if (!currentDir) return;
    const name = prompt("Enter new directory name:");
    if (!name || !name.trim()) return;

    try {
      await invoke("create_dir", { parentDir: currentDir, name: name.trim() });
      void loadDirectory(currentDir);
    } catch (err) {
      alert("Could not create directory: " + err);
    }
  });

  document.getElementById("refresh-dir-btn")?.addEventListener("click", () => {
    if (currentDir) void loadDirectory(currentDir);
  });

  // Sidebar toggle & auto-collapse
  const toggleSidebar = () => {
    sidebar.classList.toggle("collapsed");
  };

  document.getElementById("collapse-sidebar-btn")?.addEventListener("click", toggleSidebar);
  document.getElementById("toggle-sidebar-btn")?.addEventListener("click", toggleSidebar);

  // ── Stretchable / Resizable Sidebar with Auto-Close ─────────────────────────
  let isResizing = false;
  sidebarResizer.addEventListener("mousedown", (e) => {
    isResizing = true;
    sidebarResizer.classList.add("dragging");
    document.body.style.cursor = "col-resize";
    e.preventDefault();
  });

  window.addEventListener("mousemove", (e) => {
    if (!isResizing) return;
    const newWidth = e.clientX;

    // Auto-close if dragged below 100px
    if (newWidth < 100) {
      sidebar.classList.add("collapsed");
    } else {
      sidebar.classList.remove("collapsed");
      const clamped = Math.min(Math.max(newWidth, 180), 600);
      sidebar.style.width = `${clamped}px`;
      localStorage.setItem(SIDEBAR_WIDTH_KEY, String(clamped));
    }
  });

  window.addEventListener("mouseup", () => {
    if (isResizing) {
      isResizing = false;
      sidebarResizer.classList.remove("dragging");
      document.body.style.cursor = "";
    }
  });

  // ── Two-way Event Sync with Island ─────────────────────────────────────────
  try {
    await listen<string>("scratchpad-updated", (event) => {
      if (!currentFilePath && document.activeElement !== editorInput && event.payload !== editorInput.value) {
        editorInput.value = event.payload;
        try { localStorage.setItem(STORAGE_KEY, event.payload); } catch {}
        updateHighlight();
      }
    });
  } catch {}

  // Initial Load
  await openScratchpadDefault();
  await loadDirectory();
}

window.addEventListener("DOMContentLoaded", () => {
  void init();
});
