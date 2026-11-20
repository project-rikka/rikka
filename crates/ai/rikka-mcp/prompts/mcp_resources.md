+++
name = "McpResources"

[limits]
listed = 100
read_chars = 100000

[params]
action = "`list` shows the resources and resource templates the servers offer; `read` reads one resource."
server = "The server's name, as in the list of MCP servers. `list` without it covers every server; `read` needs it."
uri = "`read` only: the resource's URI, taken from `list`, from a tool's reply, or a resource template with its placeholders filled in."
+++

Lists and reads the resources of Model Context Protocol (MCP) servers: files, records, pages and other data a server makes available by URI.

- `list` shows each resource's URI, name, type and description, at most {{listed}} per server, and each resource template: a URI with `{placeholders}` that you fill in to read something specific.
- `read` returns the resource's text and shows its pictures. Text beyond {{read_chars}} characters is cut; other binary data is only described.
- The person attaches a resource by writing `@<server>:<uri>`; it arrives with their message and needs no reading.
