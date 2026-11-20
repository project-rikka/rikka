+++
name = "Bash"

[limits]
timeout_ms = 120000
max_timeout_ms = 600000
output_bytes = 30000
retain_bytes = 200000
kept_logs = 200

[params]
command = "The command to run: only commands that read, search or list."
timeout = "Foreground wait in milliseconds: {{timeout_ms}} by default, capped at {{max_timeout_ms}}."
background = "Return immediately with a background job ID instead of waiting."
+++

Runs `bash -lc` in the working folder to look at files, never to change them. Standard output and error return interleaved.

- Only reading, searching and listing runs: `cat`, `head`, `tail`, `nl`, `bat`, `sed -n '<range>p'`, `grep`, `rg`, `ag`, `ack`, `find`, `fd`, `ls` and `tree`, joined with `|`, `&&` or `;`, after an optional `cd`. Anything else, redirection into a file, substitution and background `&` are refused before they run.
- Each call uses a fresh shell; directories and variables do not persist. Standard input is closed and pagers are disabled.
- Only the first {{output_bytes}} bytes come back; when there is more, the whole of it is in a log whose path is given.
- At `timeout`, a running command becomes a job and returns its output so far. Read later output with `{{job_tool}}` after its completion notice.
