---
title: Settings schema and ownership
description: rikka-settings: transportable schemas, a three-layer cascade, one session writer, announcements and domain helpers.
about: crates/session/rikka-settings
tags: [settings, session]
---

- Every setting lives in a validated `Domain` → `<domain>.toml`. Domain validation makes traversal unrepresentable.
- Schemas are data (type from default, summary, choices, bounds). Local `checked_with` validation stays with the owner.
- Enum settings implement `rikka_types::Choice` (`ALL`, `slug`; provided `from_slug`/`of_slug`), declare with `Key::choice::<C>(name)` and read/write through `choice::<C>(key)` / `set_choice(key, c)` on `Store`, `seam::Settings` and `seam::Preferences`; unknown stored slugs read as `C::default()`. `Preferences::set_choice` writes only when the choice moves and returns whether it did, so UI owners notify observers from it.
- `SettingsService` keeps an unlocked domain's store when it is redeclared with the same schema, and every store settles the scribe for its file before reading it, so a pending write is never read stale.
- Cascade: schema default, then the first `$XDG_CONFIG_DIRS/*/rikka`, then `$XDG_CONFIG_HOME/rikka`. Only the user layer is written, so reset reveals machine policy. `Layers::volatile()` reads the user layer from disk but keeps every write in memory shared by all clones (a nested session's compositor hosts settings this way).
- Malformed user values fall through into `LoadReport`. Code errors are errors. Unknown keys are kept across writes.
- With `seam`, the compositor hosts `rikka.settings`. Reads are local, writes go through the host, and announcements carry the accepted values. Files are never watched.
- Without a session, `Settings` writes locally and announces nothing.
- `Settings::mirror` follows shared domains. `Settings::adopt` fetches another owner's schema for generic editors.
- Host-owned domains reject disagreeing redeclarations.
- `Settings::changes()` yields `Announcement::Changed|Lagged`; `Lagged` means read afresh. Apps call `Settings::refresh` for their own domains. `rikka-app` follows only theme and language.
- The scribe writes whole files atomically, coalesces, fsyncs file and dir, and flushes at shutdown; `rikka_settings::flush_pending_writes()` waits for pending writes.
- `session::{animation, appearance, capture, clipboard, color_picker, displays, emoji, ime, language, lock, network, notifications, pointer, power, remote, replay, translate, weather, windows}` is the one catalog of desktop domains, the ones the compositor or shell hosts and Settings and the CLI edit (`displays` has no static schema: the compositor declares it per connected display from `displays::{key(slug, suffix), listed, list, Mode, Position, AdaptiveSync, transform_name, transform_named, SCALES}` (plus `HDR`/`SDR_BRIGHTNESS`, only for HDR-capable displays), and the Settings app reads it with the same codecs): `DOMAIN`, key names, defaults, choices, ranges and `schema()`. Hosts (compositor, shell, rikka-replay, rikka-remote) and editors (Settings app) import them; owners only map values onto their own types.
- The catalog names no other crate's type, and rikka-settings links only foundation crates and rikka-seam. A key over another crate's vocabulary takes it as a type parameter (`emoji::schema::<Tone>()`, `translate::schema::<Profile>(efforts)` with any `rikka_types::Choice`) or as data passed by the owner; a setting enum or row with no other home lives in the catalog (`color_picker::{Activation, Click, Format, built_in_formats}`), and a constant shared with a protocol is stated here and checked by the protocol crate's tests (`remote::PORT_DEFAULT` against `rikka_remote_wire::DEFAULT_PORT`). Library-owned domains (`rikka.llm` in rikka-llm-accounts, the terminal's) and app preferences stay beside their owner.
- `Store`/`Settings::read(key) -> Option<T>` warns once with `domain/key` on a failed read; `read_or(key, declared)` falls back. No caller writes its own warn-and-default reader.
- `seam::protocol::{set, reset, get, user_values, read, sources, describe, declare, domains}` build versioned requests; nobody assembles `Request::new(CAPABILITY, …)` by hand.
- App-owned domains use `Preferences::open(schema)`: never fails (logs once), `get` falls back to schema defaults, `set` returns whether it was written and keeps an unwritten value for the run, `refresh`/`apply_change`/`changes` for live updates. `user_values()`/`set_user_values()` are the in-memory user layer. `in_memory(schema)` for tests.
- Gap: schemas are not persisted, so a domain is editable generically only while its owner runs.
- Language changes rebuild views but not strings already derived into models.
