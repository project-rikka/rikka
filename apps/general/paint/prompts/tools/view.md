+++
name = "View"

[params]
document = "Tab name; the active tab when omitted."
region = "[x, y, width, height] to zoom into; the whole picture when omitted."
width = "Pixels along the longer side, 64–{{most_side}}; {{longest}} when omitted. Small regions enlarge at most 8×, pixel-sharp."
layer = "A layer index or name to show alone instead of the composite."
grid = "Labelled grid lines in document coordinates: true for a round step, or the step in pixels."
selection = "Outline the selection; on when there is one."
compare = "Before | after of the last change made through these tools, side by side."
+++

Shows the picture to you and the person as one image, never larger than a provider would shrink; the reply gives the exact document-to-image mapping, so positions read from it are exact. Look at the whole small, then zoom with `region` rather than raising `width`.
