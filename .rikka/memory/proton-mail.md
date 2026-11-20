---
title: Proton Mail client
description: rikka-proton-mail reads, decrypts, parses MIME, searches, filters and sends mail with web-equivalent send preferences.
about: crates/proton/rikka-proton-mail
tags: [proton, mail]
---

- `MailClient::for_account` keeps an `Account` (recipient keys) and a `ContactsClient` (send preferences; callers pass one carrying `KeyTransparency` via `with_contacts`). Mail depends on contacts, calendar on both.
- Reading: `messages()`/`conversations()` are one generic `Query<K>` (`MessageQuery`/`ConversationQuery` aliases, `list`/`all`/`cached`/`cached_all`), `labels()`; `message_ids()`; `open_message(id, Some(sender))` fetches message and sender keys in one round trip. Decryption runs through `offload`. `sender_keys(email)` is the contacts crate's `verifying_keys` (web verification preferences: internal keys plus pinned contact keys; memoized 512 emails, 15 min, kept in `contacts.verifiers`).
- MIME (`src/mime/`) never fails: `MimeMessage::parse` gives a typed tree; `.content()` gives html, text, inline images, attachments, signature status and protected headers. `multipart/signed` is checked over the CRLF-canonical part; crypto runs only through the internal `MimeDecryptor` (`mime/decryptor.rs`).
- Sending: `draft()` then `send(&id)`, or one-shot `direct()` (`send/direct`). `to` resolves each recipient with the contacts crate's `send_preferences` under the account's mail settings (Sign, PGPScheme, AttachPublicKey, DraftMIMEType, cached per client); `.sign(bool)` and `.attach_public_key(bool)` override. External keyless recipients with signing get signed clear MIME (32), built from the body, `cid:` images and files. The sender key attaches as `publickey - <email> - 0x<FP8>.asc`.
- `PackageKind` bits: internal 1, encrypted-outside 2, clear 4, PGP inline 8, PGP MIME 16, clear MIME 32. One session key per package type; each recipient gets a key packet; cleartext recipients get `BodyKey`/`AttachmentKeys`; password recipients get an SRP verifier.
- Encrypted-outside recipients: `OutsideClient::unlock(id, password)` → `OutsideSession` (`Authorization: <token>`, `x-eo-uid`).
- Search: `SearchIndex` (web semantics, normalized text, substring scan) sealed as `BinaryFormat` `RKMAILIX` header + AES-GCM with that header as AAD; `indexer()` is resumable.
- Filters (`mail/v4/filters`, RFC 5228 `SieveScript`, `SimpleFilter` ↔ sieve/`Simple` JSON), typed `MailSettings` and ~45 `change_setting` writes, `import()` (≤10 messages, 70 MiB, results as `ImportOutcome`), snooze, `in_labels(..)` (`LabelSelection`: read/unread/relabel/move by search context), `undo`. These live in `impl MailClient` blocks in their own modules.
- Batches (150 per request): `message_batch(&ids)` / `conversation_batch(&ids)`; conversations need `.in_label(&folder)` before `mark_unread`/`delete`.
- With a cache: labels, message/conversation metadata and page ids, label counts (`cached_message_counts`/`cached_conversation_counts`); `apply_event` keeps them fresh (events carry `MessageCounts`/`ConversationCounts`). Listings with search filters (anything beyond label, address, sort, unread, limit) cache their items but no page.
