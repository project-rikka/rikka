+++
name = "Look"

[params]
graph = "A graph to read in full: `world`, `composite` or `material:<name>`."
figure = "A figure by name: its material slots, what each draws and the declared parameters of the graphs it uses."
+++

Reads the look as JSON. Without arguments: every graph with its node count (materials with their slot users and compile status), every figure's slots, keyed sockets and slot parameters with their key frames, and the material presets. With `graph`: each node's id, kind, title, outputs and every input as a link (`from` [node, port]), a set `value`, or its `default`; material parameters; problems that keep the graph from planning or compiling.
