// Client-side syntax highlighter producing VS Code Dark+/Light+ token spans
// Supports JavaScript, TypeScript, JSX, HTML, CSS, JSON, Rust, Python, Bash, and generic text.

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

export function highlightCode(code: string, ext: string): string {
  if (!code) return "&nbsp;";
  const extension = (ext || "").toLowerCase().replace(/^\./, "");

  // Text/Markdown files: highlight headers, lists, links, backticks
  if (extension === "txt" || extension === "md") {
    let out = escapeHtml(code);
    out = out.replace(/(#+ [^\n]+)/g, '<span class="hl-kw">$1</span>');
    out = out.replace(/(`[^`]+`)/g, '<span class="hl-str">$1</span>');
    out = out.replace(/(https?:\/\/[^\s]+)/g, '<span class="hl-tag">$1</span>');
    return out;
  }

  // General Programming languages (JS, TS, JSON, Rust, Python, Bash, CSS, HTML)
  let result = escapeHtml(code);

  // 1. Single and multi-line Comments
  // Strings and comments first to prevent keywords inside them from being highlighted
  result = result.replace(/(\/\/[^\n]*|\/\*[\s\S]*?\*\/|#[^\n]*)/g, '<span class="hl-cmt">$1</span>');

  // 2. Strings (single, double, template literals)
  result = result.replace(/(&quot;[\s\S]*?&quot;|&#039;[\s\S]*?&#039;|`[\s\S]*?`)/g, '<span class="hl-str">$1</span>');

  // 3. Numbers (integers, floats, hex)
  result = result.replace(/\b(\d+(\.\d+)?|0x[0-9a-fA-F]+)\b/g, '<span class="hl-num">$1</span>');

  // 4. Keywords
  const keywords = [
    "import", "export", "from", "default", "return", "function", "const", "let", "var",
    "if", "else", "switch", "case", "break", "continue", "for", "while", "do",
    "try", "catch", "finally", "throw", "new", "class", "extends", "async", "await",
    "true", "false", "null", "undefined", "this", "typeof", "instanceof", "interface",
    "type", "enum", "pub", "fn", "struct", "impl", "mut", "use", "mod", "def"
  ];
  const kwRegex = new RegExp(`\\b(${keywords.join("|")})\\b`, "g");
  result = result.replace(kwRegex, '<span class="hl-kw">$1</span>');

  // 5. Function calls: e.g. defineConfig(, react(
  result = result.replace(/\b([a-zA-Z_$][a-zA-Z0-9_$]*)(?=\s*\()/g, '<span class="hl-fn">$1</span>');

  // 6. JSX/HTML tags: e.g. &lt;div, &lt;/div&gt;
  result = result.replace(/(&lt;\/?[a-zA-Z0-9_$-]+)/g, '<span class="hl-tag">$1</span>');

  // 7. Common punctuation / Operators: = + - * / % & | ! ? : ; ,
  result = result.replace(/([=&gt;&lt;+\-*\/%!?:;])/g, '<span class="hl-op">$1</span>');

  return result;
}
