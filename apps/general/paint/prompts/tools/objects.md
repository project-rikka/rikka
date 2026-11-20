+++
name = "Objects"

[params]
document = "Tab name; the active tab when omitted."
name = "The history entry, in the person's language."
steps = "In order, each {\"op\", fields}."
+++

Changes drawn objects as one undo step and answers with the objects it made or changed. Objects are the lines, arrows, rectangles, ellipses, paths and text Draw makes; each keeps its id, floats above its layer's pixels and stays editable until rasterized. Every step names objects by the ids Document lists; the objects of one step share a layer.

- `move`: `objects` by `offset` [dx, dy].
- `rotate`: `objects` by `degrees` about `around` [x, y] (their shared centre); text keeps upright and turns its place only.
- `set`: `object` becomes `item`, a Draw item of `line`, `arrow`, `rectangle`, `ellipse`, `path` or `text`, keeping its id and its place in the stack.
- `delete`: `objects`.
- `arrange`: `objects` to the `stack` position among their layer's objects.
- `duplicate`: `objects` copied by `offset` [dx, dy] ([10, 10]); answers the new ids.
- `rasterize`: presses every object of `layer` (the chosen one) into its pixels.
