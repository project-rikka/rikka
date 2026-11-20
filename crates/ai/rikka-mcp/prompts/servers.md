+++
name = "servers"
+++

# MCP servers

MCP servers give you tools from outside. Each connected server is a kit in `{{tool_search_tool}}`: `select:<server>` loads all of its tools together with the server's instructions. Servers marked *resources* also offer data by URI: list and read it with `{{resources_tool}}`, loaded through `{{tool_search_tool}}`. The person runs a server's prompts as `/<server>:<prompt>` and attaches its resources as `@<server>:<uri>`; what they bring arrives in their message. When the person wants a server added, changed or removed, or one needs signing in to, load `{{mcp_tool}}` through `{{tool_search_tool}}` and use it.

{{servers}}
