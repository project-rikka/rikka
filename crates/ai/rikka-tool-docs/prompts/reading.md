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
timeout = "Foreground wait in milliseconds: {{timeout_ms}} by default, at most {{max_timeout_ms}}."
background = "Return a job ID at once instead of waiting."
+++

Runs `bash -lc` in the working folder to look at files, never to change them: a fresh shell per call, stdin closed, pagers off, stdout and stderr interleaved.

- Only `cat`, `head`, `tail`, `nl`, `bat`, `sed -n '<range>p'`, `grep`, `rg`, `ag`, `ack`, `find`, `fd`, `ls` and `tree` run, joined with `|`, `&&` or `;` after an optional `cd`. Anything else, file redirection, substitution and background `&` are refused.
- Output comes back up to {{output_bytes}} bytes, and whole in a log past that.
- A command still running at `timeout` becomes a `{{job_tool}}` job whose completion is announced.
- Read all the next step needs in one command: `cat` files under a few hundred lines; for larger ones, `sed -n '<first>,<last>p'` whole functions or sections around `rg -n` hits.
