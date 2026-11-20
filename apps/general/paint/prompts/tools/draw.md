+++
name = "Draw"

[params]
document = "Tab name; the active tab when omitted."
name = "The history entry, in the person's language."
layer = "A pixel layer's index or name; the chosen layer when omitted."
items = "Drawn in order, each {\"tool\", fields}."
+++

Draws as one undo step and answers the ids of the objects it made. `line`, `arrow`, `rectangle`, `ellipse`, `path` and `text` become objects: they float above the layer's pixels, ignore the selection and stay editable with Objects. The other tools paint pixels, beneath any objects, inside the selection's soft edges when there is one. Omitted, `color` is the primary colour (the secondary for `eraser`, white for `erase`) and `antialias` is on; widths and brush sizes are px up to 500.

- `pen`, `highlighter`: `points` [[x, y], …] one stroke, `color`, `width`.
- `line`, `arrow`: `points` [from, to], head at `to`; `color`, `width`.
- `rectangle`, `ellipse`: `rect` [x, y, width, height] turned by `angle` degrees about its centre, `color`, `width`, `filling`; `fill` colours the inside for `both`.
- `brush`, `eraser`: a trail of `points` or SVG `d`; `size` diameter, `hardness`, `opacity`, `flow` in %, `color`, `antialias`; the window's settings when omitted. The eraser clears to transparency, or paints `color` on a layer without transparency.
- `path`: SVG `d` (every command, exact curves); `fill` and/or `stroke` colours, `width` (4), `fill_rule`, `join`, `cap`, `antialias`.
- `gradient`: `shape` from `from` to `to` [x, y]; `colors` [start, end] (primary to secondary), or `fade` [start %, end %] of the layer's opacity, on layers with transparency.
- `text`: `text` (\n breaks), `font`, `size` px up to 2000, `weight` (400 regular, 700 bold), `italic`, `align`, `wrap` width, `color`, `antialias`. The ink's `anchor` point lands `at` [x, y] (`top_left`), or the text sits `inside` [x, y, width, height] at `anchor` (`center`), wrapping only at `wrap`. Answers the layout and ink boxes.
- `bucket`: fills the colour region `at` [x, y]: `tolerance` % (50), `mode`, `sample` (the layer), `color`, `antialias`.
- `fill`, `erase`: the selection, or the whole layer, filled with `color`, or cleared (a layer without transparency takes `color`).
