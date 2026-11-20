+++
name = "Cloth"

[params]
figure = "The figure's name as `{{scene_tool}}` gives it."
+++

Lists a figure's simulated cloth and hair as JSON: the whole figure's `feel`, and for each piece its `piece` name, `kind` (`sheet` for garments, `strands` for hair), `preset` and the `suggested` one, whether it is `enabled`, its collision `layer` (`auto` when chosen for it), its own `feel` values and whether `detailed` settings from the person replace them; plus the scene's cloth `quality`.

- Pieces are found from the model's meshes; the person can repaint which parts move in the Physics tab.
- Name pieces exactly as given when changing them with `{{set_cloth_tool}}`.
