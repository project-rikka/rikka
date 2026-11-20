+++
name = "SetCloth"

[params]
figure = "The figure's name as `{{scene_tool}}` gives it."
whole = "Feel values from 0 to 1 for the whole figure, 0.5 leaving it as it is: {{feels}}. They work on top of every piece's own feel."
pieces = "The pieces to change, each by its name from `{{cloth_tool}}`."
piece = "The piece's name exactly as `{{cloth_tool}}` gives it."
preset = "How the piece moves: {{presets}}. Choosing one drops detailed settings and keeps the feel."
enabled = "true simulates the piece, false leaves it following the body."
feel = "Feel values from 0 to 1, 0.5 being the preset itself: {{feels}}."
reset = "true returns the piece to its preset: every feel at 0.5 and no detailed settings."
+++

Changes how a figure's cloth and hair move, as one undo step: the whole figure's `feel`, its `pieces`, or both. The whole call is checked first; an unknown piece, preset or feel is refused with what exists and nothing changes.

- `sway` is how much and how long a piece swings on its own: low settles calmly, high swings freely and far.
- `response` is how strongly the body's own motion (hip sways, turns, steps) throws it: 0 rides along with the body, 1 swings out further than real inertia would.
- `weight` is how heavy it hangs: low floats and catches the air, high falls and drops heavier.
- Use the whole figure's `feel` for requests about everything the figure wears, a piece's `feel` for one piece. Set only the feels the request is about.
- Physics runs during playback and renders; render a moving frame range to judge a change.
