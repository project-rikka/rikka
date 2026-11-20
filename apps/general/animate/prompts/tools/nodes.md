+++
name = "Nodes"

[params]
graph = "The kind of graph the nodes are for."
search = "Words to find in a node's id, title or description; with it each match lists its inputs (type, default, range, options, unit) and outputs."
+++

Lists the nodes a graph kind can hold. Each entry's `kind` is what `{{edit_graph_tool}}` `add` takes, verbatim. Without `search` only kinds and titles are listed; search before adding to learn a node's ports.
