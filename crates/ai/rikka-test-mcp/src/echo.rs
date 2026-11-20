use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const ECHO: &str = r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *'"method":"initialize"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{},"prompts":{},"resources":{},"completions":{}},"serverInfo":{"name":"echo","title":"Echo","version":"1.0"},"instructions":"Say things back."}}\n' "$id" ;;
    *'"method":"tools/list"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"say","description":"Says it back","inputSchema":{"type":"object","properties":{"text":{"type":"string"}}}}]}}\n' "$id" ;;
    *'"method":"tools/call"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"echoed"}]}}\n' "$id" ;;
    *'"method":"prompts/list"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"prompts":[{"name":"greet","description":"Greets someone","arguments":[{"name":"who","required":true},{"name":"mood"}]}]}}\n' "$id" ;;
    *'"method":"prompts/get"'*)
      who=$(printf '%s' "$line" | sed -n 's/.*"who":"\([^"]*\)".*/\1/p')
      mood=$(printf '%s' "$line" | sed -n 's/.*"mood":"\([^"]*\)".*/\1/p')
      printf '{"jsonrpc":"2.0","id":%s,"result":{"messages":[{"role":"user","content":{"type":"text","text":"Greet %s, %s"}},{"role":"user","content":{"type":"resource","resource":{"uri":"note://hello","mimeType":"text/plain","text":"hello there"}}}]}}\n' "$id" "$who" "$mood" ;;
    *'"method":"completion/complete"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"completion":{"values":["Ada","Alan"],"hasMore":false}}}\n' "$id" ;;
    *'"method":"resources/list"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"resources":[{"uri":"note://hello","name":"hello","mimeType":"text/plain","description":"A greeting"}]}}\n' "$id" ;;
    *'"method":"resources/templates/list"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"resourceTemplates":[{"uriTemplate":"note://{name}","name":"note"}]}}\n' "$id" ;;
    *'"method":"resources/read"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"contents":[{"uri":"note://hello","mimeType":"text/plain","text":"hello there"}]}}\n' "$id" ;;
    *'"method":"ping"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{}}\n' "$id" ;;
  esac
done
"#;

#[must_use]
pub fn echo_server(folder: &Path) -> String {
    let path = folder.join("echo-server");
    std::fs::write(&path, ECHO).expect("echo server should be written");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("echo server should be executable");
    path.display().to_string()
}
