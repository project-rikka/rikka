---
title: The agent's own seat
description: The compositor gives an agent a second seat, drawn as an accent ring, so it works the desktop beside the person.
about: system/rikka-compositor/src/agent
tags: [compositor, agent]
---

- A second `wl_seat` `rikka-agent` (pointer + keyboard, no touch) sits beside `seat-0`.
- Virtual pointer and keyboard objects on that seat drive the agent. The keyboard is dispatched by `rikka-compositor-seat/src/virtual_keyboard.rs`.
- The agent's clicks move that seat's focus only, so the person's focus stays put.
- The pointer is an accent-coloured ring under the person's cursor, shown while `rikka.agent/show` is on. Client cursor images for that seat are ignored.
- `rikka.agent` `where` (`AgentClient::placement`) reports the display, its size and the pointer position. `rikka-computer` uses it for screenshots and coordinates.
- `rikka.agent` `wait-idle` is answered from `agent/idle.rs`: `compose` marks a frame in flight on the agent's output, `refreshed` marks it finished and schedules a check; a reactor timer answers each wait with `Quiescent` after the quiet period without frames (none requested or in flight) or `TimedOut` at its deadline.
- Xwayland windows answer only one seat, so they are worked through the person's pointer and seat.
- Compositor code reaches the two seats as `person()` and `agent_seat()`; both exist from launch, before the `Compositor` is built.
