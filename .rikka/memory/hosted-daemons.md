---
title: Notification, tray and polkit services
description: rikka-notifications, rikka-tray and rikka-polkit serve session singletons and leave all drawing to the shell.
about: crates/daemons
tags: [notifications, security]
---

- `rikka-notifications`, `rikka-tray` and `rikka-polkit` are public crates, but only the shell may start them: a second host would steal the session singleton.
- `rikka_notifications::Notifier` owns `org.freedesktop.Notifications` and never replaces or yields it. Ids are answered on the D-Bus thread before any surface exists, and exactly one close signal is sent. `Notifier::activate(id, action, token)` sends `ActivationToken` before `ActionInvoked`; the shell mints the token through `rikka.activation/mint`.
- Bodies keep bold, italic, underline, links, image alt and breaks, capped at 6 lines or 320 chars. Images follow spec priority and are validated.
- Capabilities advertised: persistence, no sound. Big images stay in shared memory, and the wake callback only nudges the loop.
- `rikka_tray::Tray` serves or joins `org.kde.StatusNotifierWatcher` (takes the name only when free) and publishes via `Monitor`.
- Tray signals coalesce into one `Changes` batch after 60 ms of quiet (`SOON`), at most 1 s (`LATEST`). Items are normalized against the sender and fetched in one call.
- External calls time out after 4 s, and activations are no-reply. Dbusmenus load on open after `AboutToShow`. Losing a bus owner removes its items. No XEmbed.
- `rikka_polkit::Agent::start(report)` registers the logind session's agent without blocking startup and re-registers on polkit restart.
- Reports carry the whole current `Ask`. `answer(serial, reply)` returns the choice and a zeroing `Secret`. Serials stop a repeated Enter answering the next prompt.
- PAM goes through `/run/polkit/agent-helper.socket`, falling back to the setuid helper. Output is decoded as bytes first, and unreadable prompts abort.
- One question at a time, one thread per conversation. Three tries per identity. Cancel from either side kills the helper.
