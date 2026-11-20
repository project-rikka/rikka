+++
name = "Layers"

[params]
document = "Tab name; the active tab when omitted."
name = "The history entry, in the person's language."
steps = "In order, each {\"op\", fields}."
+++

Changes layers as one undo step and answers with the layers and the chosen one. Layers are named by index or exact name as each step finds them, so a layer made by `new` can be named at once; an omitted `layer` is the chosen one, which follows what each step makes, moves or merges.

- `new`: `name`, above `above` (the chosen layer).
- `duplicate`: `layer`, `name` (" copy" added).
- `delete`: `layer`.
- `move`: `layer` `above`, `below` or `into` a group, or `by` whole steps among its siblings, positive up.
- `set`: `layer` and any of `name`, `visible`, `opacity` %, `blend`.
- `merge_down`: `layer` into the pixel layer below. `merge_visible`. `flatten` into one Background.
- `from_background`: turns the Background into a layer that can move and hold transparency; `name`.

The Background, the bottom layer without transparency, cannot move, have anything moved below it, be renamed, faded or blended. A picture keeps at least one layer.
