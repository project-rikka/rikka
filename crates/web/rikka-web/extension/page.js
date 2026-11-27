(() => {
  const STEPS = new Set(["push", "replace", "reload", "traverse"]);

  const report = (step) => {
    const navigation = globalThis.navigation;
    const index = navigation?.currentEntry?.index ?? -1;
    const count = navigation?.entries().length ?? 0;
    const state = {
      step: STEPS.has(step) ? step : null,
      url: location.href,
      length: history.length,
      back: Math.max(index, 0),
      forward: index < 0 ? 0 : Math.max(count - 1 - index, 0),
      fullscreen: document.fullscreenElement !== null,
    };
    chrome.runtime.sendMessage({ rikka: "page", state }).catch(() => {});
  };

  globalThis.navigation?.addEventListener("currententrychange", (event) => report(event.navigationType));
  document.addEventListener("fullscreenchange", () => report(null));
  addEventListener("pageshow", (event) => {
    if (event.persisted) report("traverse");
  });
  chrome.runtime.onMessage.addListener((message) => {
    if (message?.rikka === "report") report(null);
  });
  report(globalThis.navigation?.activation?.navigationType ?? null);
})();
