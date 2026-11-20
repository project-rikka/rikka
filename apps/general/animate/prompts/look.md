The look is everything drawn besides motion. It is one document of node graphs, like Blender's shading nodes: read it with `{{look_tool}}`, find nodes with `{{nodes_tool}}`, change graphs with `{{edit_graph_tool}}`, manage materials with `{{materials_tool}}`, key sockets with `{{look_keys_tool}}` and start over with `{{reset_look_tool}}`.

## Graphs

- `world`: record nodes wired into the World output describe the scene. Each output input takes one kind of record (many where marked); an unwired optional input is off:
{{world_ports}}
- Particle emitters go into the world's `emitters` input, as many as wanted: the ready-made `world.emitter.rain`, `world.emitter.snow`, `world.emitter.sparks`, `world.emitter.embers`, `world.emitter.petals`, `world.emitter.fireflies`, `world.emitter.dust`, `world.emitter.magic`, `world.emitter.confetti` and `world.emitter.leaves` take a few sockets (position in metres, yaw, pitch and roll, amount, area, radius or speed, wind, tint, start and end seconds, seed), while `world.emitter` exposes the whole description (shape, rate and bursts, lifetime, launch, forces, ground, size, opacity, four colours and emission over life, sprite flipbook, blend, lighting). Any socket may be driven by maths over Time or keyed; particles are born where the emitter was at that moment, so a moving emitter leaves a trail, and the same time always renders the same particles.
- `composite`: render layers (`image`, `depth`, `normals`, `emission` and more) flow through filter nodes (exposure, glow, grade, tone map, lens and the built-in effects) and per-pixel maths into the Output. The bare composite is exposure then a clamp tone map; the cinematic one adds motion blur, depth of field, glow, grade, a filmic tone map and lens.
- `material:<name>`: shader graphs (Principled and NPR closures, textures, maths) drawn on figure material slots. A slot with no material draws the model's imported material. Presets build ready material graphs: {{material_presets}}.
- A bare look renders like MikuMikuDance: a backdrop colour, one stage light with self shadows, a floor that shows only shadows, no tone curve and a fixed exposure. The cinematic starter adds a physical sky, contact shadows, fog, occlusion, reflections and indirect light.

## Editing

- Nodes have numeric ids; inputs and outputs are named ports. Connect an output to an input, or set an input's value. A linked input ignores its value; an unset input uses its default.
- Values: numbers, [x, y], [r, g, b] or [r, g, b, a] (linear RGB), true or false for switches, choices by name, file paths or text for file and name inputs. Angles are in degrees.
- Every edit call is checked whole; nothing changes when one edit is refused, and the reply says which edit and why.
- A material's declared parameters take values per slot, so one graph can serve many slots with different settings.

## Keying

- Any socket value of any graph and any slot parameter can be keyed over time; keys win over the set value. Numbers, colours and vectors blend along the arriving key's curve; switches, choices and files jump at keys.
- Removing a node drops the keys of its sockets.
