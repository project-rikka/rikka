---
title: Browser page-translation script
description: browser-translate's probe.js (every frame) and translator.js (on demand): units, modes, code comments.
about: apps/system/browser/crates/browser-translate/scripts
tags: [web, translation]
---

- `scripts/probe.js` is `PROBE_SCRIPT` (~6 KB): one instance per frame in isolated world `rikka-translate` (rikka-web user script, `Frames::All`, `InjectAt::End`), talking only through `window.rikka.messageHandlers.rikkaTranslate.postMessage`, a replying handler whose promise carries the app's answer (`page`, `watch`, `translate`). It reports the page, keeps the watch generation and follows navigation and `pageshow`.
- `scripts/translator.js` (~60 KB, crate-private `TRANSLATOR_SCRIPT`) is one arrow-function expression `(host) => ({ handle, stop, shown })`. A watch says whether its frame already has it (`translator`); `Command::answer(&Watch)` adds the source as `translator` to a translate answer only for frames without it, and the probe instantiates it with an indirect `eval` inside its sandbox (page CSP does not apply there). `host` carries `post`, `top` and the sample vocabulary the probe shares.
- If the first `page` sample is short, a MutationObserver re-reports it after mutations go quiet, until the sample is long enough or a translation starts.
- The top frame re-reports `page` on same-document navigations through the Navigation API (`navigation` `currententrychange`, debounced).
- A rejected `watch` stops watching; only `pageshow` with `persisted` (back/forward cache) re-posts `page` and starts a new `watch` generation.
- Everything for one session lives in `run`; `stop()` restores every applied unit and disconnects observers.
- Unit kinds: flow (inline runs of a block container), attribute (`title`, `alt`, `placeholder`, `aria-label`, button-like `value`), title (the `<title>` text node), comment (code-region paragraphs).
- Replace mode only changes text-node data and sibling order within a unit; restore records originals, created nodes and orders, and reverts only nodes still holding our text.
- Bilingual adds `rikka-translation` (styled through CSSOM only) for prose roots; other units stay replaced.
- Code regions (`pre`, GitHub `.react-code-file-contents`, diff tables, highlighter wrappers, notranslate wrappers of a lone `pre`) map comments onto text slots or `data-code-text` attributes; edits per slot compose.
- Comments come from highlighter classes when the region is highlighted, otherwise from a language-family lexer (`FAMILIES`, `LANGUAGES`).
- Our own DOM writes run inside `withoutObserving`, which drains MutationObserver records before and discards them after.
- `tests/page.rs` is a machine test (`tests/machine.rs`): it serves fixtures from a local TCP server to a real Gecko session and fakes the host (uppercase plus reversed sibling order).
