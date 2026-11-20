mod conversation;
mod terminal;

use std::io;
use std::process::ExitStatus;

use crate::error::PasswordError;
use crate::passwd::conversation::{Conversation, Ending, Transcript};
use crate::passwd::terminal::Terminal;
use crate::secret::Secret;

pub(crate) const PASSWD: &str = "/run/wrappers/bin/passwd";

const OWN_LINES: &str = "passwd:";

pub fn change_password(current: &Secret, new: &Secret) -> Result<(), PasswordError> {
    let (terminal, mut child) = Terminal::spawn(PASSWD).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => PasswordError::Unavailable,
        _ => PasswordError::Io(error),
    })?;
    let transcript = Conversation::new([current, new, new]).run(&terminal);
    if !matches!(
        transcript,
        Ok(Transcript {
            ending: Ending::Closed,
            ..
        })
    ) && let Err(error) = child.kill()
    {
        tracing::debug!(%error, "failed to stop passwd");
    }
    let status = child.wait()?;
    settle(transcript?, status)
}

fn settle(transcript: Transcript, status: ExitStatus) -> Result<(), PasswordError> {
    let reason = reason(&transcript);
    match transcript.ending {
        Ending::Unexpected(prompt) => Err(PasswordError::Unexpected(prompt)),
        Ending::AskedAgain => Err(PasswordError::Rejected { reason }),
        Ending::Closed if status.success() => Ok(()),
        Ending::Closed => match (transcript.answered, reason) {
            (0, _) => Err(PasswordError::Failed(
                heard(&transcript).unwrap_or_else(|| status.to_string()),
            )),
            (1, None) => Err(PasswordError::Incorrect),
            (_, reason) => Err(PasswordError::Rejected { reason }),
        },
    }
}

fn reason(transcript: &Transcript) -> Option<String> {
    joined(
        transcript
            .lines
            .iter()
            .filter(|line| line.answered > 0)
            .map(|line| line.text.as_str())
            .filter(|text| !text.starts_with(OWN_LINES)),
    )
}

fn heard(transcript: &Transcript) -> Option<String> {
    joined(transcript.lines.iter().map(|line| line.text.as_str()))
}

fn joined<'a>(lines: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut distinct: Vec<&str> = Vec::new();
    for line in lines.filter(|line| !line.is_empty()) {
        if !distinct.contains(&line) {
            distinct.push(line);
        }
    }
    (!distinct.is_empty()).then(|| distinct.join("\n"))
}
