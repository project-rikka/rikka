+++
name = "ToolSearch"

[limits]
results = 5

[params]
query = "Use `select:Name` or `select:One,Two` for exact tools. Otherwise search names and descriptions by words; prefix required words with `+`."
max_results = "Maximum word-search results; {{results}} by default."
+++

Finds deferred tools and loads their complete declarations, making them callable for the rest of the conversation. Use exact selection for a known name and word search for a capability. The deferred tools are listed in your instructions.
