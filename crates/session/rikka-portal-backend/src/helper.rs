use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

use futures_channel::oneshot;
use futures_util::future::{self, Either};
use serde::Serialize;
use serde::de::DeserializeOwned;
use zbus::object_server::ObjectServer;
use zbus::zvariant::OwnedObjectPath;

use rikka_portal::Outcome;

use crate::cancel::Cancel;
use crate::request::Request;
use crate::response::{CANCELLED, ENDED, SUCCESS};

const READER: &str = "rikka-portal-helper";

#[derive(Debug)]
pub enum HelperOutcome<A> {
    Answered(Box<A>),
    Cancelled,
    Ended,
}

impl<A> HelperOutcome<A> {
    #[must_use]
    pub fn code(&self) -> u32 {
        match self {
            HelperOutcome::Answered(_) => SUCCESS,
            HelperOutcome::Cancelled => CANCELLED,
            HelperOutcome::Ended => ENDED,
        }
    }
}

#[derive(Debug)]
pub struct Helper {
    program: PathBuf,
    argument: Option<OsString>,
}

impl Helper {
    #[must_use]
    pub fn program(variable: &str, name: &str) -> Helper {
        if let Some(named) = std::env::var_os(variable) {
            return Helper::new(named);
        }
        if let Ok(here) = std::env::current_exe()
            && let Some(beside) = here.parent().map(|directory| directory.join(name))
            && beside.is_file()
        {
            return Helper::new(beside);
        }
        Helper::new(name)
    }

    #[must_use]
    pub fn current_exe(argument: impl Into<OsString>) -> Helper {
        Helper {
            program: PathBuf::from("/proc/self/exe"),
            argument: Some(argument.into()),
        }
    }

    pub async fn ask<Q, A>(
        &self,
        server: &ObjectServer,
        handle: &OwnedObjectPath,
        ask: &Q,
    ) -> HelperOutcome<A>
    where
        Q: Serialize,
        A: DeserializeOwned,
    {
        let mut request = Vec::new();
        if let Err(error) = rikka_portal::write(ask, &mut request) {
            tracing::warn!(%error, "cannot say what to ask for");
            return HelperOutcome::Ended;
        }

        let (cancel, stopped) = Cancel::new();
        let cancel = Arc::new(cancel);
        if let Err(error) = server.at(handle, Request::new(Arc::clone(&cancel))).await {
            tracing::warn!(%error, %handle, "cannot answer on the request handle");
        }

        let reply = self.run(request, &cancel, stopped).await;

        if let Err(error) = server.remove::<Request, _>(handle).await {
            tracing::warn!(%error, %handle, "cannot take the request handle back");
        }

        let Some(reply) = reply else {
            return HelperOutcome::Ended;
        };
        match Outcome::read(reply.as_slice()) {
            Ok(Outcome::Chosen(answer)) => HelperOutcome::Answered(Box::new(answer)),
            Ok(Outcome::Cancelled) => HelperOutcome::Cancelled,
            Err(error) => {
                tracing::warn!(%error, "the helper said nothing usable");
                HelperOutcome::Ended
            }
        }
    }

    fn new(program: impl Into<PathBuf>) -> Helper {
        Helper {
            program: program.into(),
            argument: None,
        }
    }

    async fn run(
        &self,
        request: Vec<u8>,
        cancel: &Cancel,
        stopped: oneshot::Receiver<()>,
    ) -> Option<Vec<u8>> {
        let mut child = match Command::new(&self.program)
            .args(&self.argument)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                tracing::warn!(%error, program = %self.program.display(), "cannot run the helper");
                return None;
            }
        };
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        if !cancel.attach(child) {
            cancel.reap();
            return None;
        }

        let (sender, answer) = oneshot::channel();
        let reader = std::thread::Builder::new()
            .name(READER.to_owned())
            .spawn(move || {
                if let Some(mut stdin) = stdin
                    && let Err(error) = stdin.write_all(&request)
                {
                    tracing::warn!(%error, "failed to write helper request");
                }
                let mut reply = Vec::new();
                if let Some(mut stdout) = stdout
                    && let Err(error) = stdout.read_to_end(&mut reply)
                {
                    tracing::warn!(%error, "failed to read helper reply");
                }
                sender.send(reply).ok();
            });
        if let Err(error) = reader {
            tracing::warn!(%error, "cannot wait on a helper");
            cancel.stop();
            cancel.reap();
            return None;
        }

        let reply = match future::select(answer, stopped).await {
            Either::Left((reply, _)) => Some(reply.unwrap_or_default()),
            Either::Right(_) => None,
        };
        cancel.reap();
        reply
    }
}
