use futures_channel::mpsc::UnboundedSender;
use nixctl_task::{Ending, Error, Report, Step};
use rikka_nix_journal::{Transcript, Written};
use rikka_nix_log::{Decoder, Realisation};
use rikka_nix_protocol::{Phase, RunState, Status};

#[derive(Debug)]
pub(crate) enum Note {
    Line(Written),
    Status(Status),
}

pub(crate) struct Recorder {
    decoder: Decoder,
    transcript: Transcript,
    step: Step,
    status: Status,
    notes: UnboundedSender<Note>,
    unwritten: bool,
}

impl Recorder {
    pub(crate) fn new(transcript: Transcript, notes: UnboundedSender<Note>) -> Self {
        Self {
            decoder: Decoder::new(),
            transcript,
            step: Step::Preparing,
            status: Status::default(),
            notes,
            unwritten: false,
        }
    }

    pub(crate) fn conclude(self, ending: Ending) -> RunState {
        match ending {
            Ending::Done { generation } => RunState::Done { generation },
            Ending::Failed { error, changes } => RunState::Failed {
                reason: self.reason(&error),
                changes,
            },
        }
    }

    fn reason(&self, error: &Error) -> String {
        match error {
            Error::ExitStatus { .. } => {
                if let Some(error) = self.decoder.error() {
                    return error.to_owned();
                }
                let output: Vec<&str> = self
                    .decoder
                    .output_tail()
                    .filter(|line| !line.trim().is_empty())
                    .collect();
                match output.is_empty() {
                    true => error.to_string(),
                    false => output.join("\n"),
                }
            }
            Error::Restore { error, source } => {
                format!(
                    "{}; failed to restore the previous files: {source}",
                    self.reason(error)
                )
            }
            other => other.to_string(),
        }
    }

    fn write(&mut self, text: String) {
        match self.transcript.append(&text) {
            Ok(Some(written)) => self.send(Note::Line(written)),
            Ok(None) => {}
            Err(error) if !self.unwritten => {
                self.unwritten = true;
                eprintln!("{error}");
            }
            Err(_) => {}
        }
    }

    fn refresh(&mut self) {
        let status = Status {
            phase: self.phase(),
            tally: self.decoder.tally(),
        };
        if status != self.status {
            self.status = status;
            self.send(Note::Status(status));
        }
    }

    fn phase(&self) -> Phase {
        match self.step {
            Step::Preparing => Phase::Preparing,
            Step::Locking => Phase::Locking,
            Step::Activating => Phase::Activating,
            Step::Rebuilding => match self.decoder.realisation() {
                Realisation::Pending => Phase::Evaluating,
                Realisation::Running => Phase::Building,
                Realisation::Finished => Phase::Activating,
            },
        }
    }

    fn send(&self, note: Note) {
        if self.notes.unbounded_send(note).is_err() {
            eprintln!("progress receiver closed");
        }
    }
}

impl Report for Recorder {
    fn step(&mut self, step: Step) {
        self.step = step;
        self.refresh();
    }

    fn output(&mut self, line: &str) {
        for text in self.decoder.decode(line) {
            self.write(text);
        }
        self.refresh();
    }

    fn is_activating(&self) -> bool {
        self.phase() == Phase::Activating
    }
}
