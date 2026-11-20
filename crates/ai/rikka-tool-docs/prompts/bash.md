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
timeout = "Foreground wait in milliseconds: {{timeout_ms}} by default, capped at {{max_timeout_ms}}."
background = "Return immediately with a background job ID instead of waiting."
+++

Runs `bash -lc` in the working folder. Standard output and error return interleaved.

- Each call uses a fresh shell; directories and variables do not persist. Standard input is closed and pagers are disabled.
- A build or test command — cargo, rustc, a C or C++ compiler, make, ninja, cmake, meson, xmake, bazel, gradle, maven, dotnet, go, swift, zig, npm, pnpm, yarn, bun, deno, tsc, pytest, tox, mix, cabal, stack, composer, bundler, flutter, nix, docker and the like — answers with whether it succeeded and the path of the log its output went to, and nothing else. Read what you need out of that file: `rg -n "^error" <log>`, `tail -n 40 <log>`.
- Anything else answers with what it printed. Only the first {{output_bytes}} bytes come back; when there is more, the whole of it is in a log whose path is given.
- Logs are written to `.rikka/tmp`; the newest {{kept_logs}} are kept and the rest are dropped. They are yours to read, grep and delete.
- At `timeout`, a running command becomes a job and returns its output so far; a standalone `sleep` is killed. Read later output with `{{job_tool}}` after its completion notice.
- Set `background` for servers, watchers, or long commands whose result is not needed immediately; never append `&`.
