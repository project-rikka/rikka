---
title: Google application (rikka-google)
description: apps/system/google: Gmail, Drive and Chrome Sync passwords in one native app; crates, sign-in, sections, state.
about: apps/system/google
tags: [application, google]
---

- `rikka-google` shows Gmail, Google Drive and the Google Password Manager for several accounts. Everything is its own client; Chrome appears only as the sign-in page (`rikka_web::SignInPortal`).
- Crates (`apps/system/google/crates`): `google-account` ← `google-nigori` ← `google-sync` ← `google-passwords`; `google-drive` and `google-mail` sit on `google-account` only. Test helpers are `google-test-server` (scripted HTTP server, `Account` answering the auth endpoint) and `google-test-sync` (in-memory sync server model plus its HTTP face).
- Sign-in is the Android account flow, so no OAuth client of Rikka's exists: the portal opens `accounts.google.com/EmbeddedSetup`, the app reads the `oauth_token` cookie, `Master::sign_in` trades it at `android.clients.google.com/auth` for a master token, and `Grants` mints per-service access tokens as Google's own apps (`Service::{Mail, Drive, Sync, Identity}`). That endpoint is spoken over an ALPN-free rustls HTTP/1.1 connection (`transport.rs`), never rikka-net.
- The portal is the only sign-in route. Google delivers this credential only as a cookie to the program hosting the page: its Chrome, Android and iOS clients refuse loopback, custom-scheme and device-code redirects, so no default-browser flow exists.
- `Grants::send(service, |token| request)` is the one authorized call: it retries once with a fresh token after a 401. An invalid master token surfaces as `Error::SignedOut`; every service error has `is_signed_out()`.
- Secrets: schema `os.rikka.Google`, per account folder kinds `master` (the `Master`) and `seal` (the key of the sealed password snapshot). The account book is `$XDG_STATE_HOME/rikka/google`; caches are `$XDG_CACHE_HOME/rikka/google/<account key>/{drive,mail}`.
- `google-drive`: Drive v3 (`Drive`), `Scope` listings, resumable upload, `Export` for native documents, the changes cursor, and `Shelf` (listing snapshots, opened copies keyed by file version, measured thumbnails).
- `google-mail`: Gmail REST (`Mail`), `Summary`/`Thread`/`Message`, RFC 2047 words, `Outgoing::to_rfc822`, `Message::readable` (HTML to Markdown through `rikka_html::Document::to_markdown_with_pictures`) and `Pictures` (kept inline and remote pictures by key).
- `google-passwords`: `Credential`/`Draft` over `PasswordSpecificsData`, `Recipe` generator, `Seal` (XChaCha20-Poly1305, `RKGPASSW` header as associated data) and `Vault`, which runs the engine against `Server`, persists a sealed snapshot after every answer and serializes sync turns.
- Trusted-vault accounts fetch keys in a portal on `accounts.google.com/encryption/unlock/desktop`; the bundled page-world script `google-passwords/vault_keys.js` replaces `chrome.setSyncEncryptionKeys`/`setClientEncryptionKeys` and posts the keys, which `posted_vault_keys` accepts only for the signed-in Gaia id.
- Binary: root state `Google` (`Stage`, `Section`, accounts, portal) with `#[state]` children `MailState`, `DriveState`, `PasswordState`; each gets the `Arc<Session>` in `attach` and is reset by `detach`. Sections are `component`s inside one `page_stack`; window commands go through `Google::command`.
- Drive UI mirrors Files: self-painted `Browser`/`Cell` (details and icons, rubber band, inline rename, drop targets), `Location` trail for crumbs and history, a transfer queue (`Job`) whose progress arrives over a channel stream, and opening a file downloads it to the shelf then hands it to `rikka_open::open`; Google-native documents open their web link.
- Mail UI: mailbox and label list, lazily grown thread list, reader rendering Markdown with pictures resolved against the `Pictures` folder (remote ones only after "Show pictures"), composer sheet for new mail, replies, forwards and drafts.
- The composer never sends on a bare Enter: Ctrl+Enter or the Send button sends, Escape or a press outside sets the message aside as a draft, and only Discard drops it. `Google::command` ignores window commands while a sheet is open.
