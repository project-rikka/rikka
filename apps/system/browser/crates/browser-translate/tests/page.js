(() => {
  const functions = (globalThis[Symbol.for("rikka")] ??= {});
  const REACHED = "browser-translate-test.reached";

  const read = (element, query) => {
    switch (query.read) {
      case "html":
        return element.innerHTML;
      case "outer":
        return element.outerHTML;
      case "text":
        return element.textContent;
      case "value":
        return element.value;
      case "attribute":
        return element.getAttribute(query.attribute);
      case "code":
        return Array.from(element.querySelectorAll("[data-code-text]"), (span) => span.getAttribute("data-code-text")).join("");
      default:
        throw new Error(`unknown read ${query.read}`);
    }
  };

  const value = (query) => {
    const root = query.frame ? document.querySelector(query.frame).contentDocument : document;
    if (query.read === "title") return root.title;
    if (query.all) return Array.from(root.querySelectorAll(query.select), (element) => read(element, query));
    return read(query.select ? root.querySelector(query.select) : root.body, query);
  };

  functions["test.read"] = (query) => value(query);

  functions["test.until"] = (token, query, expected) => {
    const wanted = JSON.stringify(expected);
    const root = query.frame ? document.querySelector(query.frame).contentDocument : document;
    const observer = new MutationObserver(() => check());
    const check = () => {
      let found;
      try {
        found = value(query);
      } catch (error) {
        return false;
      }
      if (JSON.stringify(found) !== wanted) return false;
      observer.disconnect();
      chrome.runtime.sendMessage({ rikka: "post", name: REACHED, body: token }).catch(() => {});
      return true;
    };
    if (!check()) observer.observe(root, { subtree: true, childList: true, characterData: true, attributes: true });
    return true;
  };

  functions["test.translator"] = () => typeof globalThis[Symbol.for("rikka")]["browser-translate.translator"] === "function";

  functions["test.push"] = (path) => {
    history.pushState({}, "", path);
    return true;
  };

  functions["test.replace"] = (select, html) => {
    document.querySelector(select).innerHTML = html;
    return true;
  };

  functions["test.remember"] = () => {
    globalThis.rikkaRemembered = true;
    return true;
  };

  functions["test.remembered"] = () => globalThis.rikkaRemembered === true;
})();
