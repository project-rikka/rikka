use std::fs::File;
use std::os::fd::AsFd;
use std::sync::Arc;

use serde::Serialize;

use crate::error::Result;
use crate::outcome::Outcome;
use crate::progname::progname;

#[derive(Clone, Debug)]
pub struct Reply {
    channel: Arc<File>,
}

impl Reply {
    pub fn claim() -> Result<Reply> {
        let channel = std::io::stdout().as_fd().try_clone_to_owned()?;
        rustix::stdio::dup2_stdout(std::io::stderr()).map_err(std::io::Error::from)?;
        Ok(Reply {
            channel: Arc::new(File::from(channel)),
        })
    }

    pub fn send<A: Serialize>(&self, outcome: &Outcome<A>) -> Result<()> {
        outcome.write(self.channel.as_ref())
    }

    pub fn answer<A: Serialize>(&self, outcome: &Outcome<A>) -> ! {
        if let Err(error) = self.send(outcome) {
            eprintln!("{}: failed to send answer: {error}", progname());
            std::process::exit(1);
        }
        std::process::exit(0)
    }
}
