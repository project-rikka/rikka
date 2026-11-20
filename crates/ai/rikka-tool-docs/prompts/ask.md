+++
name = "AskUserQuestion"

[params]
questions = "All questions to ask, in order."
question = "The full question, ending with a question mark."
header = "A two- or three-word topic label."
multi_select = "Whether multiple options may be chosen. False if omitted."
options = "At least two distinct choices."
label = "The choice's short name, returned as its answer."
description = "What the choice means, in one sentence."
+++

Asks the person questions and pauses until they answer. Use it when a required choice depends on their intent and cannot be discovered from what is already there.

- Submit every question together.
- A free-text field accompanies the choices; do not offer "Other".
- A question of one answer returns either the chosen option or what was written in its place; a multi-select question may return both.
