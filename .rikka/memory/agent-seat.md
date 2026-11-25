---
title: The agent's own seat
description: The agent drives a carried seat projected onto every client's seat-0 objects, drawn as an accent ring beside the person.
about: system/rikka-compositor/src/agent
tags: [compositor, agent]
---

- The agent seat `rikka-agent` (pointer + keyboard) is a carried seat of `seat-0` (`Seats::add_carried`). Ordinary clients see only `seat-0`, because many toolkits break with two seats (gpui/Zed keeps the last seat bound, Chromium and GLFW the first).
- Only clients accepted on the agent socket (`ClientOptions::agent()`, `Client::is_agent`) see the `rikka-agent` global. The socket lives at `rikka_session::agent::socket_path()` (`$XDG_RUNTIME_DIR/rikka/<display>/agent-wayland`, deliberately not `.sock`). `agent/socket.rs` opens it after the seam socket.
- `rikka-computer` (`InputDriver::for_agent`) and the UI benchmark connect there through `rikka_virtual_input::Seat::connect_named(socket, SEAT)`. Their virtual pointer and keyboard drive the agent.
- The agent keeps its own pointer position, focus and held keys. Its input reaches each client through that client's `seat-0` objects (see the compositor-seat note). Clicks move only the agent's focus, so the person's focus stays put.
- When the person and the agent use the same client with the same device, the latest input decides which focus the client sees.
- The pointer is an accent ring under the person's cursor, shown while `rikka.agent/show` is on. Cursor images clients set for the agent's presentation reach the compositor as `SeatEvent::Cursor` for the agent seat and are ignored.
- Popup grabs carry the issuer of their serial (`Seats::issuer`). The agent gets its own popup chain (`input/grabs/popup.rs`, `PopupChains`), so its menus never grab the person's pointer or keyboard. Activation tokens whose serial the agent issued are refused.
- `rikka.agent` `where` (`AgentClient::placement`) reports the display, its size and the pointer position. `rikka-computer` uses it for screenshots and coordinates.
- `rikka.agent` `wait-idle` is answered from `agent/idle.rs`. `compose` marks a frame in flight on the agent's output, and `refreshed` marks it finished and schedules a check. A reactor timer answers each wait with `Quiescent` after the quiet period with no frames requested or in flight, or with `TimedOut` at its deadline.
- Xwayland windows and server-side captions are relayed through the person's seat. Over them the agent seat is given no focus (`under = None`), and its keys are replayed on the person's seat.
- Compositor code reaches the two seats as `person()` and `agent_seat()`. Both exist from launch, before the `Compositor` is built.
