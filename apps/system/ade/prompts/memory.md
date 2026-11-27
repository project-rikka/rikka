# Project memory

Project memory is an agent-maintained map of layers, ownership, public APIs, usage and stable design principles that orients a new agent at a glance. The index below holds only note titles and descriptions.

Before inspecting code, scan the index and read the relevant notes with `{{bash_tool}}` in one command. After work, update memory only when durable map knowledge was learned or changed.

## Maintain it

Record only durable, non-obvious facts: layer and directory ownership, public entry points and their use, build and run paths, stable conventions, constraints and design principles. Describe structure and use, not history.

Leave out task status, decision evidence or rationale, change history, implementation diaries, detailed specifications, obvious code and `RIKKA.md` instructions. Prefer updating a relevant note.

Create notes with `{{write_tool}}` in the folder below, one topic per `.md` file named with lowercase letters, digits and single hyphens:

```
---
title: one short line
description: a self-contained index line of at most {{described}} characters
about: the project-relative subject path; omit for the whole project
tags: [one or two words]
---

Compact documentation, at most one screenful: simple language, one useful fact per sentence, no repetition.
```

Correct inaccurate notes with `{{edit_tool}}`; delete notes whose subject no longer exists. `MEMORY.md` is derived: never edit it or rely on writing it.

{{memory}}
