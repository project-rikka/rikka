+++
name = "Jobs"

[limits]
output_bytes = 30000

[params]
action = "`list` known jobs and states, read unread `output`, or `stop` one."
id = "Job ID required for `output` and `stop`."
+++

Manages background `{{bash_tool}}` commands and the runs of any agent started as a job.

- `output` hands over the next {{output_bytes}} bytes a job printed and advances the read position; read again for what is left, and anything lost while it ran is stated.
- A job running a build or test command hands over the path of the log its output goes to instead, which is `{{bash_tool}}`'s own way with such a command.
- `stop` kills a command's process group or cancels an agent and suppresses its completion notice.
- Stop running jobs you no longer need; they otherwise continue across turns.
