---
title: XWayland drag-and-drop bridge
description: How the compositor and rikka-compositor-xwm bridge Wayland drags to XDND targets and X11 drags into Wayland windows.
about: system/rikka-compositor/src/selection/xdnd.rs
tags: [compositor, xwayland]
---

- `rikka-compositor-xwm/src/dnd` speaks XDND version 5. `SelectionKind::Drag` is the `XdndSelection` slot; its helper window is our XDND source window, and conversions reuse the selection transfers.
- Wayland to X11: a drag over an X11 surface becomes a foreign target. `Xwm::drag_enter` honours the cached `XdndProxy`/`XdndAware` (probing only untracked proxies), owns `XdndSelection` with `XdndTypeList`, and sends one `XdndPosition` per `XdndStatus`. An unaware target gets no messages. `DragStatus`/`DragFinished` feed back into the selection crate.
- X11 to Wayland: while an X11 client owns `XdndSelection`, holds a button from an X11 surface, and the pointer is over a non-X11 surface, `show_drop_proxy(true)` maps a root-sized override-redirect InputOutput window. It is XdndAware 5 and sits on top of `_NET_CLIENT_LIST_STACKING`, because some X11 clients only search that list.
- `XdndEnter` on the proxy starts a compositor-driven drag that pointer motion keeps moving; `XdndLeave` cancels it, `XdndDrop` drops it, and the Wayland finish is answered with `XdndFinished`.
- X11 windows of type DND are tagged `Tags::PASSIVE` and never hit-tested, so an X11 drag icon under the pointer does not hide the Wayland target.
