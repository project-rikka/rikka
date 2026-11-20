use std::io::{self, Write};

use cli_command::WorkspaceCommand;
use rikka_session::workspaces::{self, State, WorkspacesClient};
use serde::Serialize;

use crate::error::Result;
use crate::printer::{Printer, Report};
use crate::session::{CapabilitySocket, ReplyExt};
use crate::table::Table;

pub(super) fn run(command: WorkspaceCommand, printer: Printer) -> Result<()> {
    let socket = CapabilitySocket::connect(workspaces::CAPABILITY)?;
    let workspaces = WorkspacesClient::new(socket.client());
    match command {
        WorkspaceCommand::List => printer.report(&Workspaces(workspaces.state().decode()?)),
        WorkspaceCommand::Show { number, output } => workspaces.show(output, number).finish(),
    }
}

#[derive(Serialize)]
#[serde(transparent)]
struct Workspaces(State);

impl Report for Workspaces {
    fn write_text(&self, out: &mut dyn Write) -> io::Result<()> {
        let mut table = Table::new(["OUTPUT", "SHOWING", "HOLDING WINDOWS"]);
        for display in &self.0.displays {
            let holding: Vec<String> = display
                .holding
                .iter()
                .enumerate()
                .filter(|(_, holds)| **holds)
                .map(|(index, _)| (index + 1).to_string())
                .collect();
            table.row([
                display.output.clone(),
                display.showing.to_string(),
                holding.join(" "),
            ]);
        }
        table.write(out)
    }
}
