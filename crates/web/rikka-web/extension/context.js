(() => {
  const TYPED = new Set(["", "text", "search", "url", "tel", "email", "password", "number"]);

  const send = (message) => chrome.runtime.sendMessage(message).catch(() => {});

  const elements = (event) => event.composedPath().filter((node) => node instanceof Element);

  const address = (link) => {
    if (typeof link.href === "string") return link.href || null;
    const relative = link.href?.baseVal;
    if (!relative) return null;
    try {
      return new URL(relative, link.baseURI).href;
    } catch {
      return null;
    }
  };

  const linkIn = (path) => path.find((node) => (node.localName === "a" || node.localName === "area") && address(node) !== null);

  const focusedIn = (root) => {
    let active = root.activeElement;
    while (active?.shadowRoot?.activeElement) active = active.shadowRoot.activeElement;
    return active;
  };

  const editing = (element) => {
    if (element instanceof HTMLInputElement) {
      if (!TYPED.has(element.type) || element.readOnly || element.disabled) return null;
      const { selectionStart: start, selectionEnd: end } = element;
      const selected = start !== null && start !== end;
      const text = selected && element.type !== "password" ? element.value.slice(start, end) : "";
      return { editable: { inputType: element.type, selected }, text };
    }
    if (element instanceof HTMLTextAreaElement) {
      if (element.readOnly || element.disabled) return null;
      const { selectionStart: start, selectionEnd: end } = element;
      return { editable: { inputType: null, selected: start !== end }, text: element.value.slice(start, end) };
    }
    if (element.isContentEditable) {
      const selection = getSelection();
      return { editable: { inputType: null, selected: Boolean(selection) && !selection.isCollapsed }, text: String(selection ?? "") };
    }
    return null;
  };

  const describe = (event) => {
    const path = elements(event);
    const target = path[0] ?? null;
    const link = linkIn(path);
    const image = target instanceof HTMLImageElement && (target.currentSrc || target.src) ? { url: target.currentSrc || target.src, alt: target.alt } : null;
    const media =
      target instanceof HTMLMediaElement && (target.currentSrc || target.src)
        ? { url: target.currentSrc || target.src, kind: target instanceof HTMLVideoElement ? "video" : "audio" }
        : null;
    const edited = target ? editing(target) : null;
    return {
      link: link ? { url: address(link), text: (link.innerText ?? link.textContent ?? "").trim() } : null,
      image,
      media,
      selection: edited ? edited.text : String(getSelection() ?? ""),
      editable: edited?.editable ?? null,
    };
  };

  addEventListener(
    "contextmenu",
    (event) => {
      const context = describe(event);
      setTimeout(() => {
        if (!event.defaultPrevented) send({ rikka: "context", context });
      });
    },
    true,
  );

  let pointed = null;
  let focused = null;
  let reported = null;

  const report = () => {
    const url = pointed ?? focused;
    if (url === reported) return;
    reported = url;
    send({ rikka: "hover", url });
  };

  const linkAddress = (event) => {
    const link = linkIn(elements(event));
    return link ? address(link) : null;
  };

  addEventListener(
    "mouseover",
    (event) => {
      pointed = linkAddress(event);
      report();
    },
    true,
  );
  addEventListener(
    "mouseout",
    (event) => {
      if (event.relatedTarget !== null) return;
      pointed = null;
      report();
    },
    true,
  );
  addEventListener(
    "focusin",
    (event) => {
      const element = focusedIn(document);
      focused = element?.matches(":focus-visible") ? linkAddress(event) : null;
      report();
    },
    true,
  );
  addEventListener(
    "focusout",
    () => {
      focused = null;
      report();
    },
    true,
  );
  addEventListener("pagehide", () => {
    pointed = null;
    focused = null;
    report();
  });
})();
