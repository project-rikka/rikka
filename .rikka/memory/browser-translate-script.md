---
title: Browser page-translation script
description: browser-translate's probe.js (every frame) and translator.js (loaded on demand): messaging, units, modes, code comments.
about: apps/system/browser/crates/browser-translate/scripts
tags: [web, translation]
---

- Both files ship in the Rikka Chrome extension as `scripts/browser-translate/{probe,translator}.js` (listed in `//system/rikka-web-host:extension`); nothing is evaluated from strings.
- `probe.js` is the content script `probe_script()` (`UserScript` id `browser-translate-probe`, isolated world, `Frames::All`, `InjectAt::End`) that the app registers once per `Runtime`. It talks to Rust under the message name `MESSAGE` (`browser-translate`): `page` reports are posts (`Event::Message`), `watch` and `translate` are asks (`Event::Ask`) that the app answers with `ScriptAsk::respond`; dropping an ask answers null.
- `translator.js` registers its factory `globalThis[Symbol.for("rikka")]["browser-translate.translator"] = (host) => ({ handle, stop, shown })`. On its first translate command a frame's probe asks the extension to inject it into its own document (`{ rikka: "load", script }`, see rikka-web), then instantiates it with `host` (`ask`, `top` and the sample vocabulary the probe shares).
- A translate answer is `Translation::to_json()`; anything else (null, a dropped ask) makes the translator drop that batch's units.
- If the first `page` sample is short, a MutationObserver re-reports it after mutations go quiet, until the sample is long enough or a translation starts.
- The top frame re-reports `page` on same-document navigations through the Navigation API (`navigation` `currententrychange`, debounced).
- Watches are generations: a null or unhandled answer re-watches after `REWATCH_DELAY_MS`; a rejected ask (worker restart, page frozen into the back/forward cache) re-watches the same way while the extension context lives; `pageshow` with `persisted` re-posts `page` and starts a new generation, which makes older ones inert.
- Everything for one session lives in `run`; `stop()` restores every applied unit and disconnects observers.
- Unit kinds: flow (inline runs of a block container), attribute (`title`, `alt`, `placeholder`, `aria-label`, button-like `value`), title (the `<title>` text node), comment (code-region paragraphs).
- Replace mode only changes text-node data and sibling order within a unit; restore records originals, created nodes and orders, and reverts only nodes still holding our text.
- Bilingual adds `rikka-translation` (styled through CSSOM only) for prose roots; other units stay replaced.
- Code regions (`pre`, GitHub `.react-code-file-contents`, diff tables, highlighter wrappers, notranslate wrappers of a lone `pre`) map comments onto text slots or `data-code-text` attributes; edits per slot compose.
- Comments come from highlighter classes when the region is highlighted, otherwise from a language-family lexer (`FAMILIES`, `LANGUAGES`).
- Our own DOM writes run inside `withoutObserving`, which drains MutationObserver records before and discards them after.
- `tests/page.rs` (from `tests/machine.rs`) runs real Chrome with `:test_extension`, which adds the fixture `tests/page.js` (`test.read`/`test.until`/… over a JSON query, `until` posts `browser-translate-test.reached`); fixtures come from a local TCP server and the fake host uppercases text and reverses sibling order.
