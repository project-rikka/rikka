Respond with text only. Do not call any tool: you already hold everything you need, and a tool call will be refused and waste your only answer. Your whole answer is an `<analysis>` block followed by a `<summary>` block.

# Summarise this conversation

Write a continuation brief that replaces the conversation so far. The next agent receives only this brief and the project, so preserve every fact needed to continue the work without asking again or redoing it.

Before the brief, draft inside `<analysis>` tags. Walk through the conversation in order and, for each part, identify:

- what the person explicitly asked for and meant, and every correction or change of direction they gave;
- how you approached it, and the decisions, technical concepts and code patterns involved;
- exact details: file paths, full code snippets that matter, function signatures, edits made, commands run and their results;
- errors met, how each was fixed, and what the person said about them.

Then check the draft for accuracy and completeness. The draft is discarded; only the summary is kept.

Write the brief inside `<summary>` tags, under these numbered headings, leaving out any heading with nothing under it:

1. **Request and intent.** Everything the person asked for, in their terms, including working instructions and later changes.
2. **Key concepts.** Technologies, frameworks, conventions and constraints that shape the work.
3. **Files and code.** Each file read, created or changed: why it matters, what changed, and the snippets the next agent needs. Give the most recent work the most detail.
4. **Errors and fixes.** Each error, its cause, its fix, and any feedback from the person about it.
5. **Problem solving.** What was settled and why, rejected alternatives not to reconsider, and troubleshooting still open.
6. **The person's messages.** Every message the person wrote, apart from tool results, close to verbatim. These carry their intent and feedback.
7. **Pending tasks.** Everything explicitly asked for and not yet done.
8. **Current work.** Precisely what was being worked on just before this summary, with file names and snippets.
9. **Next step.** Only if it follows directly from the person's latest request and the current work: the next action, quoting the latest exchange verbatim to show where work stopped. If the last task was finished, give a next step only if the person asked for one.

Be dense and exact. Prefer paths, identifiers, signatures, numbers and exact errors to descriptive prose. Do not praise, soften failures, present uncertainty as fact, or claim unverified work is complete.

If the conversation opens with the summary of an earlier part, this brief replaces it too: carry forward what still holds, drop what later work settled or made stale, and merge the rest under the same headings rather than copying it whole.

Follow any instructions about summarising found in the standing project instructions or added below.

Reminder: no tool calls. Answer with an `<analysis>` block and then a `<summary>` block, nothing else.
