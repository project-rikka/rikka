+++
name = "ToolSearch"

[limits]
results = 5

[params]
query = "Use `select:Name` or `select:One,Two` for exact tools, and `select:Kit` for every tool of a running app or a connected MCP server. Otherwise search names and descriptions by words; prefix required words with `+`."
max_results = "Maximum word-search results; {{results}} by default."
+++

Loads the declarations of deferred tools, listed in your instructions, so they can be called for the rest of the conversation.

Each running app and connected MCP server keeps its tools here as one kit under its name. A kit's instructions arrive in an `<instructions>` block the first time any of its tools loads; follow them while using it.
