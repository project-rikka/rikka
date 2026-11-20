use rikka_assist::{Apps, UsedTool};
use rikka_chat::{Attached, CallSummary, Chat, Labels, SummarizeCall, symbol};
use rikka_format::truncate;
use rikka_llm::Effort;
use rikka_llm_accounts::Selection;
use rikka_mcp::ToolCall;
use rikka_thread::Call;
use rikka_tools::{ToolAction, naming};
use rikka_ui::{Handler, Label, MaterialSymbol, tr};

use crate::panel::Panel;

const GIST: usize = 96;

pub(super) struct ShellChat;

impl Labels for ShellChat {
    fn placeholder() -> Label {
        tr!("shell.assistant.placeholder")
    }

    fn answering() -> Label {
        tr!("shell.assistant.working")
    }

    fn folding() -> Label {
        tr!("shell.assistant.folding")
    }

    fn folded() -> Label {
        tr!("shell.assistant.folded")
    }

    fn noticed() -> Label {
        tr!("shell.assistant.noticed")
    }

    fn main_thread() -> Label {
        tr!("shell.assistant.main")
    }

    #[expect(
        clippy::cast_possible_wrap,
        reason = "a count of lines, which no output has more of than an i64 holds"
    )]
    fn more_lines(count: usize) -> Label {
        tr!("shell.assistant.rest", count = count as i64)
    }

    fn pick_model() -> Label {
        tr!("shell.assistant.model")
    }

    fn reasoning() -> Label {
        tr!("shell.assistant.reasoning")
    }

    fn write_answer() -> Label {
        tr!("shell.assistant.ask.written")
    }

    fn stop_asking() -> Label {
        tr!("shell.assistant.ask.stop")
    }

    fn back() -> Label {
        tr!("shell.assistant.ask.back")
    }

    fn next() -> Label {
        tr!("shell.assistant.ask.next")
    }

    fn send_answers() -> Label {
        tr!("shell.assistant.ask.answer")
    }

    fn choose_files() -> Label {
        tr!("shell.assistant.attach.chooser")
    }

    fn attach() -> Label {
        tr!("shell.assistant.attach.accept")
    }

    fn unreadable() -> Label {
        tr!("shell.assistant.attach.unreadable")
    }

    fn refused(reason: &str) -> Label {
        tr!("shell.assistant.attach.refused", reason = reason)
    }

    #[expect(
        clippy::cast_possible_wrap,
        reason = "a count of files, which no drop holds more of than an i64 holds"
    )]
    fn refused_more(count: usize) -> Label {
        tr!("shell.assistant.attach.rest", count = count as i64)
    }

    fn dropped() -> Label {
        tr!("shell.assistant.attach.dropped")
    }

    fn pasted() -> Label {
        tr!("shell.assistant.attach.pasted")
    }

    fn compact_hint() -> Label {
        tr!("shell.assistant.command.compact.hint")
    }

    fn compact_about() -> Label {
        tr!("shell.assistant.command.compact.about")
    }
}

impl Chat for ShellChat {
    fn offer() -> Handler {
        Handler::new(Panel::offer)
    }

    fn send() -> Handler {
        Handler::new(Panel::send)
    }

    fn stop() -> Handler {
        Handler::new(Panel::stop)
    }

    fn attached() -> Handler<(Attached,)> {
        Handler::new(Panel::attach_all)
    }

    fn unattach() -> Handler<(usize,)> {
        Handler::new(Panel::unattach)
    }

    fn step() -> Handler<(bool,)> {
        Handler::new(Panel::step)
    }

    fn agree() -> Handler {
        Handler::new(Panel::agree)
    }

    fn take() -> Handler<(usize,)> {
        Handler::new(Panel::take)
    }

    fn point() -> Handler<(usize,)> {
        Handler::new(Panel::point)
    }

    fn dismiss() -> Handler {
        Handler::new(Panel::dismiss)
    }

    fn choose_option() -> Handler<(usize,)> {
        Handler::new(Panel::chose)
    }

    fn write_in() -> Handler {
        Handler::new(Panel::wrote)
    }

    fn go_back() -> Handler {
        Handler::new(Panel::back)
    }

    fn go_ahead() -> Handler {
        Handler::new(Panel::ahead)
    }

    fn stop_job() -> Handler<(u64,)> {
        Handler::new(Panel::stop_job)
    }

    fn pick_models() -> Handler<(bool,)> {
        Handler::new(Panel::pick_models)
    }

    fn unfold_reasoning() -> Handler<(Option<f32>,)> {
        Handler::new(Panel::unfold_reasoning)
    }

    fn chose_model() -> Handler<(Selection,)> {
        Handler::new(Panel::chose_model)
    }

    fn chose_effort() -> Handler<(Effort,)> {
        Handler::new(Panel::chose_effort)
    }
}

pub(super) struct Calls {
    apps: Apps,
}

impl Calls {
    pub(crate) fn new(apps: Apps) -> Calls {
        Calls { apps }
    }
}

impl SummarizeCall for Calls {
    fn summarize<'a>(&self, call: &'a Call) -> CallSummary<'a> {
        if let Some(used) = self.apps.used_tool(call.tool()) {
            return app(&used, call);
        }
        match ToolCall::parse(call.tool(), call.input()) {
            Some(called) => server(called, call),
            None => desk(call),
        }
    }
}

fn app<'a>(used: &UsedTool, call: &'a Call) -> CallSummary<'a> {
    CallSummary {
        name: format!("{} · {}", used.app(), used.tool()),
        symbol: MaterialSymbol::WIDGETS,
        gist: used
            .gist(call.input())
            .map(|gist| truncate(&gist, GIST).into_owned()),
        action: ToolAction::Unknown,
        pictures: true,
    }
}

fn server<'a>(called: ToolCall<'_>, call: &'a Call) -> CallSummary<'a> {
    let name = match called {
        ToolCall::Tool { server, tool } => format!("{server} · {tool}"),
        ToolCall::Managed { .. } | ToolCall::Resources { .. } => call.tool().to_owned(),
    };
    CallSummary {
        name,
        symbol: MaterialSymbol::EXTENSION,
        gist: called.gist().map(|gist| truncate(&gist, GIST).into_owned()),
        action: ToolAction::Unknown,
        pictures: true,
    }
}

fn desk(call: &Call) -> CallSummary<'_> {
    let deed = ToolAction::classify(naming(), call.tool(), call.input());
    let gist = match &deed {
        ToolAction::Operate(operation) => Some(operation.to_string()),
        other => other.subject().map(|gist| gist.trim().to_owned()),
    };
    CallSummary {
        name: deed.name(call.tool()).to_owned(),
        symbol: symbol(&deed),
        gist: gist.map(|gist| truncate(&gist, GIST).into_owned()),
        pictures: true,
        action: deed,
    }
}
