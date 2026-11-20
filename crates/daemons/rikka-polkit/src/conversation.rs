use std::sync::Arc;
use std::sync::mpsc::Receiver;

use crate::ask::Notice;
use crate::error::Error;
use crate::helper::Helper;
use crate::pending::{Outcome, Pending};
use crate::protocol::Line;
use crate::reply::Reply;
use crate::secret::Secret;

const ATTEMPTS: u32 = 3;

pub(crate) fn start(pending: Arc<Pending>, id: u64, cookie: String, replies: Receiver<Reply>) {
    let failed = Arc::clone(&pending);
    let started = std::thread::Builder::new()
        .name("rikka-polkit-auth".to_owned())
        .spawn(move || {
            let outcome = run(&pending, id, &cookie, &replies);
            pending.finish(id, outcome);
        });
    if let Err(error) = started {
        tracing::error!("no identity can authenticate: {error}");
        failed.finish(id, Outcome::Denied);
    }
}

fn run(pending: &Arc<Pending>, id: u64, cookie: &str, replies: &Receiver<Reply>) -> Outcome {
    loop {
        let Some(user) = pending.user(id) else {
            tracing::warn!("none of the offered identities can authenticate");
            return Outcome::Denied;
        };
        let helper = match Helper::start(&user.name, cookie) {
            Ok(helper) => helper,
            Err(error) => {
                tracing::error!("{error}");
                return Outcome::Denied;
            }
        };
        pending.running(id, helper.stop());

        match converse(pending, id, helper, replies) {
            Step::Authorized => return Outcome::Authorized,
            Step::Dismissed => return Outcome::Cancelled,
            Step::Chose => {}
            Step::Refused => {
                if pending.failed(id) >= ATTEMPTS {
                    return Outcome::Denied;
                }
            }
            Step::Broken(error) => {
                if pending.cancelled(id) {
                    return Outcome::Cancelled;
                }
                tracing::warn!("authentication aborted: {error}");
                return Outcome::Denied;
            }
        }
    }
}

enum Step {
    Authorized,
    Refused,
    Dismissed,
    Chose,
    Broken(Error),
}

fn converse(
    pending: &Arc<Pending>,
    id: u64,
    mut helper: Helper,
    replies: &Receiver<Reply>,
) -> Step {
    loop {
        let line = match helper.read() {
            Ok(line) => line,
            Err(error) => return Step::Broken(error),
        };
        match line {
            Line::Prompt { text, echo } => {
                let serial = pending.asking(id, text, echo);
                let secret = match wait(replies, serial) {
                    Ok(secret) => secret,
                    Err(step) => return step,
                };
                pending.working(id);
                if let Err(error) = helper.respond(&secret) {
                    return Step::Broken(error);
                }
            }
            Line::Info(text) => pending.notify(id, Notice::info(text)),
            Line::Error(text) => pending.notify(id, Notice::error(text)),
            Line::Success => return Step::Authorized,
            Line::Failure => return Step::Refused,
        }
    }
}

fn wait(replies: &Receiver<Reply>, serial: u64) -> std::result::Result<Secret, Step> {
    while let Ok(reply) = replies.recv() {
        match reply {
            Reply::Response {
                serial: answering,
                secret,
            } if answering == serial => return Ok(secret),
            Reply::Response { .. } => {}
            Reply::Choose(_) => return Err(Step::Chose),
            Reply::Dismiss => return Err(Step::Dismissed),
        }
    }
    Err(Step::Dismissed)
}
