use rikka_app::prelude::*;
use rikka_chat::{AmendActions, Attached, Chat, Labels};
use rikka_llm::Effort;
use rikka_llm_accounts::Selection;

use crate::project::act::{act, given, given_with_setup, with_setup};

pub(crate) struct AdeChat;

impl Labels for AdeChat {
    fn placeholder() -> Label {
        tr!("ade.chat.placeholder")
    }

    fn answering() -> Label {
        tr!("ade.chat.answering")
    }

    fn folding() -> Label {
        tr!("ade.chat.folding")
    }

    fn folded() -> Label {
        tr!("ade.chat.folded")
    }

    fn noticed() -> Label {
        tr!("ade.chat.noticed")
    }

    fn main_thread() -> Label {
        tr!("ade.chat.main")
    }

    fn more_lines(count: usize) -> Label {
        tr!("ade.chat.rest", count = count as i64)
    }

    fn pick_model() -> Label {
        tr!("ade.chat.pick")
    }

    fn reasoning() -> Label {
        tr!("ade.chat.reasoning")
    }

    fn write_answer() -> Label {
        tr!("ade.chat.ask.write")
    }

    fn stop_asking() -> Label {
        tr!("ade.chat.ask.stop")
    }

    fn back() -> Label {
        tr!("ade.chat.ask.back")
    }

    fn next() -> Label {
        tr!("ade.chat.ask.next")
    }

    fn send_answers() -> Label {
        tr!("ade.chat.ask.send")
    }

    fn choose_files() -> Label {
        tr!("ade.chat.attach.chooser")
    }

    fn attach() -> Label {
        tr!("ade.chat.attach.accept")
    }

    fn unreadable() -> Label {
        tr!("ade.chat.attach.unreadable")
    }

    fn refused(reason: &str) -> Label {
        tr!("ade.chat.attach.refused", reason = reason)
    }

    fn refused_more(count: usize) -> Label {
        tr!("ade.chat.attach.rest", count = count as i64)
    }

    fn dropped() -> Label {
        tr!("ade.chat.attach.dropped")
    }

    fn pasted() -> Label {
        tr!("ade.chat.attach.pasted")
    }

    fn compact_hint() -> Label {
        tr!("ade.command.compact.hint")
    }

    fn compact_about() -> Label {
        tr!("ade.command.compact.about")
    }
}

impl Chat for AdeChat {
    fn offer() -> Handler {
        Handler::new(act(|project| project.offer()))
    }

    fn send() -> Handler {
        Handler::new(with_setup(|project, setup| project.send(setup)))
    }

    fn stop() -> Handler {
        Handler::new(act(|project| project.stop()))
    }

    fn attached() -> Handler<(Attached,)> {
        Handler::new(given(|project, attached: Attached| {
            project.attach_all(attached);
        }))
    }

    fn unattach() -> Handler<(usize,)> {
        Handler::new(given(|project, at: usize| project.unattach(at)))
    }

    fn step() -> Handler<(bool,)> {
        Handler::new(given(|project, down: bool| project.step(down)))
    }

    fn agree() -> Handler {
        Handler::new(act(|project| project.agree()))
    }

    fn take() -> Handler<(usize,)> {
        Handler::new(given(|project, at: usize| project.take(at)))
    }

    fn point() -> Handler<(usize,)> {
        Handler::new(given(|project, at: usize| project.point(at)))
    }

    fn dismiss() -> Handler {
        Handler::new(act(|project| project.dismiss()))
    }

    fn choose_option() -> Handler<(usize,)> {
        Handler::new(given(|project, at: usize| project.pick_option(at)))
    }

    fn write_in() -> Handler {
        Handler::new(act(|project| project.wrote()))
    }

    fn go_back() -> Handler {
        Handler::new(act(|project| project.back()))
    }

    fn go_ahead() -> Handler {
        Handler::new(act(|project| project.ahead()))
    }

    fn stop_job() -> Handler<(u64,)> {
        Handler::new(given(|project, id: u64| project.stop_job(id)))
    }

    fn pick_models() -> Handler<(bool,)> {
        Handler::new(given(|project, open: bool| project.picking(open)))
    }

    fn unfold_reasoning() -> Handler<(Option<f32>,)> {
        Handler::new(given(|project, at: Option<f32>| project.reasoning(at)))
    }

    fn chose_model() -> Handler<(Selection,)> {
        Handler::new(given_with_setup(|project, setup, chosen: Selection| {
            project.model(chosen.account(), chosen.model(), setup);
        }))
    }

    fn chose_effort() -> Handler<(Effort,)> {
        Handler::new(given_with_setup(|project, setup, effort: Effort| {
            project.effort(effort, setup);
        }))
    }

    fn watch_agent() -> Option<Handler<(u64,)>> {
        Some(Handler::new(given(|project, job: u64| {
            project.watch_agent(job)
        })))
    }

    fn watch_main() -> Option<Handler> {
        Some(Handler::new(act(|project| project.watch_main())))
    }

    fn amend() -> Option<AmendActions> {
        Some(AmendActions {
            edit: Handler::new(given(|project, turn: u64| project.amend(turn))),
            resend: Handler::new(act(|project| project.amended())),
            cancel: tr!("ade.chat.amend.cancel"),
            send: tr!("ade.chat.amend.send"),
            restores: |count| tr!("ade.chat.amend.restore", count = count as i64),
        })
    }

    fn interjects() -> bool {
        true
    }

    fn recall() -> Option<Handler<(u64,)>> {
        Some(Handler::new(given(|project, id: u64| project.recall(id))))
    }

    fn withdraw() -> Option<Handler<(u64,)>> {
        Some(Handler::new(given(|project, id: u64| project.withdraw(id))))
    }
}
