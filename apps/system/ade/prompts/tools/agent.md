+++
name = "Agent"

[limits]
most = 8

[params]
tasks = "1 to {{most}} agents to start or continue."
label = "Short task name shown to the person; required for new agents, unchanged for existing ones."
task = "Self-contained brief: objective, scope, relevant paths and facts, expected report."
job = "Job ID of an existing agent to continue; omit to start a new one."
model = "One of the agent models in the environment; omit to use this conversation's model."
effort = "How hard it reasons, clamped to what its model offers; omit to match this conversation."
+++

Starts or continues agents in the same project and returns their job IDs at once. Read each report with `{{job_tool}}` after its completion notice; never guess unread results.

- New agents get the standing project instructions but not this conversation; `task` is their only brief.
- Agents share the project and machine, so give them disjoint edit scopes. They cannot delegate or ask the person.
- Set `model` or `effort` only when the person or project instructions call for it. Both settle when an agent starts: a running agent cannot take them, a reported one resumes on them.
- With `job`, `task` reaches a running agent at its next step or resumes an ended one with its history; reuse it for changed briefs and partial, empty or stopped runs. Name each job once per call.
