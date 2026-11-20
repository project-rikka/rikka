use std::io::BufReader;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::environment;
use crate::error::{Error, Result};
use crate::ffi::c_string;
use crate::secret::Secret;
use crate::step::{Outcome, Step};
use crate::wire;

const HELPER: &str = "/run/current-system/sw/bin/rikka-pam-helper";

const THREAD: &str = "rikka-pam";

#[derive(Debug)]
pub struct Authentication {
    answers: ChildStdin,
}

impl Authentication {
    pub fn start(
        service: &str,
        user: &str,
        report: impl FnMut(Step) + Send + 'static,
    ) -> Result<Authentication> {
        c_string(service)?;
        c_string(user)?;
        let mut command = Command::new(HELPER);
        command
            .args([service, user])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        environment::pass_locale(&mut command);
        let mut child = command.spawn().map_err(|source| Error::Helper {
            path: HELPER,
            source,
        })?;
        let (Some(answers), Some(steps)) = (child.stdin.take(), child.stdout.take()) else {
            unreachable!("helper stdin and stdout are piped");
        };
        std::thread::Builder::new()
            .name(THREAD.to_owned())
            .spawn(move || listen(child, steps, report))
            .map_err(Error::Spawn)?;
        Ok(Authentication { answers })
    }

    pub fn answer(&self, serial: u64, secret: Secret) {
        if let Err(error) = wire::send_reply(&mut &self.answers, serial, &secret) {
            tracing::debug!(%error, "failed to send reply to PAM helper");
        }
    }
}

fn listen(mut child: Child, steps: ChildStdout, mut report: impl FnMut(Step)) {
    let mut steps = BufReader::new(steps);
    let mut finished = false;
    loop {
        match wire::receive_step(&mut steps) {
            Ok(Some(step)) => {
                finished |= matches!(step, Step::Finished(_));
                report(step);
            }
            Ok(None) => break,
            Err(error) => {
                let _ = child.kill();
                if !finished {
                    finished = true;
                    report(Step::Finished(Outcome::Error(error.to_string())));
                }
                break;
            }
        }
    }
    let ended = child.wait();
    if !finished {
        let reason = match ended {
            Ok(status) => format!("{HELPER} exited without a result ({status})"),
            Err(error) => format!("{HELPER} exited without a result: {error}"),
        };
        report(Step::Finished(Outcome::Error(reason)));
    }
}
