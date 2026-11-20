+++
name = "Select"

[params]
document = "Tab name; the active tab when omitted."
name = "The history entry, in the person's language."
steps = "In order, each {\"op\", fields}."
+++

Changes the selection as one undo step. Adjust and Draw stay inside it, its soft edges blending in, until `none`.

- `rectangle`, `ellipse`: `rect` [x, y, width, height]; `polygon`: `points` [[x, y], …]; `path`: SVG `d` with `fill_rule`. Each takes `combine` with the held selection (`replace`) and `antialias` (on).
- `wand`: the colour region `at` [x, y]: `tolerance` % (50), `mode` (`global` takes every similar pixel), `sample` (the image), `combine`, `antialias`.
- `layer`: the opaque pixels of `layer` (the chosen one), with `combine`.
- `all`, `none`, `invert`.
- `expand`, `contract`, `border`, `smooth`, `feather`: `by` px.
- `move`: the outline by `dx`, `dy` px.
