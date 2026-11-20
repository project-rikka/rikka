---
title: Shared AI settings page (rikka-llm-settings)
description: rikka-llm-settings is the one AI page (accounts, sign-in, endpoints, models, reply language) Settings and ADE show.
about: crates/ai/rikka-llm-settings
tags: [llm, settings]
---

- Public UI crate over `rikka-llm-accounts` (which stays UI-free); `LlmSettings::load()` owns its own `Llm`, `llm_settings(&LlmSettings)` draws it. It derives `State`, so hosts hold it plain (Settings) or as `#[state] Option<LlmSettings>` (ADE); handlers target `&mut LlmSettings`.
- `open()` reloads `rikka.llm`, rereads the key, language and usage fields and follows `Llm::changes()` (keyed stream); `close()` cancels that stream and commits typed keys and the reply language. A browser sign-in keeps running after `close`, so hosts keep the state alive (ADE loads it on the first settings visit and never drops it).
- Contents: ready accounts with one add button (`accounts.rs`, `card.rs`, `signing.rs`), usage gauges per metered account (`usage.rs`, `rikka_usage_bar`), OpenAI-compatible endpoints (`endpoint_form.rs`: name, base URL, optional key, Chat Completions/Responses, extra headers, Test connection through `Endpoint::check`), their models (`models.rs`, `model_editor.rs`: display name, context, output, tools, vision, reasoning levels; the first saved model becomes the account's model) and reply language (`language.rs`).
- Strings are `llm-settings.*` (per-backend hints `llm-settings.hint.<backend id>`); every process that shows the page embeds `rikka_llm_settings::CATALOGS`, `rikka_llm_accounts::CATALOGS` and `rikka_usage_bar::CATALOGS`.
- Hosts: Settings' AI section and ADE's settings section `Section::Llm` (first, default; ADE's welcome notice's Sign in button opens it through `Ade::open_settings`).
