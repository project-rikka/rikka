use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};

use crate::service::{HOLD, INTERFACE, OBJECT, SERVICE};

const HELD_FOR: Duration = Duration::from_secs(1);

const RENEW_AFTER: Duration = Duration::from_millis(250);

const LOOK_AGAIN_AFTER: Duration = Duration::from_secs(30);

const UNINSTALLED: &str = "org.freedesktop.DBus.Error.ServiceUnknown";

#[derive(Debug)]
pub struct Booster {
    sender: SyncSender<()>,
}

impl Booster {
    pub fn start() -> Self {
        let (sender, touches) = mpsc::sync_channel(1);
        let started = std::thread::Builder::new()
            .name("rikka-boost".to_owned())
            .spawn(move || serve(&touches));
        if let Err(error) = started {
            tracing::info!("failed to spawn GPU boost thread: {error}");
        }
        let booster = Self { sender };
        booster.touch();
        booster
    }

    pub fn touch(&self) {
        let _ = self.sender.try_send(());
    }
}

fn serve(touches: &Receiver<()>) {
    let millis =
        u32::try_from(HELD_FOR.as_millis()).expect("boost hold should fit in u32 milliseconds");
    let mut connected: Option<zbus::blocking::Connection> = None;
    let mut sent: Option<Instant> = None;
    let mut missing: Option<Instant> = None;
    while touches.recv().is_ok() {
        if sent.is_some_and(|sent| sent.elapsed() < RENEW_AFTER) {
            continue;
        }
        if connected.is_none() && missing.is_some_and(|since| since.elapsed() < LOOK_AGAIN_AFTER) {
            continue;
        }
        let asking = match connected.take() {
            Some(connection) => Ok(connection),
            None => zbus::blocking::Connection::system(),
        };
        match asking.and_then(|connection| ask(&connection, millis).map(|()| connection)) {
            Ok(connection) => {
                if sent.is_none() {
                    tracing::info!("GPU clock boost active");
                }
                connected = Some(connection);
                sent = Some(Instant::now());
                missing = None;
            }
            Err(error) if absent(&error) => {
                if missing.is_none() {
                    tracing::info!(
                        "{SERVICE} is not available, GPU clocks stay driver-managed: {error}"
                    );
                }
                sent = None;
                missing = Some(Instant::now());
            }
            Err(error) => {
                tracing::info!("{SERVICE} rejected the boost request, disabling: {error}");
                return;
            }
        }
    }
}

fn absent(error: &zbus::Error) -> bool {
    match error {
        zbus::Error::MethodError(name, _, _) => name.as_str() == UNINSTALLED,
        _ => true,
    }
}

fn ask(connection: &zbus::blocking::Connection, millis: u32) -> zbus::Result<()> {
    connection.call_method(Some(SERVICE), OBJECT, Some(INTERFACE), HOLD, &(millis,))?;
    Ok(())
}
