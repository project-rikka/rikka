const HOST = "os.rikka.web";
const PROTOCOL = 1;
const HOST_MISSING = "Specified native messaging host not found.";
const RECONNECT_FIRST_MS = 1000;
const RECONNECT_LONGEST_MS = 60000;

const NAMESPACES = new Set([
  "bookmarks",
  "browsingData",
  "cookies",
  "downloads",
  "history",
  "identity",
  "management",
  "privacy",
  "scripting",
  "sessions",
  "tabs",
  "webNavigation",
  "windows",
]);

const WINDOW_TYPES = { windowTypes: ["normal", "popup", "devtools"] };

const EVENTS = [
  "bookmarks.onChanged",
  "bookmarks.onChildrenReordered",
  "bookmarks.onCreated",
  "bookmarks.onMoved",
  "bookmarks.onRemoved",
  "downloads.onChanged",
  "downloads.onCreated",
  "downloads.onErased",
  "history.onVisitRemoved",
  "history.onVisited",
  "identity.onSignInChanged",
  "management.onDisabled",
  "management.onEnabled",
  "management.onInstalled",
  "management.onUninstalled",
  "tabs.onActivated",
  "tabs.onCreated",
  "tabs.onRemoved",
  "tabs.onReplaced",
  "tabs.onUpdated",
  "tabs.onZoomChange",
  "webNavigation.onBeforeNavigate",
  "webNavigation.onCommitted",
  "webNavigation.onCompleted",
  "webNavigation.onErrorOccurred",
  "webNavigation.onHistoryStateUpdated",
  "webNavigation.onReferenceFragmentUpdated",
  "windows.onCreated",
  "windows.onRemoved",
];

const FILTERED = new Set(["windows.onCreated", "windows.onRemoved"]);

const BUNDLED = /^[\w-]+\/[\w-]+(?:\.[\w-]+)*\.js$/;

const EDITS = new Set(["undo", "redo", "cut", "copy", "selectAll", "delete", "insertText"]);

let port = null;
let nextAsk = 1;
let reconnectDelay = RECONNECT_FIRST_MS;
const asks = new Map();

const resolve = (path) => {
  let owner = chrome;
  for (const part of path.slice(0, -1)) owner = owner?.[part];
  return [owner, owner?.[path.at(-1)]];
};

const send = (message) => {
  if (port !== null) port.postMessage(message);
};

const ask = (kind, args, settle) => {
  if (port === null) {
    settle(null);
    return;
  }
  const id = nextAsk++;
  asks.set(id, settle);
  send({ ask: id, kind, args });
};

const special = {
  async run({ target, world, files, invoke, args }) {
    if (files?.length) await chrome.scripting.executeScript({ target, world, files });
    if (!invoke) return [];
    const results = await chrome.scripting.executeScript({
      target,
      world,
      args: [invoke, args ?? []],
      func: (name, values) => {
        const run = globalThis[Symbol.for("rikka")]?.[name];
        if (typeof run !== "function") throw new Error(`${name} is not loaded`);
        return run(...values);
      },
    });
    return results.map(({ frameId, result }) => ({ frame: frameId, value: result ?? null }));
  },

  async register(registrations) {
    const ids = registrations.map((registration) => registration.id);
    const known = await chrome.scripting.getRegisteredContentScripts({ ids });
    if (known.length > 0) await chrome.scripting.unregisterContentScripts({ ids: known.map((script) => script.id) });
    await chrome.scripting.registerContentScripts(registrations);
  },

  async control({ tab, action, delta }) {
    const controls = {
      print: () => print(),
      stop: () => stop(),
      go: (steps) => history.go(steps),
      "exit-fullscreen": () => document.fullscreenElement && document.exitFullscreen(),
    };
    const func = controls[action];
    if (func === undefined) throw new Error(`unknown control ${action}`);
    await chrome.scripting.executeScript({ target: { tabId: tab }, world: "MAIN", func, args: [delta] });
  },

  async travel({ tab, delta }) {
    try {
      await (delta < 0 ? chrome.tabs.goBack(tab) : chrome.tabs.goForward(tab));
    } catch {
      await special.control({ tab, action: "go", delta });
    }
  },

  async favicon({ page, size }) {
    const url = new URL(chrome.runtime.getURL("/_favicon/"));
    url.searchParams.set("pageUrl", page);
    url.searchParams.set("size", String(size));
    const response = await fetch(url);
    const bytes = new Uint8Array(await response.arrayBuffer());
    let binary = "";
    for (let at = 0; at < bytes.length; at += 0x8000) {
      binary += String.fromCharCode(...bytes.subarray(at, at + 0x8000));
    }
    return { type: response.headers.get("content-type") ?? "image/png", data: btoa(binary) };
  },

  async edit({ tab, frame, command, text }) {
    if (!EDITS.has(command)) throw new Error(`unknown edit ${command}`);
    const [edited] = await chrome.scripting.executeScript({
      target: { tabId: tab, frameIds: [frame] },
      func: (name, value) => document.execCommand(name, false, value ?? undefined),
      args: [command, text ?? null],
    });
    return edited?.result ?? false;
  },

  async selection({ tab, frame }) {
    const [read] = await chrome.scripting.executeScript({
      target: { tabId: tab, frameIds: [frame] },
      func: () => {
        let active = document.activeElement;
        while (active?.shadowRoot?.activeElement) active = active.shadowRoot.activeElement;
        if (active instanceof HTMLInputElement && active.type === "password") return "";
        if (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement) {
          const { selectionStart: start, selectionEnd: end } = active;
          if (start !== null && start !== end) return active.value.slice(start, end);
        }
        return String(getSelection() ?? "");
      },
    });
    return read?.result ?? "";
  },
};

const load = async (sender, script) => {
  if (typeof script !== "string" || !BUNDLED.test(script)) throw new Error("not a bundled script");
  const tabId = sender.tab?.id;
  if (tabId === undefined) throw new Error("not sent from a tab");
  const target = sender.documentId ? { tabId, documentIds: [sender.documentId] } : { tabId, frameIds: [sender.frameId ?? 0] };
  await chrome.scripting.executeScript({ target, files: [`scripts/${script}`] });
};

const call = async (api, args) => {
  const path = api.split(".");
  if (path[0] === "rikka") {
    const run = special[path[1]];
    if (typeof run !== "function") throw new Error(`unknown function ${api}`);
    return run(...args);
  }
  if (!NAMESPACES.has(path[0])) throw new Error(`${api} is not allowed`);
  const [owner, run] = resolve(path);
  if (typeof run !== "function") throw new Error(`${api} does not exist`);
  return run.apply(owner, args);
};

const received = (message) => {
  reconnectDelay = RECONNECT_FIRST_MS;
  if ("call" in message) {
    call(message.api, message.args ?? [])
      .then((value) => send({ reply: message.call, value: value ?? null }))
      .catch((error) => send({ reply: message.call, error: String(error?.message ?? error) }));
  } else if ("answer" in message) {
    const settle = asks.get(message.answer);
    asks.delete(message.answer);
    settle?.(message.value ?? null);
  }
};

const disconnected = () => {
  const missing = chrome.runtime.lastError?.message === HOST_MISSING;
  port = null;
  for (const settle of asks.values()) settle(null);
  asks.clear();
  if (missing) return;
  setTimeout(connect, reconnectDelay);
  reconnectDelay = Math.min(reconnectDelay * 2, RECONNECT_LONGEST_MS);
};

const connect = () => {
  port = chrome.runtime.connectNative(HOST);
  port.onMessage.addListener(received);
  port.onDisconnect.addListener(disconnected);
  send({
    hello: {
      protocol: PROTOCOL,
      version: chrome.runtime.getManifest().version,
    },
  });
};

for (const name of EVENTS) {
  const [, event] = resolve(name.split("."));
  const forward = (...args) => send({ event: name, args });
  if (FILTERED.has(name)) event.addListener(forward, WINDOW_TYPES);
  else event.addListener(forward);
}

chrome.runtime.onMessage.addListener((message, sender, respond) => {
  if (sender.id !== chrome.runtime.id || typeof message !== "object" || message === null) return;
  const origin = {
    tab: sender.tab?.id ?? null,
    frame: sender.frameId ?? 0,
    url: sender.url ?? null,
  };
  if (message.rikka === "page") {
    send({ event: "rikka.page", args: [origin, message.state] });
  } else if (message.rikka === "post") {
    send({ event: "rikka.post", args: [origin, message.name, message.body ?? null, message.world ?? "isolated"] });
  } else if (message.rikka === "context") {
    send({ event: "rikka.context", args: [origin, message.context ?? null] });
  } else if (message.rikka === "hover") {
    send({ event: "rikka.hover", args: [origin, message.url ?? null] });
  } else if (message.rikka === "ask") {
    ask("message", [origin, message.name, message.body ?? null], respond);
    return true;
  } else if (message.rikka === "load") {
    load(sender, message.script).then(
      () => respond(true),
      () => respond(false),
    );
    return true;
  }
});

chrome.runtime.onUpdateAvailable.addListener(() => chrome.runtime.reload());
chrome.runtime.requestUpdateCheck().catch(() => {});

connect();
