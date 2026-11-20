+++
name = "Agent"

[limits]
most = 8

[params]
tasks = "1 to {{most}} agents to start or continue."
label = "Short task name shown to the person; required for new agents, unchanged for existing ones."
task = "Self-contained brief: objective, scope, relevant paths and facts, and expected report."
job = "Existing agent job ID to continue; omit to start a new agent."
model = "Model to run it on, named as one of the agent models in the environment. Omit to run it on this conversation's own model."
effort = "How hard it reasons, clamped to what its model offers. Omit to reason as hard as this conversation does."
+++

Starts or continues explicitly requested agents in the same project; returns job IDs immediately. Read reports with `{{job_tool}}` after completion notices; never infer unread results.

- New agents inherit standing project instructions, not this conversation; `task` is their only task-specific context.
- Agents share the project and machine; assign disjoint edit scopes. They cannot delegate, and they cannot ask the person anything.
- Set `model` or `effort` only when the person or project instructions call for it. Otherwise give neither: the agent is this conversation over again, same model, same reasoning.
- `model` and `effort` settle when an agent starts, so a running agent takes neither; give them to a `job` that has already reported and it is resumed on that model instead.
- With `job`, `task` reaches a running agent at its next step or resumes an ended agent with its history. Each job may appear only once per call.
- Reuse `job` for changed briefs or partial, empty, or stopped runs. Each completed run sends a fresh notice.
