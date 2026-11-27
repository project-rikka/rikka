(() => {
  const functions = (globalThis[Symbol.for("rikka")] ??= {});
  functions["probe.title"] = () => document.title;
  functions["probe.post"] = (name, body) => {
    chrome.runtime.sendMessage({ rikka: "post", name, body });
    return true;
  };
  functions["probe.ask"] = async (name, body) => chrome.runtime.sendMessage({ rikka: "ask", name, body });
  functions["probe.load"] = async (script) => {
    const loaded = await chrome.runtime.sendMessage({ rikka: "load", script });
    return { loaded, value: globalThis[Symbol.for("rikka")]["loaded.value"]?.() ?? null };
  };
  functions["probe.select"] = (id) => {
    const range = document.createRange();
    range.selectNodeContents(document.getElementById(id));
    getSelection().removeAllRanges();
    getSelection().addRange(range);
    return true;
  };
  functions["probe.contextmenu"] = (id) => {
    const target = document.getElementById(id);
    target.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, composed: true }));
    return true;
  };
  functions["probe.hover"] = (id) => {
    document.getElementById(id).dispatchEvent(new MouseEvent("mouseover", { bubbles: true, composed: true }));
    return true;
  };
  functions["probe.focus"] = (id) => {
    document.getElementById(id).focus();
    return true;
  };
  functions["probe.value"] = (id) => document.getElementById(id).value;
})();
