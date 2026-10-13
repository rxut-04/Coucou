// Coucou Focus Tracker - KWin script
// Reports active window changes to Coucou via DBus

const DBUS_SERVICE = "com.coucou.FocusTracker";
const DBUS_PATH = "/com/coucou/FocusTracker";
const DBUS_INTERFACE = "com.coucou.FocusTracker";

// Find the KWin workspace object (handles both Plasma 5 and 6)
function getWorkspace() {
    if (typeof workspace !== "undefined") {
        return workspace;
    }
    // Try to find it via the global object
    if (typeof kwin !== "undefined" && typeof kwin.workspace !== "undefined") {
        return kwin.workspace;
    }
    return null;
}

// Connect to DBus and call the focusChanged method
function reportFocusChange(window) {
    if (!window) return;

    const resourceClass = window.resourceClass || "unknown";
    const caption = window.caption || "unknown";
    const pid = window.pid || 0;

    // Call the DBus method using Qt
    const bus = QDBusConnection.sessionBus();
    if (!bus.isConnected()) return;

    const message = QDBusMessage.createMethodCall(
        DBUS_SERVICE,
        DBUS_PATH,
        DBUS_INTERFACE,
        "focusChanged"
    );
    message << resourceClass << caption << pid;

    bus.call(message, QDBus::NoBlock);
}

// Main initialization
function init() {
    const ws = getWorkspace();
    if (!ws) {
        console.log("Coucou Focus Tracker: Could not find KWin workspace object");
        return;
    }

    // Plasma 6 signals
    if (typeof ws.windowActivated !== "undefined") {
        ws.windowActivated.connect(reportFocusChange);
    } else if (typeof ws.activeWindow !== "undefined") {
        ws.activeWindow.connect(reportFocusChange);
    }

    // Plasma 5 signals (fallback)
    if (typeof ws.clientActivated !== "undefined") {
        ws.clientActivated.connect(reportFocusChanged);
    } else if (typeof ws.activeClient !== "undefined") {
        ws.activeClient.connect(reportFocusChange);
    }

    // Also track current window changes
    if (typeof ws.windowChanged !== "undefined") {
        ws.windowChanged.connect(function(window) {
            reportFocusChange(window);
        });
    }

    console.log("Coucou Focus Tracker: Initialized");
}

init();
