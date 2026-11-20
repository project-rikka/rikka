---
title: Proton Pass client and item model
description: rikka-proton-pass holds the item protobufs and PassClient over shares, items, invites, files, links, aliases and orgs.
about: crates/proton/rikka-proton-pass
tags: [proton, pass]
---

- Layers: `content` (protobuf), `api` (wire types), `crypto` (keys), `client` (`PassClient`). Only `lib.rs` is public. Protobufs `item_v1`, `vault_v1`, `file_v1`, `folder_v1` are hand-written `prost` derives (no protoc). Empty proto3 strings read as `None`.
- `PassClient::for_account` keeps the `AccountKeys` (organization key via `Account::organization_key`), the user ring plus every address ring and a map address → `UserKeys` (primary address key signs); `new()` needs `with_address_keys` for invites and cannot open the organization key. All paths are `pass/v1/…`.
- Reads page themselves. Decrypted share and folder keys sit in a shared `Keyring` with a per-share async lock; share-key PGP runs through `offload`, item decryption through `parallel_map`. With a cache, wire shares, share keys, items and the last event per share persist (`cached_shares`, `cached_items`, `sync_items`).
- `EncryptionKey::seal`/`open` is the AEAD primitive over `SymmetricKey` with `EncryptionTag` AAD; tags must not change: `vaultcontent, itemkey, itemcontent, linkkey, filekey, filedata, key.folder.pass.proton, content.folder.pass.proton, v2;…`. `wrap`/`unwrap` PGP-encrypt a key to `UserKeys`; `ShareKey::decrypt` requires the signature. The `crypto/*` helpers are `encrypt_*`/`decrypt_*` per object.
- Invites: existing users are found through `core/v4/keys/all?InternalOnly=1` (33102/33103 → new-user invite). Signature contexts `pass.invite.vault.existing-user` and `pass.invite.vault.new-user` (body `email|raw key`). Group invites open the group key via the owner's key token (`account.key-token.address`) or the org key (`account.key-token.organization`). Group shares decrypt with forwarding allowed and must be signed by the group.
- Files: 4 MiB chunks, encryption version 2, multipart `ChunkIndex`/`ChunkData`, file key under `filekey` with the item key; `Download` prefetches 2 chunks.
- Secure links encrypt the link key with the item key and put it in the URL fragment as unpadded base64url; recipients use `SecureLinkViewer::new(api)`.
- Also: extra password (SRP), SimpleLogin sync/settings/mailboxes/contacts, notifications, organization read and admin writes, breach monitor, bulk move (`item/share`, `item/folder`) and `item/import/batch`.
- Batch sizes follow the web: invites and file adds 10, removals/moves/imports/pending aliases 100, file restores 50.
