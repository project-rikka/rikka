use std::io::{self, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cli_command::RemoteCommand;
use rikka_session::remote::{self, Encoding, RemoteClient, RemoteStatus, Viewer};
use serde::Serialize;

use crate::error::Result;
use crate::printer::{Printer, Report};
use crate::session::{CapabilitySocket, ReplyExt};
use crate::table::Table;

const MEGABIT: f64 = 1_000_000.0;
const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;

pub(super) fn run(command: RemoteCommand, printer: Printer) -> Result<()> {
    let socket = CapabilitySocket::connect(remote::CAPABILITY)?;
    let remote = RemoteClient::new(socket.client());
    match command {
        RemoteCommand::Status => printer.report(&StatusReport(remote.status().decode()?)),
        RemoteCommand::Disconnect => remote.disconnect().finish(),
    }
}

#[derive(Serialize)]
#[serde(transparent)]
struct StatusReport(RemoteStatus);

impl Report for StatusReport {
    fn write_text(&self, out: &mut dyn Write) -> io::Result<()> {
        let status = &self.0;
        let mut table = Table::without_header("");
        match &status.serving {
            Some(serving) => {
                for (index, address) in serving.addresses.iter().enumerate() {
                    let label = if index == 0 { "serving" } else { "" };
                    table.row([label.to_owned(), address.address.clone()]);
                }
                table.row(["fingerprint".to_owned(), serving.fingerprint.clone()]);
            }
            None => table.row(["serving".to_owned(), "off".to_owned()]),
        }
        match &status.viewer {
            Some(viewer) => {
                table.row(["viewer".to_owned(), who(viewer)]);
                table.row(["connected".to_owned(), elapsed(viewer.since)]);
                if let Some(encoding) = &viewer.encoding {
                    table.row(["picture".to_owned(), picture(encoding)]);
                }
            }
            None => table.row(["viewer".to_owned(), "none".to_owned()]),
        }
        if let Some(problem) = &status.problem {
            table.row(["problem".to_owned(), problem.clone()]);
        }
        table.write(out)
    }
}

fn who(viewer: &Viewer) -> String {
    format!("{} ({})", viewer.device, viewer.address)
}

fn elapsed(since: i64) -> String {
    let connected = UNIX_EPOCH + Duration::from_secs(u64::try_from(since).unwrap_or_default());
    let seconds = SystemTime::now()
        .duration_since(connected)
        .unwrap_or_default()
        .as_secs();
    match seconds {
        0..MINUTE => format!("{seconds}s ago"),
        MINUTE..HOUR => format!("{}m ago", seconds / MINUTE),
        _ => format!("{}h {:02}m ago", seconds / HOUR, seconds % HOUR / MINUTE),
    }
}

fn picture(encoding: &Encoding) -> String {
    format!(
        "{} {}x{} {:.1} Mbit/s via {}",
        encoding.codec,
        encoding.width,
        encoding.height,
        encoding.bitrate as f64 / MEGABIT,
        encoding.encoder
    )
}
