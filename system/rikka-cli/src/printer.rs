use std::io::{self, IsTerminal, Write};

use serde::Serialize;

use crate::error::Result;

pub(crate) trait Report: Serialize {
    fn write_text(&self, out: &mut dyn Write) -> io::Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Format {
    Text,
    Json,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Printer {
    format: Format,
}

impl Printer {
    pub(crate) fn new(format: Format) -> Printer {
        Printer { format }
    }

    pub(crate) fn report(&self, report: &impl Report) -> Result<()> {
        self.emit(report, io::stdout().is_terminal())
    }

    pub(crate) fn stream(&self, report: &impl Report) -> Result<()> {
        self.emit(report, false)
    }

    fn emit(&self, report: &impl Report, pretty: bool) -> Result<()> {
        let mut out = io::stdout().lock();
        match self.format {
            Format::Text => report.write_text(&mut out)?,
            Format::Json => {
                match pretty {
                    true => serde_json::to_writer_pretty(&mut out, report)?,
                    false => serde_json::to_writer(&mut out, report)?,
                }
                out.write_all(b"\n")?;
            }
        }
        out.flush()?;
        Ok(())
    }
}
