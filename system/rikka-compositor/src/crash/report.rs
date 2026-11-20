use std::fmt::{self, Display};
use std::io;
use std::path::PathBuf;

use jiff::Timestamp;

use crate::retention::keep_newest;

const DIRECTORY: &str = "crashes";
const KEPT: usize = 20;
const STAMP: &str = "%Y%m%dT%H%M%SZ";

#[derive(Debug)]
#[must_use]
pub(crate) struct Report {
    program: &'static str,
    pid: u32,
    at: Timestamp,
    fields: Vec<(&'static str, String)>,
    sections: Vec<(&'static str, String)>,
}

impl Report {
    pub(crate) fn new(program: &'static str, pid: u32) -> Report {
        Report {
            program,
            pid,
            at: Timestamp::now(),
            fields: Vec::new(),
            sections: Vec::new(),
        }
    }

    pub(crate) fn field(mut self, name: &'static str, value: impl Display) -> Report {
        self.fields.push((name, value.to_string()));
        self
    }

    pub(crate) fn section(mut self, name: &'static str, text: impl Display) -> Report {
        self.sections.push((name, text.to_string()));
        self
    }

    pub(crate) fn write(&self) -> io::Result<PathBuf> {
        let directory = crate::logging::state_directory()?.join(DIRECTORY);
        let name = format!("{}-{}-{}", self.program, self.at.strftime(STAMP), self.pid);
        let path = directory.join(format!("{name}.log"));
        rikka_fs::replace(&path, self.to_string())?;
        keep_newest(&directory, KEPT);
        Ok(path)
    }
}

impl Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} crashed", self.program)?;
        writeln!(f)?;
        writeln!(f, "time: {}", self.at)?;
        writeln!(f, "pid: {}", self.pid)?;
        for (name, value) in &self.fields {
            writeln!(f, "{name}: {value}")?;
        }
        for (name, text) in &self.sections {
            writeln!(f)?;
            writeln!(f, "--- {name} ---")?;
            write!(f, "{text}")?;
            if !text.ends_with('\n') {
                writeln!(f)?;
            }
        }
        Ok(())
    }
}
