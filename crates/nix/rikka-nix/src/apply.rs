use std::sync::mpsc::{Receiver, channel};

use rikka_bus::Heard;
use rikka_nix_protocol::{ChangeState, Error as Malformed, Request, RunId, RunState};
use zbus::Message;
use zbus::blocking::Proxy;

use crate::bus::{self, Follower};
use crate::error::{Error, Result};
use crate::event::Event;
use crate::outcome::{Outcome, Progress, Refusal};

const EXITED: &str = "rikka-nixctl exited before the run finished";

const DISCONNECTED: &str = "the system bus connection closed before the run finished";

pub(crate) fn run(request: &Request, report: impl Fn(Progress) + Send + 'static) -> Result<()> {
    let connection = bus::connect()?;
    let (heard, messages) = channel();
    let signals = bus::signals(&[bus::FINISHED])
        .hear(
            &connection,
            |message| Some(message.clone()),
            move |message| heard.send(message).is_ok(),
        )
        .map_err(|error| bus::unsubscribed(&error))?;
    let proxy = bus::proxy(&connection)?;
    let request = request.clone();
    std::thread::Builder::new()
        .name("rikka-nix-apply".to_owned())
        .spawn(move || {
            follow(&proxy, &request, &messages, &report);
            drop(signals);
        })
        .map_err(|error| Error::Unavailable(error.to_string()))?;
    Ok(())
}

fn follow(
    proxy: &Proxy<'_>,
    request: &Request,
    messages: &Receiver<Heard<Message>>,
    report: &impl Fn(Progress),
) {
    let (run, owner) = match call(proxy, request) {
        Ok(accepted) => accepted,
        Err(refusal) => return report(Progress::Finished(Outcome::Refused(refusal))),
    };
    report(Progress::Started);

    let mut follower = Follower::new(Some(owner));
    while let Ok(Heard::Signal(message)) = messages.recv() {
        match follower.read(&message) {
            Some(Event::Finished(finished)) if finished.id == run => {
                return report(Progress::Finished(outcome(finished.state)));
            }
            Some(Event::Exited) => return report(Progress::Finished(lost(EXITED))),
            _ => {}
        }
    }
    report(Progress::Finished(lost(DISCONNECTED)));
}

fn call(proxy: &Proxy<'_>, request: &Request) -> Result<(RunId, String), Refusal> {
    let reply = match request {
        Request::Apply { files, action } => {
            let files: Vec<(&str, &str, &str)> = files
                .iter()
                .map(|file| {
                    (
                        file.module.name(),
                        file.base.as_str(),
                        file.contents.as_str(),
                    )
                })
                .collect();
            proxy.call_method("Apply", &(files, action.name()))
        }
        Request::Rollback { generation } => proxy.call_method("Rollback", &(*generation,)),
        Request::Update { action } => proxy.call_method("Update", &(action.name(),)),
    }
    .map_err(|error| bus::refusal(&error))?;
    let malformed =
        |reason: &str| Refusal::Unavailable(Malformed::Malformed(reason.to_owned()).to_string());
    let owner = reply
        .header()
        .sender()
        .map(ToString::to_string)
        .ok_or_else(|| malformed("method reply has no sender"))?;
    let run: u64 = reply
        .body()
        .deserialize()
        .map_err(|_| malformed("method reply carries no run"))?;
    Ok((RunId::new(run), owner))
}

fn outcome(state: RunState) -> Outcome {
    match state {
        RunState::Done { generation } => Outcome::Done { generation },
        RunState::Failed { reason, changes } => Outcome::Failed { reason, changes },
        RunState::Running | RunState::Interrupted => lost(EXITED),
    }
}

fn lost(reason: &str) -> Outcome {
    Outcome::Failed {
        reason: reason.to_owned(),
        changes: ChangeState::Modified,
    }
}
