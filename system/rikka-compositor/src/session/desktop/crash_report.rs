use std::io;
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::time::Duration;

use rikka_compositor_restarts::Restart;

use crate::crash::{Report, Tail};
use crate::programs::SHELL;

#[derive(Debug)]
pub(super) struct Crash<'a> {
    pub(super) program: &'a Path,
    pub(super) pid: u32,
    pub(super) status: &'a io::Result<ExitStatus>,
    pub(super) uptime: Duration,
    pub(super) restart: Restart,
    pub(super) log: Option<&'a Path>,
    pub(super) output: &'a Tail,
}

impl Crash<'_> {
    pub(super) fn write(&self) -> io::Result<PathBuf> {
        let mut report = Report::new(SHELL, self.pid)
            .field("program", self.program.display())
            .field("uptime", format!("{:.1}s", self.uptime.as_secs_f64()))
            .field("status", exit_status(self.status));
        if self
            .status
            .as_ref()
            .is_ok_and(|status| status.core_dumped())
        {
            report = report.field("core", format!("coredumpctl debug {}", self.pid));
        }
        report = report.field("restart", self.restart);
        if let Some(log) = self.log {
            report = report.field("log", log.display());
        }
        report.section("output", self.output.text()).write()
    }
}

pub(super) fn exit_status(status: &io::Result<ExitStatus>) -> String {
    match status {
        Ok(status) => status.to_string(),
        Err(error) => format!("unknown: {error}"),
    }
}
