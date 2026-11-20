use std::io::{self, Write};

use cli_command::DesktopCommand;
use rikka_session::desktop::{self, DesktopClient, Status};
use serde::Serialize;

use crate::error::Result;
use crate::printer::{Printer, Report};
use crate::session::{CapabilitySocket, ReplyExt};

pub(super) fn run(command: DesktopCommand, printer: Printer) -> Result<()> {
    let socket = CapabilitySocket::connect(desktop::CAPABILITY)?;
    let desktop = DesktopClient::new(socket.client());
    match command {
        DesktopCommand::Status => printer.report(&StatusReport(desktop.status().decode()?)),
        DesktopCommand::Start => desktop.start().finish(),
        DesktopCommand::Stop => desktop.stop().finish(),
        DesktopCommand::Restart => desktop.restart().finish(),
    }
}

#[derive(Serialize)]
#[serde(transparent)]
struct StatusReport(Status);

impl Report for StatusReport {
    fn write_text(&self, out: &mut dyn Write) -> io::Result<()> {
        let program = self.0.program.display();
        match self.0.pid {
            Some(pid) => writeln!(out, "running  pid {pid}  {program}"),
            None => writeln!(out, "stopped  {program}"),
        }
    }
}
