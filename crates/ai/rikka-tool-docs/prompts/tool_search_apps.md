+++
name = "ToolSearch"

[limits]
results = 5

[params]
query = "Use `select:Name` or `select:One,Two` for exact tools, and `select:Kit` for every tool of a running app or a connected MCP server. Otherwise search names and descriptions by words; prefix required words with `+`."
max_results = "Maximum word-search results; {{results}} by default."
+++

Finds deferred tools and loads their complete declarations, making them callable for the rest of the conversation. Use exact selection for a known name and word search for a capability. The deferred tools are listed in your instructions.

Running apps and connected MCP servers keep their tools here too, each as one kit named after the app or the server. The first time any of a kit's tools loads, its instructions come with it in an `<instructions>` block; follow them while using that kit.
