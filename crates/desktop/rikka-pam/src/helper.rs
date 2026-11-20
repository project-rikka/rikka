use std::fs::File;
use std::io;
use std::os::fd::AsFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use crate::error::{Error, Result};
use crate::ffi::c_string;
use crate::step::Step;
use crate::transaction::authenticate;
use crate::wire;

const ANSWERS_THREAD: &str = "rikka-pam-answers";

pub fn serve(service: &str, user: &str) -> Result<()> {
    let service = c_string(service)?;
    let user = c_string(user)?;
    let (answers, answer_receiver) = mpsc::channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancel_flag = Arc::clone(&cancelled);
    std::thread::Builder::new()
        .name(ANSWERS_THREAD.to_owned())
        .spawn(move || {
            if let Ok(unbuffered) = io::stdin().as_fd().try_clone_to_owned() {
                let mut input = File::from(unbuffered);
                while let Ok(Some(answer)) = wire::receive_reply(&mut input) {
                    if answers.send(answer).is_err() {
                        return;
                    }
                }
            }
            cancel_flag.store(true, Ordering::Release);
        })
        .map_err(Error::Spawn)?;
    let mut output = io::stdout().lock();
    let mut report = |step: Step| {
        let _ = wire::send_step(&mut output, &step);
    };
    let outcome = authenticate(&service, &user, &mut report, &answer_receiver, &cancelled);
    report(Step::Finished(outcome));
    Ok(())
}
