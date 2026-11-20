---
title: App tools for the desktop assistant (rikka.assist)
description: rikka-assist lets any app offer its own agent tools over seam; the shell's sidebar finds, loads and calls them.
about: crates/ai/rikka-assist
tags: [agent, protocol]
---

- Depends on `rikka-tool`, `rikka-prompt`, `rikka-tool-docs` and `rikka-seam`, so an app offering tools links no model client.
- An app offers tools per window with `ToolRegistration::register(Offering, title, Vec<Function>)`; the first seat starts a `rikka-assist` host thread and socket `<kit>-<pid>.sock` in the session's `assist/` subfolder (kept apart so `session_sockets` never probes apps), the last seat dropped removes it. One kit per process; a function named `Windows` or taking `window` is refused.
- `Offering::new(kit)` (ASCII, letter first) `.with_title` (localized) `.with_description` (one line for the model) `.with_instructions` (the app's prompt) `.gist(tool, paths)`. An app's kit is a `static KIT: Kit = Kit::new(name, about, system prompt, &TOOLS, facts)` over `KitTool { stem, document, gist }`: `Kit::prepared()` loads the tool prompts once (shared tool-doc facts, `<stem>_tool` names, the app's `facts` fn, rendered system prompt) and `Kit::offering(title)` builds the `Offering`; the app declares its `Function`s from `Prepared::{tool(index), facts()}` (Paint and Animate).
- Capability `rikka.assist` 1.0: `describe` (kit, title, about, instructions, tools of the window used last), `call {call, tool, input}` → `Outcome` (text, failed, base64 pictures with `shown`), `cancel {call}` (the call answers as stopped), latest event `changed` only when the front window's tools change.
- The host adds an optional `window` property to every tool and a `Windows` tool; calls go to the named window or the one used last (`ToolRegistration::mark_active`, driven by `Window::on_focus`). `window` is stripped before the app's handler runs.
- Model-facing names are `exposed(kit, tool)` = `<Kit>_<Tool>`; app prompts must name tools that way.
- `Apps::watch(toolkit_set)` (assistant side) inotify-watches that folder (the watcher lives with `Apps`), keeps one connection thread per app, re-describes on `changed` (an unchanged description is ignored), and stocks the shared `ToolKitRegistry` with one `ToolKit` per app; dropped futures send `cancel`. `running()` lists `App`s, `used_tool(tool)` keeps display info (title, local name, gist paths) after the app quits.
- `open_assistant(kit)` asks `rikka.assistant` 1.1 `summon` to open the sidebar with `@Kit` in the draft.
- Seam frames allow 32 MiB so render sheets fit.
