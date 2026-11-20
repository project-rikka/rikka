+++
name = "Adjust"

[params]
document = "Tab name; the active tab when omitted."
name = "The history entry, in the person's language: \"Warm up the sky\"."
steps = "In order, each {\"op\", parameters, \"layer\"?, \"mask\"?}; omitted parameters take their defaults."
+++

Applies adjustments and effects as one undo step. A step works on `layer` (index or name; the chosen layer when omitted), inside its `mask`, else the selection, else everywhere; pixel layers only.

A `mask` is `rect` or `ellipse` [x, y, width, height], or `path` SVG `d` with `fill_rule`; plus `feather` px and `invert`.

Ops with their parameters as range =default; `{…}` groups nested parameters, `a|b` names groups that share them, colours are "#rrggbb", curves are [[input, output], …] in 0–1, levels in 0–255; unknown choices are refused with the valid ones:
{{operations}}
