// App classification - maps window class strings to app types.

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AppType {
    VsCode,
    Browser,
    Terminal,
    Other,
}

const VSCODE: &[&str] = &["code", "code-oss", "code-insiders", "vscodium", "cursor", "code_code"];
const BROWSER: &[&str] = &[
    "firefox",
    "org.mozilla.firefox",
    "google-chrome",
    "chromium",
    "brave-browser",
    "brave",
    "microsoft-edge",
    "vivaldi",
];
const TERMINAL: &[&str] = &[
    "konsole",
    "org.kde.konsole",
    "kitty",
    "alacritty",
    "gnome-terminal",
    "org.gnome.terminal",
    "wezterm",
    "foot",
];

pub fn classify(class: &str) -> AppType {
    let c = class.trim().to_lowercase();
    if VSCODE.contains(&c.as_str()) {
        return AppType::VsCode;
    }
    if BROWSER.contains(&c.as_str()) {
        return AppType::Browser;
    }
    if TERMINAL.contains(&c.as_str()) {
        return AppType::Terminal;
    }
    AppType::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_apps() {
        assert_eq!(classify("code"), AppType::VsCode);
        assert_eq!(classify("Code"), AppType::VsCode);
        assert_eq!(classify("org.mozilla.firefox"), AppType::Browser);
        assert_eq!(classify("org.kde.konsole"), AppType::Terminal);
        assert_eq!(classify("dolphin"), AppType::Other);
        assert_eq!(classify(""), AppType::Other);
    }
}
