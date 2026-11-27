(() => {
  "use strict";

  const NAME = "browser-translate";
  const TRANSLATOR = "browser-translate/translator.js";
  const FACTORY = "browser-translate.translator";
  const TOP = window.top === window;
  const SAMPLE_CHARS = 4000;
  const SHORT_SAMPLE = 200;
  const NAVIGATION_REPORT_MS = 800;
  const RESAMPLE_QUIET_MS = 400;
  const REWATCH_DELAY_MS = 1000;
  const LETTER = /\p{L}/u;
  const SPACES = /\s+/g;
  const INVISIBLE = new Set(["script", "style", "noscript", "template", "link", "meta", "base", "head", "title", "slot", "wbr", "param", "source", "track", "datalist", "area"]);
  const OPAQUE = new Set(["img", "picture", "svg", "math", "canvas", "video", "audio", "iframe", "frame", "object", "embed", "input", "select", "textarea", "meter", "progress", "map", "applet", "model"]);
  const KEPT = new Set(["kbd", "samp", "var", "tt"]);
  const EDITORS = ".CodeMirror, .cm-editor, .monaco-editor, .ace_editor";
  const CODE_SURE = ".react-code-file-contents, .react-code-lines, .blob-wrapper, .blob-code-inner, table.highlight, table.diff-table, .diff-text-cell, .react-file-line";
  const CODE_HINT = ".highlight, .hljs, .shiki, .prism-code, .sourceCode, .codehilite, [class*=language-], [class*=lang-], [class*=highlight-source-], [class*=highlight-text-]";

  const send = (message) => {
    try {
      return chrome.runtime.sendMessage(message);
    } catch (error) {
      return Promise.reject(error);
    }
  };
  const post = (body) => send({ rikka: "post", name: NAME, body }).catch(() => {});
  const ask = (body) => send({ rikka: "ask", name: NAME, body });
  const alive = () => Boolean(globalThis.chrome?.runtime?.id);
  const host = { ask, top: TOP, LETTER, SPACES, INVISIBLE, OPAQUE, KEPT, EDITORS, CODE_SURE, CODE_HINT };
  let translator = null;

  const language = () => {
    const lang = document.documentElement.getAttribute("lang");
    if (lang && lang.trim()) return lang.trim();
    const meta = document.querySelector("meta[http-equiv='content-language' i]");
    return meta ? (meta.getAttribute("content") || "").split(",")[0].trim() : "";
  };

  const sample = () => {
    const parts = [];
    let length = 0;
    const visit = (node) => {
      for (let child = node.firstChild; child && length < SAMPLE_CHARS; child = child.nextSibling) {
        if (child.nodeType === Node.TEXT_NODE) {
          const text = child.data.replace(SPACES, " ").trim();
          if (text && LETTER.test(text)) {
            parts.push(text);
            length += text.length + 1;
          }
          continue;
        }
        if (child.nodeType !== Node.ELEMENT_NODE) continue;
        const tag = child.localName;
        if (INVISIBLE.has(tag) || OPAQUE.has(tag) || KEPT.has(tag) || tag === "pre" || tag === "code" || tag === "rikka-translation" || child.hidden) continue;
        if (child.classList.length && child.matches(`${EDITORS}, ${CODE_SURE}, ${CODE_HINT}`)) continue;
        if (child.firstChild && typeof child.checkVisibility === "function" && !child.checkVisibility()) continue;
        visit(child);
        if (child.shadowRoot) visit(child.shadowRoot);
      }
    };
    if (document.body) visit(document.body);
    return parts.join(" ").slice(0, SAMPLE_CHARS);
  };

  const report = () => {
    const text = sample();
    post({ kind: "page", lang: language(), sample: text });
    return text;
  };

  let resampler = null;

  const stopResampling = () => {
    if (!resampler) return;
    resampler.observer.disconnect();
    clearTimeout(resampler.timer);
    resampler = null;
  };

  const resample = () => {
    const current = resampler;
    current.timer = 0;
    const lang = language();
    const text = sample();
    if (text === current.text && lang === current.lang) return;
    current.text = text;
    current.lang = lang;
    post({ kind: "page", lang, sample: text });
    if (text.length >= SHORT_SAMPLE) stopResampling();
  };

  const resampleWhenQuiet = () => {
    clearTimeout(resampler.timer);
    resampler.timer = setTimeout(resample, RESAMPLE_QUIET_MS);
  };

  const resampleOnGrowth = (text) => {
    resampler = { text, lang: language(), timer: 0, observer: new MutationObserver(resampleWhenQuiet) };
    resampler.observer.observe(document.documentElement, {
      childList: true,
      subtree: true,
      characterData: true,
      attributes: true,
      attributeFilter: ["lang"],
    });
  };

  const follow = () => {
    let address = location.href.split("#")[0];
    let pending = 0;
    navigation.addEventListener("currententrychange", () => {
      const now = location.href.split("#")[0];
      if (now === address) return;
      address = now;
      clearTimeout(pending);
      pending = setTimeout(report, NAVIGATION_REPORT_MS);
    });
  };

  const factory = () => globalThis[Symbol.for("rikka")]?.[FACTORY];

  const instantiate = async () => {
    if (typeof factory() !== "function" && !(await send({ rikka: "load", script: TRANSLATOR }))) return null;
    return typeof factory() === "function" ? factory()(host) : null;
  };

  const handleCommand = async (command) => {
    if (!command || typeof command !== "object") return false;
    if (command.do === "restore") {
      if (translator) translator.stop();
      return true;
    }
    if (command.do !== "translate") return false;
    if (!translator) translator = await instantiate();
    if (!translator) return false;
    stopResampling();
    return translator.handle(command);
  };

  let watching = 0;

  const watch = () => {
    const generation = ++watching;
    const current = () => generation === watching;
    const later = () => setTimeout(() => current() && watch(), REWATCH_DELAY_MS);
    const shown = translator && translator.shown();
    ask({ kind: "watch", top: TOP, session: shown ? shown.session : null, display: shown ? shown.display : null }).then(
      (command) => {
        if (!current()) return;
        handleCommand(command).then(
          (handled) => current() && (handled ? watch() : later()),
          () => current() && later(),
        );
      },
      () => current() && alive() && later(),
    );
  };

  if (TOP) {
    const text = report();
    if (text.length < SHORT_SAMPLE) resampleOnGrowth(text);
    follow();
  }
  addEventListener("pageshow", (event) => {
    if (!event.persisted) return;
    if (TOP) report();
    watch();
  });
  watch();
})();
