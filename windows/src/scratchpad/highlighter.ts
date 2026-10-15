// Non-destructive lexical tokenizer for syntax highlighting.
// Processes source text sequentially in a single pass without mutating already-injected HTML spans.
// Correctly highlights code in JS/TS/JSX, Python, Rust, Bash, and mixed text files/notes.

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

export function highlightCode(raw: string, _ext: string = "txt"): string {
  if (!raw) return "&nbsp;";
  const lines = raw.split("\n");

  // Single-pass Tokenizer Regex:
  // 1: Comments: // ... or /* ... */ or # ...
  // 2: Strings: "...", '...', `...`
  // 3: JSX / HTML tags: <tag>, </tag>, <tag ...>, <>, </>
  // 4: Keywords: import, export, from, default, function, etc.
  // 5: Function calls: name(...)
  // 6: Numbers: integers, floats, hex
  // 7: Operators & punctuation: = + - * / % ! ? : ; ,
  const tokenRegex = /(\/\/[^\n]*|\/\*[\s\S]*?\*\/|(?:^|\s)#[^\n]*)|("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`)|(<\/?[a-zA-Z0-9_$-]*(?:\s+[^>]*)?>|<>\s*|<\/\s*>)|(\b(?:import|export|from|default|return|function|const|let|var|if|else|switch|case|break|continue|for|while|do|try|catch|finally|throw|new|class|extends|async|await|true|false|null|undefined|this|typeof|instanceof|interface|type|enum|pub|fn|struct|impl|mut|use|mod|def|echo|select|where|package|public|private|protected|static|void|require|module|yield)\b)|(\b[a-zA-Z_$][a-zA-Z0-9_$]*(?=\s*\())|(\b\d+(?:\.\d+)?\b|0x[0-9a-fA-F]+\b)|([=+\-*\/%!?:;])/g;

  return lines.map((line) => {
    let lastIndex = 0;
    let match: RegExpExecArray | null;
    let out = "";
    tokenRegex.lastIndex = 0;

    while ((match = tokenRegex.exec(line)) !== null) {
      if (match.index > lastIndex) {
        out += escapeHtml(line.slice(lastIndex, match.index));
      }
      const [full, comment, str, tag, keyword, fn, num, op] = match;
      const escaped = escapeHtml(full);

      if (comment) out += `<span class="hl-cmt">${escaped}</span>`;
      else if (str) out += `<span class="hl-str">${escaped}</span>`;
      else if (tag) out += `<span class="hl-tag">${escaped}</span>`;
      else if (keyword) out += `<span class="hl-kw">${escaped}</span>`;
      else if (fn) out += `<span class="hl-fn">${escaped}</span>`;
      else if (num) out += `<span class="hl-num">${escaped}</span>`;
      else if (op) out += `<span class="hl-op">${escaped}</span>`;
      else out += escaped;

      lastIndex = tokenRegex.lastIndex;
    }

    if (lastIndex < line.length) {
      out += escapeHtml(line.slice(lastIndex));
    }
    return out;
  }).join("\n");
}
