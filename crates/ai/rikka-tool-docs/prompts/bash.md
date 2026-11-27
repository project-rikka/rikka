+++
name = "Bash"

[limits]
timeout_ms = 120000
max_timeout_ms = 600000
output_bytes = 30000
retain_bytes = 200000
kept_logs = 200

[params]
command = "The command to run."
timeout = "Foreground wait in milliseconds: {{timeout_ms}} by default, at most {{max_timeout_ms}}."
background = "Return a job ID at once instead of waiting."
+++

Runs `bash -lc` in the working folder: a fresh shell per call, stdin closed, pagers off, stdout and stderr interleaved.

- Build and test commands (cargo, bazel, make, npm, pytest, go, nix, docker and the like) answer only with success and a log path under `.rikka/tmp`; read what you need from it, such as `rg -n "^error" <log>` or `tail -n 40 <log>`. Run them plainly, without filters, pipes, redirections or timeout wrappers.
- Other commands return their output up to {{output_bytes}} bytes, and the whole in a log past that. The newest {{kept_logs}} logs are kept.
- A command still running at `timeout`, or started with `background`, becomes a `{{job_tool}}` job whose completion is announced; never wait with `sleep` or polling. Use `background` for servers and watchers, never `&`.
- Search with `rg`, not `grep`. Read all the next step needs in one command, numbering the lines of files you may change: `cat -n` files under a few hundred lines; for larger ones, `cat -n <file> | sed -n '<first>,<last>p'` whole functions or sections around `rg -n` hits. Numbered lines of working-folder files come back anchored (`12#ab`) for `{{edit_tool}}`; read files you only study without numbers.
- Change files only with `{{edit_tool}}` and `{{write_tool}}`, never `sed -i`, `awk`, `perl`, heredocs or redirection. The one exception is a broad, mechanical change by a focused script, whose diff you then review.
