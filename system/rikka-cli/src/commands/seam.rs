use std::io::{self, Write};
use std::sync::mpsc;
use std::thread;

use cli_command::SeamCommand;
use rikka_seam::{Delivery, Manifest, Payload, Request, SeamClient};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::printer::{Printer, Report};
use crate::session::{CapabilitySocket, Session};
use crate::table::Table;

pub(super) fn run(command: SeamCommand, printer: Printer) -> Result<()> {
    match command {
        SeamCommand::Capabilities => printer.report(&Capabilities(Session::connect()?.manifests())),
        SeamCommand::Call {
            capability,
            method,
            params,
        } => {
            let payload = match params {
                Some(text) => Payload::from_json(&text).map_err(Error::InvalidJson)?,
                None => Payload::none(),
            };
            let socket = CapabilitySocket::connect(&capability)?;
            let answer = socket
                .client()
                .call(Request::new(capability, method).params(payload))
                .blocking_wait()?;
            printer.report(&Answer(answer))
        }
        SeamCommand::Watch {
            capability: Some(capability),
        } => {
            let socket = CapabilitySocket::connect(&capability)?;
            watch_events(vec![socket.client()], Some(capability), printer)
        }
        SeamCommand::Watch { capability: None } => {
            let session = Session::connect()?;
            watch_events(session.clients().collect(), None, printer)
        }
    }
}

#[derive(Serialize)]
#[serde(transparent)]
struct Capabilities(Vec<Manifest>);

impl Report for Capabilities {
    fn write_text(&self, out: &mut dyn Write) -> io::Result<()> {
        for manifest in &self.0 {
            match manifest.summary_text() {
                Some(summary) => writeln!(
                    out,
                    "{} {}  {summary}",
                    manifest.capability(),
                    manifest.version()
                )?,
                None => writeln!(out, "{} {}", manifest.capability(), manifest.version())?,
            }
            let mut members = Table::without_header("  ");
            for method in manifest.methods() {
                members.row(member("method", method.name(), method.summary_text()));
            }
            for event in manifest.events() {
                members.row(member("event", event.name(), event.summary_text()));
            }
            members.write(out)?;
        }
        Ok(())
    }
}

fn member(kind: &str, name: &str, summary: Option<&str>) -> [String; 3] {
    [
        kind.to_owned(),
        name.to_owned(),
        summary.unwrap_or_default().to_owned(),
    ]
}

#[derive(Serialize)]
#[serde(transparent)]
struct Answer(Payload);

impl Report for Answer {
    fn write_text(&self, out: &mut dyn Write) -> io::Result<()> {
        if self.0.is_none() {
            return Ok(());
        }
        serde_json::to_writer_pretty(&mut *out, &self.0)?;
        writeln!(out)
    }
}

fn watch_events(
    watched: Vec<SeamClient>,
    capability: Option<String>,
    printer: Printer,
) -> Result<()> {
    let (sender, receiver) = mpsc::channel();
    for seam in watched {
        let sender = sender.clone();
        let capability = capability.clone();
        thread::Builder::new()
            .name("rikka-cli-watch".to_owned())
            .spawn(move || {
                let mut events = seam.poll_events(capability);
                while let Some(event) = events.blocking_next() {
                    if sender.send(Announcement::from(event)).is_err() {
                        return;
                    }
                }
            })
            .map_err(Error::WatchThread)?;
    }
    drop(sender);
    for announcement in receiver {
        printer.stream(&announcement)?;
    }
    Err(Error::SessionEnded)
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Announcement {
    Event {
        capability: String,
        event: String,
        payload: Payload,
    },
    #[serde(rename = "missed")]
    Lagged {
        count: u64,
        capabilities: Vec<String>,
    },
}

impl From<Delivery> for Announcement {
    fn from(delivery: Delivery) -> Announcement {
        match delivery {
            Delivery::Event(event) => Announcement::Event {
                capability: event.capability().to_string(),
                event: event.name().to_owned(),
                payload: event.payload().clone(),
            },
            Delivery::Lagged(lagged) => Announcement::Lagged {
                count: lagged.count(),
                capabilities: lagged
                    .capabilities()
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            },
        }
    }
}

impl Report for Announcement {
    fn write_text(&self, out: &mut dyn Write) -> io::Result<()> {
        match self {
            Announcement::Event {
                capability,
                event,
                payload,
            } => {
                write!(out, "{capability} {event} ")?;
                serde_json::to_writer(&mut *out, payload)?;
                writeln!(out)
            }
            Announcement::Lagged {
                count,
                capabilities,
            } => writeln!(
                out,
                "missed {count} events from {}",
                capabilities.join(", ")
            ),
        }
    }
}
