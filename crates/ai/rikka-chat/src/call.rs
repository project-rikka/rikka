use rikka_llm::Value;
use rikka_thread::{Call, OutputLog, SideEffect};
use rikka_tools::ToolAction;

use crate::call_summary::{CallSummary, SummarizeCall};
use crate::chat::Chat;
use crate::expanded::Expanded;
use crate::prelude::*;
use crate::printed::PrintedDocuments;
use crate::{console, diff, question_answers, shown};
use rikka_widgets::{column, row};

const GLYPH: f32 = 15.0;
const PITCH: f32 = 1.4;
const RISE: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CallState {
    Settled,
    Arriving,
}

pub(crate) struct CallContext<'a> {
    pub(crate) summarizer: &'a dyn SummarizeCall,
    pub(crate) expanded: &'a Expanded,
    pub(crate) documents: &'a PrintedDocuments,
}

pub(crate) fn view<C: Chat>(call: &Call, context: &CallContext<'_>) -> Element {
    render::<C>(call, context, CallState::Settled)
}

pub(crate) fn arriving<C: Chat>(call: &Call, context: &CallContext<'_>) -> Element {
    render::<C>(call, context, CallState::Arriving)
        .transition((fade() + offset(0.0, RISE)).enter(Curve::Spring(Spring::DEFAULT_SPATIAL)))
}

fn render<C: Chat>(call: &Call, context: &CallContext<'_>, state: CallState) -> Element {
    let summary = context.summarizer.summarize(call);
    let failed = call.is_failed();

    row! {
        mark(call, &summary, state);
        column! {
            header(&summary);
            match (&summary.action, failed) {
                (ToolAction::Command { command }, _) => console::view(
                    console::CommandCall {
                        call: call.id(),
                        command,
                        output: call.output(),
                        log: output_log(call),
                        failed,
                        open: context.expanded.holds(call.id()),
                    },
                    context.documents,
                )
                .any(),
                (_, true) => Element::empty(),
                (ToolAction::Write { path, content }, false) => diff::arriving::<C>(path, content).any(),
                (ToolAction::Edit { path }, false) => changed::<C>(path, call),
                (ToolAction::Ask { questions }, false) => question_answers::view(questions, call.output()).any(),
                (_, false) => Element::empty(),
            }
            if summary.pictures && !failed && !call.displayed().is_empty() {
                shown::view(call.displayed());
            }
        }
        .gap(Space::SNUG)
        .grow(1.0)
        .fill_width();
    }
    .gap(Space::SNUG)
    .align_items(Align::Start)
    .fill_width()
    .build()
    .keyed(call.id())
}

fn changed<C: Chat>(path: &str, call: &Call) -> Element {
    match call.detail().and_then(SideEffect::file_change) {
        Some(change) => diff::changed::<C>(change).any(),
        None => diff::arriving::<C>(path, &proposed(call.input())).any(),
    }
}

fn output_log(call: &Call) -> Option<&OutputLog> {
    call.detail().and_then(SideEffect::output_log)
}

fn proposed(input: &Value) -> String {
    input
        .get("ops")
        .and_then(Value::as_array)
        .map(|ops| {
            ops.iter()
                .filter_map(|op| op.get("text").and_then(Value::as_str))
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn header(summary: &CallSummary<'_>) -> impl View {
    let gist = summary.gist.clone().filter(|_| !has_body(&summary.action));
    row! {
        text(summary.name.clone()).label().medium().color(palette().text);
        if let Some(gist) = gist {
            text(gist).label().muted().grow(1.0);
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Start)
    .fill_width()
}

fn has_body(action: &ToolAction<'_>) -> bool {
    matches!(
        action,
        ToolAction::Command { .. }
            | ToolAction::Write { .. }
            | ToolAction::Edit { .. }
            | ToolAction::Ask { .. }
    )
}

fn mark(call: &Call, summary: &CallSummary<'_>, state: CallState) -> Element {
    let running = call.is_running();
    let glyph = match running {
        true => spinner().extent(Length::px(GLYPH)).any(),
        false => {
            let (symbol, colour) = match call.is_failed() {
                true => (MaterialSymbol::ERROR, palette().danger),
                false => (summary.symbol, palette().text_muted),
            };
            icon(symbol).extent(Length::px(GLYPH)).color(colour).any()
        }
    };
    let glyph = match state {
        CallState::Arriving => glyph.transition(fade_scale()).keyed(running),
        CallState::Settled => glyph,
    };

    container(glyph)
        .height(Length::px(line()))
        .align_y(Align::Center)
        .any()
}

fn line() -> f32 {
    (env().font_size * TextRole::Label.scale() * PITCH).round()
}
