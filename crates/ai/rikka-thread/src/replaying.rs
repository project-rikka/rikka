use std::collections::HashSet;
use std::sync::Arc;

use rikka_llm::{Content, Image, Message, Role, ToolResult, ToolUse};

use crate::call::Call;
use crate::replay::Replay;
use crate::round::Round;
use crate::turn::Body;

#[derive(Debug, Default)]
pub(crate) struct ResidentBodies {
    requested: HashSet<String>,
}

impl ResidentBodies {
    pub(crate) fn new(bodies: &[Arc<Body>]) -> ResidentBodies {
        let requested = bodies
            .iter()
            .flat_map(|body| body.rounds().iter().flat_map(Round::calls))
            .map(|call| call.id().to_owned())
            .collect();
        ResidentBodies { requested }
    }
}

pub(crate) fn model_turn(
    transcript: &mut Vec<Message>,
    body: &Body,
    thinking: bool,
    bodies: &ResidentBodies,
) {
    let mut covered: Vec<&Round> = Vec::new();
    for round in body.rounds() {
        covered.push(round);
        if let Some(replay) = round.replay() {
            replayed(transcript, replay, &covered, thinking, bodies);
            covered.clear();
        }
    }

    match body.replay() {
        Some(replay) => replayed(transcript, replay, &covered, thinking, bodies),
        None => {
            for round in covered {
                retold(transcript, round);
            }
            if !body.text().trim().is_empty() {
                spoke(transcript, vec![Content::text(body.text())]);
            }
        }
    }
}

fn replayed(
    transcript: &mut Vec<Message>,
    replay: &Replay,
    covered: &[&Round],
    thinking: bool,
    bodies: &ResidentBodies,
) {
    spoke(
        transcript,
        replay.content(thinking, |id| bodies.requested.contains(id)),
    );
    let calls: Vec<&Call> = replay
        .requested_ids()
        .filter_map(|id| {
            covered
                .iter()
                .flat_map(|round| round.calls())
                .find(|call| call.id() == id)
        })
        .collect();
    let notices: Vec<String> = covered
        .iter()
        .flat_map(|round| round.notices())
        .cloned()
        .collect();
    answer(transcript, &calls, &notices);
}

fn retold(transcript: &mut Vec<Message>, round: &Round) {
    let calls: Vec<&Call> = round
        .calls()
        .iter()
        .filter(|call| !call.is_hosted())
        .collect();
    let text_content = (!round.text().trim().is_empty()).then(|| Content::text(round.text()));
    let tool_uses = calls
        .iter()
        .map(|call| Content::ToolUse(ToolUse::new(call.id(), call.tool(), call.input().clone())));
    spoke(
        transcript,
        text_content.into_iter().chain(tool_uses).collect(),
    );
    answer(transcript, &calls, round.notices());
}

fn spoke(transcript: &mut Vec<Message>, content: Vec<Content>) {
    if !content.is_empty() {
        transcript.push(Message::new(Role::Assistant, content));
    }
}

fn answer(transcript: &mut Vec<Message>, calls: &[&Call], notices: &[String]) {
    let notice_text = (!notices.is_empty()).then(|| notices.join("\n"));
    if calls.is_empty() {
        transcript.extend(notice_text.map(Message::user));
        return;
    }
    let answers = Message::tool_results(calls.iter().map(|call| tool_result(call)));
    transcript.push(match notice_text {
        Some(notice_text) => answers.with(Content::text(notice_text)),
        None => answers,
    });
}

fn tool_result(call: &Call) -> ToolResult {
    let output = call.output().unwrap_or_default();
    let answer = match call.is_failed() {
        true => ToolResult::failed(call.id(), output),
        false => ToolResult::new(call.id(), output),
    };
    let pictured = call
        .pictured()
        .iter()
        .chain(call.displayed())
        .fold(answer, |answer, picture| {
            answer.image(Image::inline(picture.media(), Arc::clone(picture.data())))
        });
    call.loaded()
        .iter()
        .fold(pictured, |answer, tool| answer.reference(tool.as_ref()))
}
