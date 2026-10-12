// Notification-area icon: Open, Weekly recap, Wardrobe, Settings, Pause, Quit.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::i18n::{n_, t};
use crate::island::WINDOW_LABEL;

/// The menu's items, id and English label, in order. The labels are shown in
/// the interface language (i18n.rs) and follow it when it changes.
/// CUSTOMIZED: Added Toggle island, Open chat, Go to alert, Next/Previous pill, Focus tracker
const ITEMS: [(&str, &str); 13] = [
    ("open", n_("Open Coucou")),
    ("toggleIsland", n_("Toggle island")),
    ("openChat", n_("Open chat")),
    ("goToAlert", n_("Go to alert")),
    ("nextPill", n_("Next pill")),
    ("prevPill", n_("Previous pill")),
    ("recap", n_("Weekly recap")),
    ("wardrobe", n_("Wardrobe…")),
    ("settings", n_("Settings…")),
    ("focusTracker", n_("Focus tracker")),
    ("focusTrackerReset", n_("Reset focus timer")),
    ("pause", n_("Pause")),
    ("quit", n_("Quit")),
];

/// Kept so a language change relabels the menu without rebuilding the tray.
struct Items(Vec<(&'static str, MenuItem<Wry>)>);

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let mut items = Vec::new();
    for (id, label) in ITEMS {
        items.push((label, MenuItem::with_id(app, id, t(label), true, None::<&str>)?));
    }
    let [open, toggle_island, open_chat, go_to_alert, next_pill, prev_pill, recap, wardrobe, settings, focus_tracker, focus_tracker_reset, pause, quit] =
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12].map(|i| &items[i].1);
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(app, &[
        open, toggle_island, open_chat, go_to_alert, next_pill, prev_pill,
        &sep1,
        recap, wardrobe,
        &sep2,
        settings, focus_tracker, focus_tracker_reset, pause,
        &sep3,
        quit,
    ])?;
    app.manage(Items(items));

    let mut builder = TrayIconBuilder::with_id("coucou")
        .tooltip("Coucou")
        .menu(&menu)
        .on_menu_event(|app: &AppHandle, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "settings" => crate::show_settings_window(app),
            "focusTracker" => {
                let _ = app.emit_to(WINDOW_LABEL, "focus-tracker-toggle", "");
            }
            "focusTrackerReset" => {
                let _ = app.emit_to(WINDOW_LABEL, "focus-tracker-reset", "");
            }
            id => {
                let _ = app.emit_to(WINDOW_LABEL, "tray", id.to_string());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder.build(app)?;
    Ok(())
}

/// Relabels the menu in the current language.
pub fn retitle(app: &AppHandle) {
    let Some(items) = app.try_state::<Items>() else { return };
    for (label, item) in &items.0 {
        let _ = item.set_text(t(label));
    }
}
