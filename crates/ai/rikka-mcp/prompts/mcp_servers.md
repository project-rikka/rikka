+++
name = "McpServers"

[limits]
listed_tools = 24

[params]
action = "`list` shows every server; `add` adds or replaces one; `remove`, `enable` and `disable` change one; `sign_in` and `sign_out` handle a remote server's account; `reconnect` restarts one."
name = "The server's name: short, lowercase, letters, digits, `-` or `_`, such as `github`. Every action except `list` needs it."
scope = "Where the server is kept: {{scope_names}}. `add` needs it when there is more than one; the other actions find the server by name."
command = "Local servers only: the command line that starts it, quoted as in a shell, such as `npx -y @modelcontextprotocol/server-filesystem ~/Documents`."
cwd = "Local servers only: the folder it starts in."
env = "Local servers only: environment variables it is given, by name."
url = "Remote servers only: the Streamable HTTP endpoint, such as `https://mcp.example.com/mcp`."
headers = "Remote servers only: HTTP headers sent with every request, by name."
oauth = "Remote servers only, and only when the service's documentation says so: a registered OAuth app's `client_id`, optional `client_secret`, and the `callback_port` registered with it."
+++

Adds, changes and removes the Model Context Protocol (MCP) servers that give you tools from outside, and signs in to remote ones. Servers are kept in:

{{scopes}}

- A local server is a program this machine starts; it talks over its standard input and output. A remote server is reached at a URL.
- Take the package, image or URL from the server's own documentation, and prefer the service's official server; never guess one. Local servers usually start with `npx -y <package>` (Node), `uvx <package>` (Python) or `docker run -i --rm <image>`.
- Before `add`, `remove` or `enable`, say in one sentence what the server is and why.
- Never put a secret in a server unless the person gave it to you for that purpose. Name one kept in the environment as `${NAME}` (or `${NAME:-fallback}`); it is filled in when the server starts, in `command`, `cwd`, `env`, `url`, `headers` and `oauth`. Otherwise add the server without it and ask the person to type the key into the server's entry in the MCP settings, where it stays out of this conversation.
- Remote services with accounts usually sign in with OAuth and need no keys at all. When a reply says a server needs sign-in, tell the person, then call `sign_in`: it opens the service's page in their browser and waits until they finish.
- Every reply says how the server stands afterwards. A connected server is a kit in `{{tool_search_tool}}`: `select:<name>` loads all its tools with the server's instructions, and they are callable at once.
- `list` shows each server, where it is kept and how it stands; values of variables and headers are hidden.
