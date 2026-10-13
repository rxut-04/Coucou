// Coucou Focus Tracker - KWin script
// Reports active window changes and the live window snapshot to Coucou via DBus.

function normalizeWindow(w, activeWindow) {
    if (!w) return null;
    const className = String(w.resourceClass || w.windowClass || "");
    const caption = String(w.caption || w.title || "");
    const pid = Number(w.pid || 0);
    const id = w.internalId !== undefined && w.internalId !== null
        ? w.internalId.toString()
        : `${pid}:${className}:${caption}`;
    return {
        id,
        resourceClass: className,
        caption,
        pid,
        active: Boolean(w.active || (activeWindow && activeWindow.internalId &&
            String(activeWindow.internalId) === id)),
        minimized: Boolean(w.minimized || false),
    };
}

function toWindowArray(source) {
    if (Array.isArray(source)) return source;
    if (source && typeof source.toArray === "function") {
        const array = source.toArray();
        if (Array.isArray(array)) return array;
    }
    const length = source && typeof source.length === "number"
        ? source.length
        : (source && typeof source.count === "function" ? Number(source.count()) : -1);
    if (!Number.isInteger(length) || length < 0) return [];
    const array = [];
    for (let index = 0; index < length; index++) {
        const window = source[index];
        if (window) {
            watchWindow(window);
            array.push(window);
        }
    }
    return array;
}

let emptyListDiagnostics = 0;

function listWindows() {
    let source;
    let sourceName;
    if (typeof workspace.windowList === "function") {
        source = workspace.windowList();
        sourceName = "windowList()";
    } else if (workspace.stackingOrder !== undefined) {
        source = workspace.stackingOrder;
        sourceName = "stackingOrder";
    } else if (typeof workspace.clientList === "function") {
        source = workspace.clientList();
        sourceName = "clientList()";
    } else {
        source = workspace.windows || [];
        sourceName = "windows";
    }
    const list = toWindowArray(source);
    if (list.length === 0 && emptyListDiagnostics < 3) {
        const sourceLength = source && (source.length ?? (typeof source.count === "function" ? source.count() : "unknown"));
        print(`[CoucouFocus] empty window source=${sourceName} type=${typeof source} array=${Array.isArray(source)} length=${sourceLength}`);
        emptyListDiagnostics++;
    }
    const activeWindow = workspace.activeWindow || workspace.activeClient || null;
    return list.map((window) => normalizeWindow(window, activeWindow)).filter(Boolean);
}

function connectSignal(target, signalName, callback) {
    const signal = target && target[signalName];
    if (signal && typeof signal.connect === "function") signal.connect(callback);
}

const watchedWindows = Object.create(null);

function watchWindow(window) {
    if (!window) return;
    const id = window.internalId && typeof window.internalId.toString === "function"
        ? window.internalId.toString()
        : `${window.pid || 0}:${window.resourceClass || ""}:${window.caption || ""}`;
    if (watchedWindows[id]) return;
    watchedWindows[id] = true;
    connectSignal(window, "captionChanged", sendSnapshot);
    connectSignal(window, "minimizedChanged", sendSnapshot);
}

function report(w) {
    if (!w) return;
    callDBus(
        "com.coucou.FocusTracker",
        "/com/coucou/FocusTracker",
        "com.coucou.FocusTracker",
        "WindowChanged",
        String(w.resourceClass || w.windowClass || ""),
        String(w.caption || w.title || ""),
        String(w.pid || 0)
    );
    callDBus(
        "com.coucou.FocusTracker",
        "/com/coucou/FocusTracker",
        "com.coucou.FocusTracker",
        "WindowsSnapshot",
        JSON.stringify(listWindows())
    );
}

function sendSnapshot() {
    const snapshot = listWindows();
    callDBus(
        "com.coucou.FocusTracker",
        "/com/coucou/FocusTracker",
        "com.coucou.FocusTracker",
        "WindowsSnapshot",
        JSON.stringify(snapshot)
    );
}

function windowAdded(window) {
    watchWindow(window);
    sendSnapshot();
}

connectSignal(workspace, "windowActivated", report);
connectSignal(workspace, "windowAdded", windowAdded);
connectSignal(workspace, "windowRemoved", sendSnapshot);
connectSignal(workspace, "clientActivated", report);
connectSignal(workspace, "clientAdded", windowAdded);
connectSignal(workspace, "clientRemoved", sendSnapshot);

const activeWindow = workspace.activeWindow || workspace.activeClient || null;
if (activeWindow) report(activeWindow);
else sendSnapshot();
