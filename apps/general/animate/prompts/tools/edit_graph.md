+++
name = "EditGraph"

[params]
graph = "`world`, `composite` or `material:<name>`."
edits = "Edits applied in order as one change."
edit = "One of: {\"add\": kind, \"name\": a name later edits use instead of the id, \"at\": [x, y], \"values\": {port: value}}; {\"remove\": node}; {\"connect\": [node, output], \"to\": [node, input]}; {\"disconnect\": [node, input], \"from\": [node, output]} (without `from` every link into the input); {\"set\": [node, input], \"value\": value}; {\"unset\": [node, input]}. A node is its id or an added name."
+++

Changes one graph as one undo step. The whole batch is checked before anything changes: unknown nodes and ports, values that do not fit an input, links between incompatible types and loops are refused with the edit's index. The reply gives the ids of named added nodes.

- Wire a world record into the output: add `world.light`, then connect [light, "light"] to [output, "lights"].
- Keyed sockets keep driving their input after `set`; key them with `{{look_keys_tool}}`.
