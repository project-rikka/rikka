---
title: Proton Authenticator codes and sync
description: rikka-proton-authenticator parses otpauth entries, computes codes from given time, and syncs sealed entries.
about: crates/proton/rikka-proton-authenticator
tags: [proton, otp]
---

- `Otp` parses and writes `otpauth://totp|hotp|steam` and `steam://` without a URL crate. `Entry` pairs id, note and `Otp` over the `AuthenticatorEntry` protobuf.
- `code_at(ts)`/`codes_at(ts)` return the current and next code. Time is always an argument.
- `Base32` is its own RFC 4648 codec. Steam secrets fall back to base64. `MigrationPayload::from_uri` reads Google Authenticator exports.
- `AuthenticatorClient::new(api, UserKeys)`/`for_account(&Account, &Arc<AccountKeys>)` (user keys only, from `AccountKeys::user_signing()`) moves to `with_prefix("authenticator")` and fails on an invalid host. HMACs come from rikka-proton-pgp. The shared API host answers these routes with `Path not found`.
- `AuthenticatorKey` (32 bytes) is wrapped in a PGP message the primary user key encrypts and signs. Unsigned keys are refused.
- Entries seal with `SymmetricKey` AAD `entrycontent`, base64, `ContentFormatVersion` 1.
- `sync` reads keys and entries at once; opened sync keys are cached per client (shared by clones), entries decrypt through `parallel_map`.
- Reads page with `Paging::since()` (stops on an empty or repeated `LastID`), writes batch 100 at a time. An entry no key opens is counted, not fatal.
- `SyncPlan::compute(remote, local)` decides per entry by `EntryState`, revision and times. `sync` also works out the order like the official mobile client (`SyncOrder`), pushes it and returns `SyncOutcome` (store, drop, still-sealed count, `order: Vec<Placement>`).
- `CreateTime`/`ModifyTime` are Unix seconds held as `jiff::Timestamp`; positions are server list indices. With a cache, the wire key and entry payloads (still sealed by Proton) persist (`cached_keys`, `cached_entries`).
