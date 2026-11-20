---
title: Settings Users page (avatar, name, password)
description: Users page: AccountsService avatar with circular crop, display name staged as lib.mkForce, passwd password change.
about: apps/system/settings/src/users
tags: [settings, accounts]
---

- Only the session user's own account. Users are declared in `/etc/nixos/configuration.nix` and `users.mutableUsers` is NixOS's default `true`, so passwords persist but names do not.
- Avatar: `rikka_accounts::Accounts` (crate `crates/daemons/rikka-accounts`, async zbus over `org.freedesktop.Accounts`): `current()`, `account()`, `set_icon(path)`, `clear_icon()`, `changes()` (the user's `Changed` signal). `Error::Missing` means accounts-daemon is not installed; the page then offers to stage `services.accounts-daemon.enable` in the desktop module (the installer writes it too).
- AccountsService stores the icon in `/var/lib/AccountsService/icons/<user>`; the shell lock screen (`system/rikka-shell/crates/shell-lock/src/who.rs`) reads that first, then `~/.face`. Both Settings and the lock read the file's bytes (`ImageSource::Bytes`), because path-keyed image cache entries would show a replaced picture stale.
- Picking: `rikka-dialog` chooser filtered by `settings_avatar::Photo::mime_types()`, then the in-page crop card (`cropper.rs` widget: drag, wheel, pinch, arrows, +/-; zoom slider). `settings-avatar` (app-private, tested) owns `Framing` (square crop kept inside the photo, zoom 1..=`MAX_ZOOM` around an anchor) and `Photo::avatar` (`AVATAR_SIDE` RGB PNG, or `TRANSPARENT_SIDE` RGBA when the crop has transparency, so even incompressible pictures stay under AccountsService's 1 MiB limit). The PNG goes to a temp file handed to `SetIconFile`.
- Names and the initial come from `rikka_dirs::passwd::Entry` (`display_name`, `initial`); `Account` carries only `kind` and `icon`. The page follows AccountsService only while open: closing cancels every keyed job, except a password change in flight, whose form stays.
- Display name: `users.users.<user>.description = lib.mkForce "…"` in Rikka's system module (`users/name.rs`), adding `lib` to the module's formals through `Document::add_formal`; it takes effect after the draft is applied. Typing the current name with nothing staged removes the binding.
- Password: `rikka_pam::change_password(current, new)` drives the setuid `/run/wrappers/bin/passwd` on a pty (`rikka-pam/src/passwd/`): a non-newline tail with ECHO off is a hidden prompt, answered current/new/new; a newline re-arms. Exit after only the first answer with no non-`passwd:` lines is `Incorrect`; a fourth prompt or other lines are `Rejected { reason }`. On success the login keyring follows through `Keyring::login(new)` unless it is absent.
- Untested by design: the passwd pty conversation (needs the setuid binary) and the AccountsService client.
