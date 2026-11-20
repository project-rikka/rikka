---
title: Shell translation window
description: rikka.translate translates typed text or a captured region through a model with externalized prompts.
about: system/rikka-shell/crates/shell-translate
tags: [shell, ai]
---

- A `shell_kit::floating` overlay: text on the left, answer on the right. `Super+T` toggles it. `Super+Shift+T` opens the capture picker in `Mode::Translate`.
- Capture owns the handoff: `capture::shot::snip` crops off-thread and `capture::Snip` keeps a `Snapshot` (the preview crop, plus a PNG ≤1600 px for the model) and notifies.
- A new window takes a waiting snip in `on_start`. An open one hears `snipped`.
- `TranslateService` = `PresenceService` + `picture`.
- Prompts: `system/rikka-shell/crates/shell-translate/prompts/{text,image}.md` via `rikka_prompt`. `text.md` takes a `{{profile}}` paragraph filled from `shell-translate/prompts/profiles/<slug>.md`.
- Profiles (`profile.rs`, a `rikka_types::Choice`): natural (default, keeps the original's register), casual, polite, business. Built-in only; they shape text translation only, so the heading combo is disabled while a picture is shown. Adding one means a variant, a prompt file, a `Profiles` field in `prompts.rs` and locale labels.
- Output is structured: `source` (BCP-47), `translation`, `original`. Fenced objects still parse.
- Text runs only on the Translate button or `Ctrl+Enter` (Enter is a newline, as in Nani). A new snip runs at once.
- Typing, language, profile, model or effort changes never run; the result is muted as stale when its `Asking` (text, languages, text-only profile, selection) differs from the current one. A fresh swap trades both texts and stays fresh.
- While running, the button becomes Stop; superseded or stopped runs are cancelled and their answers ignored.
- Languages: `rikka_languages::Language` (a fixed offered list of tags in `language.rs`, native names as labels, English names in prompts) plus free text. The source may be auto-detected.
- `rikka.translate` remembers target, source, profile and model. An unset target means the session language; an unset model means `rikka.llm`'s default. Choices never write `rikka.llm`.
- The feature keeps no `Llm`: each window loads its own off the UI thread in `start` (`spawn_blocking(Llm::load, ..)`) and follows `Llm::changes()` while open, since nothing follows accounts while it is closed. Until it loads, models are disabled, Translate is off and waiting snips stay in `Snip`. The model dropdown is disabled while no model is offered.
