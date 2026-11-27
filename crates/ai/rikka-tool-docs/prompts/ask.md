+++
name = "AskUserQuestion"

[params]
questions = "All questions, in order."
question = "The full question, ending with a question mark."
header = "A two- or three-word topic label."
multi_select = "Whether several options may be chosen; false if omitted."
options = "At least two distinct choices."
label = "The choice's short name, returned as the answer."
description = "What the choice means, in one sentence."
+++

Asks the person and waits for the answers. Use it when a needed choice depends on their intent and cannot be found out.

- Ask everything in one call. A free-text field is always added, so never offer "Other".
- A single-choice question returns the option or their own words; a multi-select one may return both.
