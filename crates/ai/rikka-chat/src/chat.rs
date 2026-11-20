use rikka_llm::Effort;
use rikka_llm_accounts::Selection;
use rikka_ui::{Handler, Label};

use crate::attaching::Attached;

pub trait Labels: 'static {
    fn placeholder() -> Label;

    fn answering() -> Label;

    fn folding() -> Label;

    fn folded() -> Label;

    fn noticed() -> Label;

    fn main_thread() -> Label;

    fn more_lines(count: usize) -> Label;

    fn pick_model() -> Label;

    fn reasoning() -> Label;

    fn write_answer() -> Label;

    fn stop_asking() -> Label;

    fn back() -> Label;

    fn next() -> Label;

    fn send_answers() -> Label;

    fn choose_files() -> Label;

    fn attach() -> Label;

    fn unreadable() -> Label;

    fn refused(reason: &str) -> Label;

    fn refused_more(count: usize) -> Label;

    fn dropped() -> Label;

    fn pasted() -> Label;

    fn compact_hint() -> Label;

    fn compact_about() -> Label;
}

#[derive(Debug)]
pub struct AmendActions {
    pub edit: Handler<(u64,)>,
    pub resend: Handler,
    pub cancel: Label,
    pub send: Label,
    pub restores: fn(usize) -> Label,
}

pub trait Chat: Labels {
    fn offer() -> Handler;

    fn send() -> Handler;

    fn stop() -> Handler;

    fn attached() -> Handler<(Attached,)>;

    fn unattach() -> Handler<(usize,)>;

    fn step() -> Handler<(bool,)>;

    fn agree() -> Handler;

    fn take() -> Handler<(usize,)>;

    fn point() -> Handler<(usize,)>;

    fn dismiss() -> Handler;

    fn choose_option() -> Handler<(usize,)>;

    fn write_in() -> Handler;

    fn go_back() -> Handler;

    fn go_ahead() -> Handler;

    fn stop_job() -> Handler<(u64,)>;

    fn pick_models() -> Handler<(bool,)>;

    fn unfold_reasoning() -> Handler<(Option<f32>,)>;

    fn chose_model() -> Handler<(Selection,)>;

    fn chose_effort() -> Handler<(Effort,)>;

    fn watch_agent() -> Option<Handler<(u64,)>> {
        None
    }

    fn watch_main() -> Option<Handler> {
        None
    }

    fn amend() -> Option<AmendActions> {
        None
    }

    fn interjects() -> bool {
        false
    }

    fn recall() -> Option<Handler<(u64,)>> {
        None
    }

    fn withdraw() -> Option<Handler<(u64,)>> {
        None
    }
}
