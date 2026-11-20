use std::io::{self, Read, Write};

use zeroize::Zeroizing;

use crate::passwd::terminal::Terminal;
use crate::secret::Secret;

const CHUNK: usize = 4096;

#[derive(Debug)]
pub(crate) enum Ending {
    Closed,
    AskedAgain,
    Unexpected(String),
}

#[derive(Debug)]
pub(crate) struct Line {
    pub(crate) answered: usize,
    pub(crate) text: String,
}

#[derive(Debug)]
pub(crate) struct Transcript {
    pub(crate) answered: usize,
    pub(crate) lines: Vec<Line>,
    pub(crate) ending: Ending,
}

pub(crate) struct Conversation<'a> {
    answers: [&'a Secret; 3],
    answered: usize,
    armed: bool,
    partial: Vec<u8>,
    lines: Vec<Line>,
}

impl<'a> Conversation<'a> {
    pub(crate) fn new(answers: [&'a Secret; 3]) -> Conversation<'a> {
        Conversation {
            answers,
            answered: 0,
            armed: true,
            partial: Vec::new(),
            lines: Vec::new(),
        }
    }

    pub(crate) fn run(mut self, terminal: &Terminal) -> io::Result<Transcript> {
        let mut chunk = [0; CHUNK];
        loop {
            let read = match terminal.output().read(&mut chunk) {
                Ok(0) => return Ok(self.end(Ending::Closed)),
                Ok(read) => read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.raw_os_error() == Some(libc::EIO) => {
                    return Ok(self.end(Ending::Closed));
                }
                Err(error) => return Err(error),
            };
            self.partial.extend_from_slice(&chunk[..read]);
            self.take_lines();
            if !self.armed || String::from_utf8_lossy(&self.partial).trim().is_empty() {
                continue;
            }
            let prompt = String::from_utf8_lossy(&std::mem::take(&mut self.partial)).into_owned();
            if !terminal.hides_input()? {
                return Ok(self.end(Ending::Unexpected(prompt.trim().to_owned())));
            }
            let Some(answer) = self.answers.get(self.answered) else {
                return Ok(self.end(Ending::AskedAgain));
            };
            let mut reply = Zeroizing::new(answer.reveal().as_bytes().to_vec());
            reply.push(b'\n');
            terminal.output().write_all(&reply)?;
            self.answered += 1;
            self.armed = false;
        }
    }

    fn take_lines(&mut self) {
        while let Some(end) = self.partial.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.partial.drain(..=end).collect();
            self.lines.push(Line {
                answered: self.answered,
                text: String::from_utf8_lossy(&line).trim().to_owned(),
            });
            self.armed = true;
        }
    }

    fn end(self, ending: Ending) -> Transcript {
        Transcript {
            answered: self.answered,
            lines: self.lines,
            ending,
        }
    }
}
