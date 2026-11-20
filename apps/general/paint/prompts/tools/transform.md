+++
name = "Transform"

[params]
document = "Tab name; the active tab when omitted."
name = "The history entry, in the person's language."
steps = "In order, each {\"op\", fields}; each sees the size the last one left."
+++

Changes the whole canvas as one undo step.

- `crop`: `rect` [x, y, width, height], or the selection (transparent outside a shaped one).
- `resize`: `width` and/or `height` px, one alone keeping proportions; `resampling` (`lanczos`).
- `canvas_size`: `width` and/or `height`, `anchor` (`center`), `fill` for new area (white when opaque, else transparent).
- `rotate`: `degrees` clockwise, growing the canvas to the corners, `fill`, `resampling` (`bicubic`).
- `turn`: `direction`; `flip`: `axis`. Both lossless.
- `convert`: pixels to `profile`, one of {{profiles}} or an absolute .icc path, with `intent` (`relative`).

Crops, resizes and rotations clear the selection; turns, flips and canvas size carry it.
