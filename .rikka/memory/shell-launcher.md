---
title: Shell launcher
description: A recurring overlay over pure per-keystroke providers, fzy scoring, distant providers and a wallpaper gallery.
about: system/rikka-shell/crates/shell-launcher
tags: [shell, launcher]
---

- An exclusive overlay (`rikka.launcher`) opened by the `toggle-launcher` shortcut.
- Providers: apps, arithmetic (`fend-core`), command palette, `$PATH`, web. Prefixes `=`, `/`, `>`, `?` select one.
- Search is synchronous and pure per keystroke. Scans build indexes off-thread and swap in whole. Candidates are folded once into `rikka_fuzzy::Haystack` keys per candidate set (apps per `Arc<rikka_xdg::Catalog>`, `$PATH` per scan); apps show every match (`score::ranked`), `$PATH` the top N (`score::best`), ties in candidate order. The registry does not truncate.
- With an empty field `Providers::answer` lists, without duplicates: fresh hits (`Hit::arrived` later than the item's last launch, newest first) → empty-query search hits → 8 recalled by frecency → the rest of `Provider::listing` (apps: every listable entry A–Z, case-insensitive). The list is a lazy collection bounded by `max_height`.
- The result list keeps one collection subject across keystrokes with `.glides(false)`: rows named `(provider, Hit::key)` snap to the new results (the ride's tucking showed stale rows at the top), while the selection pill, the sheet's `resizing()` height and the results' keyed enter transition carry the motion.
- Hits that echo the typed text (web open/search, calculator result, the typed `$PATH` line) are marked `Hit::echo`, so their row key stays fixed while `Hit::id` (URL, result, command line) changes per keystroke.
- App install times: `src/arrivals.rs` stamps each newly seen desktop id into `launcher-arrivals.json`, at startup, from `Installed::on_change` (watched rescans only, after the scan is published) and whenever `AppsProvider` rekeys a new catalogue, so the listing never misses a stamp. The rule (first sighting is a baseline, removed ids forgotten, empty catalogues ignored) lives in the tested `system/rikka-shell/crates/app-arrivals`. Nix store mtimes are useless for this.
- A `RemoteSearch` provider (the calculator) runs on its own thread: it is asked anew per query and told via `QueryToken` to drop stale work. Results arrive through `RemoteProvider::arrivals`; local providers report changes through `Provider::changes` (a `ChangeStream`), and `Providers::changes` merges both into the stream the launcher refreshes on. Nothing polls.
- One `rikka-fuzzy` score for all (`provider::CERTAIN`/`LAST_RESORT` are the launcher's own classes), with explicit classes for exact answers and last-resort web, plus a launch-history boost.
- History lives in `$XDG_STATE_HOME/rikka` via `shell_kit::StateFile`. Entries no provider offers any more disappear.
- The web provider classifies the text with `rikka_omnibox::classify` over the built-in `SearchEngines`: addresses and file paths open (`about:` pages are skipped), anything else searches the default engine; the `?` trigger always searches.
- The field text is the only command state: `/<id> <arguments>` opens a command and searches its choices; `CommandsProvider` re-derives it per keystroke and `Provider::leave` resets it when another scope is typed.
- A `Hit::completes` row (command rows) rewrites the field to trigger + completion on Enter or Tab instead of activating; `Within::opened_by` names the opening text.
- Escape unwinds arguments → `/<id> ` → trigger → empty → closes the launcher.
- `/wallpaper` rebuilds a gallery from `rikka.wallpaper` `listed`: an endless ring (`GalleryState`, `gallery::Turn`), and the field leaves Left/Right to it.
- Desktop entries are watched via `rikka-xdg`, including Nix profile relinks, with catch-up on open if a watch failed. Icons come from `rikka-icons` with a Material category fallback.
- `Exec` is parsed, never passed to a shell. Spawned programs get their own process group and are reaped. Files open via `rikka-open`/`xdg-open`.
