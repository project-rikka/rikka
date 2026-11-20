use std::io::{self, Write};

use cli_command::WindowCommand;
use rikka_session::toplevels::{self, State, Toplevel, ToplevelsClient};
use serde::Serialize;

use crate::error::Result;
use crate::printer::{Printer, Report};
use crate::session::{CapabilitySocket, ReplyExt};
use crate::table::Table;

pub(super) fn run(command: WindowCommand, printer: Printer) -> Result<()> {
    let socket = CapabilitySocket::connect(toplevels::CAPABILITY)?;
    let windows = ToplevelsClient::new(socket.client());
    match command {
        WindowCommand::List => {
            let state: State = windows.state().decode()?;
            printer.report(&Windows(state.toplevels))
        }
        WindowCommand::Focus { id } => windows.focus(id).finish(),
        WindowCommand::Close { id } => windows.close(id).finish(),
        WindowCommand::Minimize { id } => windows.minimize(id).finish(),
        WindowCommand::Move { id, workspace } => windows.move_to_workspace(id, workspace).finish(),
    }
}

#[derive(Serialize)]
#[serde(transparent)]
struct Windows(Vec<Toplevel>);

impl Report for Windows {
    fn write_text(&self, out: &mut dyn Write) -> io::Result<()> {
        let mut table = Table::new(["ID", "APP", "OUTPUT", "WORKSPACE", "STATE", "TITLE"]);
        for window in &self.0 {
            table.row([
                window.id.clone(),
                window.app_id.clone(),
                window.output.clone(),
                window.workspace.to_string(),
                state_of(window).to_owned(),
                window.title.clone(),
            ]);
        }
        table.write(out)
    }
}

fn state_of(window: &Toplevel) -> &'static str {
    if window.focused {
        "focused"
    } else if window.minimized {
        "minimized"
    } else if window.on_screen {
        "shown"
    } else {
        "hidden"
    }
}
