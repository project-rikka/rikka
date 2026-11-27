(async () => {
  const DOCUMENT = "browser-reader.document";
  const parts = [];
  for (let index = 0; ; index += 1) {
    const part = await chrome.runtime.sendMessage({ rikka: "ask", name: DOCUMENT, body: { index } });
    if (typeof part?.text !== "string") return;
    parts.push(part.text);
    if (!part.more) break;
  }
  document.open();
  document.write(parts.join(""));
  document.close();
})();
