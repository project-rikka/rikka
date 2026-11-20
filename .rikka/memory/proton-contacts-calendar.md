---
title: Proton Contacts and Calendar
description: rikka-proton-contacts (vCard, send preferences) and rikka-proton-calendar (keys, events, RRULE, iTIP) over contentline.
about: crates/proton/rikka-proton-contacts
tags: [proton, calendar]
---

- `rikka-proton-contentline` is the shared RFC 5545/6350 line syntax (folding at 75 bytes on UTF-8 boundaries, escaping, RFC 6868 params, groups) plus the shared `decode_charset` (encoding_rs), `decode_quoted_printable` and the RFC 2047 `decode_q_encoded` that mail also uses; contacts and calendar build their models on it.
- `ContactsClient` / `CalendarClient` follow the product pattern (`for_account`, `new`, `with_pgp`); calendar depends on contacts (recipient keys) and mail (iTIP sending).
- Contact cards: FN/UID/EMAIL/KEY/`X-PM-*` signed-only (text signature over trailing-space-stripped text), CATEGORIES clear when present, the rest encrypted and signed, all with the primary user key. vCard 3.0 and 2.1 import upgrade to 4.0.
- Send preferences port the web's `getSendPreferences`: pinned keys and `X-PM-*` per email group, API keys from `Account::public_keys` (Primary flag), KT verdicts via `with_key_transparency(kt, KtActivation)`, `email_mismatch` against key user IDs.
- Calendar keys: `unlock()` opens the member passphrase with address keys, then every calendar key. The passphrase is base64 of 32 random bytes; invites sign the session-key bytes with context `calendar.sharing.invite`.
- Event parts (`seal_event`/`decrypt_event`): shared, calendar and attendee parts with binary detached signatures by the address key, data packets under the shared or calendar session key (base64), key packets to the calendar primary key on create only. `sync()` posts `events/sync` 10 at a time; personal changes use `events/{id}/personal` (Notifications, Color).
- iCalendar: `ICalendar`, `Event`, `Alarm`, `Rule`; `Zones` prefers IANA names, falls back to file VTIMEZONEs and `jiff-tzdb`; Windows names map through the committed CLDR table `data/windows-zones.txt` (`xtask windows-zones`). Recurrence and VTIMEZONE lookups jump to the requested window for COUNT-less rules.
- iTIP: `ItipMail::for_event` builds the web's subjects and bodies (`InviteWording`, `DateWording`), sent as `invite.ics` with `text/calendar; method=…` through mail's direct send.
- Endpoints worth knowing: `calendar/v2/{id}/bootstrap`, `calendar/v1/vtimezones?Timezones[]=`, `core/v4/addresses/canonical?Emails[]=`, `calendar/v1/directory?Type=2`.
- Model and core events apply to caller models (`ContactModel`/`ContactBook`, `CalendarModel`/`CalendarBook`) and to the cache. Cached calendar events are keyed `{calendar}/{event}` with a per-calendar id index (`calendar.event-ids`), because cache keys are hashed and cannot be prefix-scanned; cache writes are serialized per client.
