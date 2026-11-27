---
title: Chrome Sync client (google-sync, google-nigori)
description: Sans-IO Chrome Sync engine for passwords: messages, Nigori encryption states, commit and conflict rules, fake server.
about: apps/system/google/crates/google-sync
tags: [google, sync]
---

- `google-nigori` is the only crypto crate (built without Cranelift): `Nigori::derive` (PBKDF2-HMAC-SHA1 1003/1004 rounds or scrypt 8192/8/11), key name, AES-128-CBC + HMAC-SHA256 blobs. Its tests hold Chromium's reference vectors.
- `google-sync::proto` is `sync_pb` written by hand with prost (password, Nigori, GetUpdates, Commit; `EntryResponse` is a proto2 group). Types holding secrets skip prost's `Debug`.
- `Engine` does no I/O: `next_request()` yields the next `ClientToServerMessage`, `receive(response)` applies the answer, `abandon()` drops a failed request. Order: GetUpdates while unsynced or a refresh is asked, then a Nigori commit, then password commits in batches. `snapshot()`/`restore()` carry everything as one prost message.
- `Server::exchange` posts gzip protobuf to `clients4.google.com/chrome-sync/command/?client=Google+Chrome&client_id=<cache guid>` with the Chrome user agent and the `Sync` grant.
- Encryption (`encryption.rs`): keystore, custom passphrase (PBKDF2 or scrypt), frozen implicit and trusted vault. A keybag that does not open stays pending and `Lock` says what is missing; entities sealed with unknown keys are stashed and replayed once keys arrive. An empty Nigori node is initialized with the keystore key and committed. The client never rotates keys or re-encrypts entities sealed with older keys.
- Password entities are keyed by Chromium's client tag hash (`client_tag_hash`). Unknown `PasswordSpecificsData` fields are kept byte for byte and appended on commit; password issues stay an opaque field.
- Conflicts follow Chromium's default: a newer remote change replaces an unsent local one, except a remote deletion, which yields to a local edit. During the first merge after a store reset the newer `date_created` wins.
- `NOT_MY_BIRTHDAY`, `CLIENT_DATA_OBSOLETE`, `ENCRYPTION_OBSOLETE` and `MIGRATION_DONE` reset sync metadata, keep unsent creations and return `Error::StoreReset`; `Vault::sync` continues once after it.
- Explicit-passphrase accounts commit no `unencrypted_metadata`; every commit carries `encrypted_notes_backup`.
- `google_test_sync::FakeServer` models birthday, versions, paging, keystore key and commit conflicts; `sync(engine, server)` drives an engine against it and `Google` serves it over HTTP together with the auth endpoint.
