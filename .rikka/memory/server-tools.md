---
title: Provider-hosted web tools (server tools)
description: rikka-llm's ServerTool/Hosted blocks, Anthropic web_search/web_fetch declaration, replay, pause_turn and display.
about: crates/ai/rikka-llm/src/messages/hosted.rs
tags: [llm, agent]
---

- Web search and fetch exist only as provider-hosted tools; there is no client web tool or search scraper. Only the Messages protocol (Anthropic, Claude Code) hosts them; others offer no web access.
- `rikka_llm::ServerTool::{WebSearch, WebFetch}` (names `web_search`, `web_fetch`). `RequestBuilder::server_tools(..)` asks for them; `Messages` declares those the configured model lists in `ModelInfo::server_tools()` (set through `Messages::listed`), after client tools, taking the tools cache mark. Versions: `web_search_20250305`, `web_fetch_20250910` with `max_content_tokens` 50 000, the same generation Claude Code uses.
- Support comes from the Anthropic listing: `capabilities.server_tools.web_search.supported` adds `WebSearch`; `WebFetch` is added for every model with capabilities. `ListedModel::server_tools` → `lineup` → `ModelInfo`.
- Responses keep `server_tool_use`, `web_search_tool_result` and `web_fetch_tool_result` as `Content::Hosted(Hosted)`: the verbatim block plus its parsed `HostedPart::{Call, Result}` (`HostedOutcome::{Found(sources), Fetched(source), Failed(code)}`, `Display` is the shown text). It serializes as vendor + payload, replays only to Anthropic and is dropped elsewhere. Text citations are not kept.
- Streaming emits `Event::Hosted(part)` when each block closes. `pause_turn` is `Stop::Paused`: the agent resends the paused answer unchanged; a thread continues it with `messages: []`.
- A `server_tool_use` with no result anywhere in the request is dropped unless its message is last or is followed by tool results, so a stopped paused turn never blocks the next one.
- `rikka-thread` shows hosted calls as `Call`s that apps classify as `ToolAction::Search`/`Fetch`; `ToolAction::name` shows them by `ServerTool::label()` (`WebSearch`, `WebFetch`), never the wire name. Tests use `Scripted::searches`.
