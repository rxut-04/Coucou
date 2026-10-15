// Client-side syntax highlighter producing VS Code Dark+/Light+ token spans.
// Supports JavaScript, TypeScript, JSX, HTML, CSS, JSON, Rust, Python, Bash,
// and intelligent code snippet detection inside .txt and notes!

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

export function highlightCode(code: string, ext: string = "txt"): string {
  if (!code) return "&nbsp;";
  const extension = (ext || "").toLowerCase().replace(/^\./, "");

  // If file is Markdown or explicitly requested, format fenced blocks ```...``` specially
  if (extension === "md" && code.includes("```")) {
    return highlightMarkdownFences(code);
  }

  // Universal Code & Snippet Highlighting:
  // Apply full VS Code rich highlighting across all files (including .txt files with code snippets).
  let result = escapeHtml(code);

  // 1. Comments: // line comments, /* block comments */, and # hash comments (Python/Bash)
  result = result.replace(/(\/\/[^\n]*|\/\*[\s\S]*?\*\/|#[^\n]*)/g, '<span class="hl-cmt">$1</span>');

  // 2. Strings: double quotes, single quotes, backticks
  result = result.replace(/(&quot;[\s\S]*?&quot;|&#039;[\s\S]*?&#039;|`[\s\S]*?`)/g, '<span class="hl-str">$1</span>');

  // 3. Numbers: integers, decimals, hex
  result = result.replace(/\b(\d+(\.\d+)?|0x[0-9a-fA-F]+)\b/g, '<span class="hl-num">$1</span>');

  // 4. Keywords (JS, TS, React, Python, Rust, Bash, Go, C/C++)
  const keywords = [
    "import", "export", "from", "default", "return", "function", "const", "let", "var",
    "if", "else", "switch", "case", "break", "continue", "for", "while", "do",
    "try", "catch", "finally", "throw", "new", "class", "extends", "async", "await",
    "true", "false", "null", "undefined", "this", "typeof", "instanceof", "interface",
    "type", "enum", "pub", "fn", "struct", "impl", "mut", "use", "mod", "def",
    "echo", "select", "where", "from", "package", "public", "private", "protected",
    "static", "void", "require", "module", "yield"
  ];
  const kwRegex = new RegExp(`\\b(${keywords.join("|")})\\b`, "g");
  result = result.replace(kwRegex, '<span class="hl-kw">$1</span>');

  // 5. Function calls: e.g. defineConfig(, react(, console.log(, print(
  result = result.replace(/\b([a-zA-Z_$][a-zA-Z0-9_$]*)(?=\s*\()/g, '<span class="hl-fn">$1</span>');

  // 6. JSX/HTML tags: e.g. <div, </span, <Route, etc.
  result = result.replace(/(&lt;\/?[a-zA-Z0-9_$-]+)/g, '<span class="hl-tag">$1</span>');

  // 7. Operators & punctuation
  result = result.replace(/([=&gt;&lt;+\-*\/%!?:;])/g, '<span class="hl-op">$1</span>');

  return result;
}

function highlightMarkdownFences(md: string): string {
  const parts = md.split(/(```[\s\S]*?```)/g);
  return parts.map((part) => {
    if (part.startsWith("```") && part.endsWith("```")) {
      const inner = part.slice(3, -3);
      const firstLineEnd = inner.indexOf("\n");
      const codeOnly = firstLineEnd !== -1 ? inner.slice(firstLineEnd + 1) : inner;
      return `<span class="hl-cmt">\`\`\`</span>\n${highlightCode(codeOnly, "js")}\n<span class="hl-cmt">\`\`\`</span>`;
    }
    return escapeHtml(part);
  }).join("");
}
