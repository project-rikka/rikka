---
title: Agent execution loop
description: rikka-agent streams model-tool turns with budgets, notices, cancellation and compaction; no approval gate.
about: crates/ai/rikka-agent
tags: [agent, ai]
---

- Depends only on `rikka-llm` and `rikka-tool`, which owns `Function`, `Handler`, `Reply`, `ToolKit`, `ToolKitSet` and `ToolKitRegistry` (re-exported here). `Agent::new(model)` configures instructions, tools, subagents, budget (default 16 turns), compaction, effort, output format, observer and cancellation.
- `run` starts empty, `session` keeps history, `resume` continues given history. Every turn streams to the observer: text, reasoning deltas, the whole answer, tool lifecycle, usage, compaction.
- Handlers are async `JSON → Reply` (or the `Handler` trait). A failure, missing tool or denial becomes a failed tool result, not a run error.
- `Ok(reply.failed())` fails while still passing `detail`.
- `Reply::image` goes to the model only; `Reply::shown` goes to the model and rides on `Event::ToolFinished { shown }` for watchers.
- `.without_vision()` (set by `rikka-thread` when the hired model's `ModelInfo::vision()` is false) replaces every picture, attached or in a tool result, with the `prompts/unseen.md` text in each request sent (`Inner::shown`, via `rikka_llm::Message::replacing_images`); the request itself and the stored thread keep the pictures.
- Each parallel call gets exactly one result in one message.
- `Notices` is a mailbox: work holds a `PendingNotice` from `Notices::pending()`. Notices ride along with the next tool results or become a user message. `Notices::wait()` keeps the run open (subagents). `is_quiet()` (no pending guard held, nothing unread) has no gap between a post and its guard's release. A `Notice::held()` notice (or everything posted before `Notices::hold()`) is delivered but should start no run; apps wake on `is_waking()`.
- `Interjections` (`.interjections(..)`) carry the person's mid-run messages: drained after each tool-results message (or after an answer, continuing the run) as separate user messages, each observed as `Event::Interjected { id }`; never drained once cancelled, and `withdraw(id)` succeeds only before the drain.
- Subagents are one-string tools. Only their final answer enters the parent, but their cost counts.
- Cancel drops running tool futures (blocking tasks continue) and fills outstanding calls with "stopped", so the transcript stays resumable.
- Every appended answer is observed whole as `Event::Responded(Message)` right after `TurnFinished { turn, usage, total_usage }` (`usage` is that turn's, threads keep the answer verbatim).
- The request is built once and appended. Tools held by `Deferred` and racked `ToolKit` tools are always declared, flagged `Tool::deferred()` until `Deferred::reveal`ed (only `open_toolkit` reveals); a reply loads one by reference with `Reply::load_tool(name)`, which leaves the declarations untouched. Only a reveal or a change of the `ToolKitRegistry` replaces the tool list.
- A `ToolKitRegistry` is `ToolKitRegistry::new([ToolKitSet, ..])`: each owner (running apps, MCP servers) restocks its own `ToolKitSet` of `ToolKit`s live; the first shelf wins a kit-name clash.
- `Compaction::new(window, CompactionPrompts).output(max_output)` folds before any model call once the gauge (last reported usage plus a guess for later messages; `Session::filled` seeds it) reaches `folds_at` = window − min(output, 20k) − 13k, floored at half. `Compaction::is_near_limit` is 20k earlier, for gauges. Window 0 never folds by itself.
- A fold summarises by resending the same request (cache-sharing) with the instruction appended; if that fails or is empty it retells the transcript as text, dropping the oldest rounds on overflow. `<analysis>` is stripped, `<summary>` unwrapped.
- `Event::Compacted { summary, compacted_messages }` carries the exact lead the live request continued from (framing, summary, recalled results of `Function::survives_compaction()` tools, the names of deferred tools loaded only in the folded part so the model reloads them, `Compaction::restating` text, and `CompactionPrompts::resuming` for automatic folds), which threads store verbatim. `Session::compact(focus)` is the person's fold, with `CompactionPrompts::focusing` before the focus. `CompactionPrompts::new(instruction)` defaults framing/focusing/resuming to `crates/ai/rikka-agent/prompts/{folded,focus,resumed}.md` (`.framing`, `.focus_heading`, `.resume_prompt` override); ADE and the shell use the defaults.
- A context overflow folds once (retelling, never resending) and retries. Three failed automatic folds stop folding for the run; failures emit `CompactFailed` and are nonfatal. `Event::Compacting` precedes `Compacted`.
- Caching is on by default for conversations, with an optional cache key.
- There is no approval gate: every Rikka agent runs its tools with full access, straight after `ToolStarted`.
- `answer()` forwards text and reasoning deltas and each finished hosted block as `Event::Hosted(HostedPart)`; client tool-call stream events are dropped, so watchers first see a call as `Event::ToolStarted` with its whole input.
- `.server_tools(..)` puts hosted tools on every request (a client tool of the same name is `DuplicateTool`). A `Stop::Paused` answer is resent as it is at once: no notice, interjection or user turn goes between.
