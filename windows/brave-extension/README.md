# Coucou Browser Intent — Brave Extension (Manifest V3)

This extension reports **only the active tab's domain hostname** (never paths, query strings, or full URLs) to Coucou's local token-protected endpoint at `http://127.0.0.1:49172/api/browser/domain`.

## How to Load Unpacked in Brave (3 Simple Steps)
1. Open Brave and navigate to `brave://extensions`.
2. Toggle **Developer mode** ON (top-right corner).
3. Click **Load unpacked** (top-left button) and select this folder:
   ```
   /run/media/om-raut/New Volume/COUCOU/coucou_old/windows/brave-extension
   ```

## Token & Editable Domain Rules
- **Token**: Coucou automatically writes its local token (`~/.config/coucou/browser-token.txt`) into `brave-extension/config.js` on startup, so the extension works immediately without manual copy-pasting.
- **Editable Domain Rules**: Edit either of these files at any time (changes take effect immediately on the next tab switch without restarting Coucou):
  - `/run/media/om-raut/New Volume/COUCOU/coucou_old/windows/config/domain-rules.json`
  - `~/.config/coucou/domain-rules.json`
