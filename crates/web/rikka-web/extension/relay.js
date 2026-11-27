(() => {
  const MARK = "rikka-message";
  addEventListener("message", (event) => {
    if (event.source !== window || typeof event.data !== "object" || event.data === null) return;
    const name = event.data[MARK];
    if (typeof name !== "string") return;
    chrome.runtime.sendMessage({ rikka: "post", name, body: event.data.body ?? null, world: "page" }).catch(() => {});
  });
})();
