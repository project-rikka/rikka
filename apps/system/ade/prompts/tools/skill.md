+++
name = "Skill"

[params]
skill = "The exact name of a skill listed under Skills in your instructions."
+++

Loads a listed skill's instructions and inventory of supporting scripts, references, and templates. Use it only when a listed skill matches the task.

- Load a matching skill before doing its work; load none when no skill applies.
- Do not claim to use a skill without loading it, and load each skill only once per conversation.
- The result lists supporting files but does not read them. Use `{{bash_tool}}` only when the skill directs you to one.
- Supporting paths are relative to the returned skill directory.
