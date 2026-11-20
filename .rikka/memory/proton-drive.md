---
title: Proton Drive client
description: rikka-proton-drive walks the node key hierarchy on v2 volume endpoints: files, photos, sharing, devices, docs, events.
about: crates/proton/rikka-proton-drive
tags: [proton, drive]
---

- `DriveClient::for_account`, `new` or `with_pgp`. Address keys sign and verify; node passphrases are `Secret`s. Every level is an armored locked key, a passphrase encrypted to the parent, and a detached signature.
- Every node request goes through `drive/v2/volumes/{volume}`; methods locate nodes by `&VolumeId` and `Link` (read from the v2 shape with `File`/`Folder`/`Album`/`Photo`/`Sharing` parts) carries its volume. Share URLs, share bootstrap/delete and editors-can-share stay share-scoped as in the SDK.
- `NodeKeys` holds node key, passphrase and their session keys plus a folder `HashKey` or file content `SessionKey`; moves, album adds and share creation re-encrypt with them. `HashKey::hash(name)` = hex HMAC-SHA256; content hashes use the same HMAC.
- Invalid signatures are `Error::Signature`; missing or unverifiable ones go to `Signatures`.
- Listings fetch child IDs page by page, then links in batches of 100, decrypting with `parallel_map_each`; files without a committed revision are skipped. `open_root(volume, share, root)` is one round trip. `AnchorID`/`More` responses flatten `crate::anchor::Anchor` and page with `anchored()` (a `Paging::cursor`).
- Download streams six blocks at once, checks SHA-256, verifies the manifest. Upload: `Draft` → sealed 4 MiB blocks with per-block `Verifier.Token` (verification code XOR encrypted bytes) → commit with the manifest signature; `expected_sha1`, `upload_revision`, `with_client_uid`, `on_progress` + `resume_from(ResumePoint)` for resumable drafts; failed drafts are deleted with retries. Block storage failures are `Error::Transfer`. `Download` ends in `into_stream`/`into_vec`/`for_each_block`/`save_into(folder, progress)` (partial file in the folder, fsync, `rikka_fs::rename_new` to the file name or `name (n).ext`; nothing is left behind on failure); trash/restore/delete and photo moves answer `Vec<rikka_proton_api::Outcome<LinkId>>`; `Upload` in `upload_bytes`/`upload_file`.
- Thumbnails (incl. HD) batch 30 ids through `drive/volumes/{v}/thumbnails`.
- Photos: library (`drive/v2/shares/photos`, created when missing), timeline, upload with capture time and `ContentHash`, duplicates, albums, tags/favorites, cross-volume copy/move (`photos/transfers.rs`).
- Sharing: share URLs with SRP password, invitations (contexts `drive.share-member.{inviter,member,external-invitation}`), members, shared-with-me, public links (`open_public_link`), bookmarks, abuse reports. The share passphrase session key is the secret behind invitations and links.
- Devices (`devices.rs`), documents (`create_document`, `File::document_url`), revisions, `available_name`.
- Events: v2 feed carries only link ID, parent, trashed, shared; `catch_up` follows `More` into an `EventLog` (`needs_refresh` drops it). With a cache, volumes, shares, links, child lists and encrypted thumbnails persist; events invalidate them.
- Integer wire enums (`LinkType`, `ShareType`, `ThumbnailType`, …) are all declared with `rikka_proton_api::wire_enum!`: `Other(i32)` fallback, `From<i32>`, serde as `i32`. Crypto helpers in `crypto.rs` are `decrypt_*`/`generate_*`/`seal_*`; `DriveClient::decrypt_link` turns a `Link` into a `Node`.
- Tests run against `rikka-test-proton-drive`'s stateful `DummyDrive` (`with_shape(Shape::new()…)` seeds folder `photos`, file `holiday.txt`, the `elsewhere` volume and fixture keys; `Shape::empty()` for drive-sync). Request bodies are asserted by route with `*` segments (`last_body("PUT /drive/v2/volumes/volume/files/*/revisions/*")`, `bodies`, `request_count`), uploads by block index (`uploaded(n)`, 0 = thumbnail).
- Copies are numbered `name (n).ext` from 1 by `rikka_proton_names::{numbered, numbered_name}`; local file names use `numbered_local_name`, which keeps the number inside the 255-byte limit.
