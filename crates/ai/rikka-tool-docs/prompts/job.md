+++
name = "Jobs"

[limits]
output_bytes = 30000

[params]
action = "`list` jobs and states, read a job's unread `output`, or `stop` it."
id = "Job ID, required for `output` and `stop`."
+++

Manages background `{{bash_tool}}` commands and agents run as jobs.

- `output` returns the next {{output_bytes}} bytes since the last read, or a build or test job's log path, and says if any output was lost.
- `stop` kills a command's process group or cancels an agent, with no completion notice. Stop jobs you no longer need; they outlive the turn.
