+++
name = "Materials"

[params]
add = "Material graphs to create: a `name` and an optional `preset` key; without a preset the graph is one Principled BSDF."
name = "The new graph's name: letters, digits, spaces, `_`, `-` or `.`, up to 48."
preset = "A preset key from `{{look_tool}}`."
remove = "Material graphs to delete; their slots return to the imported material."
rename = "Graphs to rename; slots and keys follow."
from = "The current name."
to = "The new name."
assign = "What figure slots draw."
figure = "The figure's name as `{{scene_tool}}` gives it."
slots = "`all`, `class:<class>` for every slot of a class (`skin`, `hair`, `face`, `cloth`, `eye`…), or a list of slot indices or material names."
material = "A material graph name, `preset:<key>` to build a new graph from a preset, or null for the imported material."
unique = "true gives every chosen slot its own copy of the graph."
values = "Per-slot values of the graph's declared parameters; with keys on the parameter the value is keyed at the playhead."
slot = "A slot index or material name."
parameter = "A declared parameter of the slot's graph."
value = "The value, or null to fall back to the graph's default."
+++

Adds, removes and renames material graphs, assigns them to figure material slots and sets per-slot parameter values, as one undo step, in that order. The reply names graphs it created.
