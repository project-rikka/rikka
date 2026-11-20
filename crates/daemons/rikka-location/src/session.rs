use std::time::{Duration, SystemTime};

use std::convert::Infallible;

use rikka_bus::{EventSink, Handler, Notifier, Properties, Rule, Signals, StartError, Watch};
use zbus::blocking::Connection;
use zbus::proxy::CacheProperties;
use zbus::zvariant::OwnedObjectPath;

use crate::fix::{Accuracy, Fix};
use crate::proxies::{ClientProxy, ManagerProxy};

const UNKNOWN_ALTITUDE: f64 = -1.0e300;
const NOWHERE: &str = "/";
const SERVICE: &str = "org.freedesktop.GeoClue2";
const CLIENT: &str = "org.freedesktop.GeoClue2.Client";
const LOCATION: &str = "org.freedesktop.GeoClue2.Location";
const UPDATED: &str = "LocationUpdated";

#[derive(Clone, Debug, Default)]
pub(crate) struct Request {
    pub(crate) desktop: String,
    pub(crate) accuracy: Accuracy,
}

pub(crate) struct GeoClue {
    connection: Connection,
    client: ClientProxy<'static>,
    _signals: Watch,
}

impl Handler for GeoClue {
    type Config = Request;
    type Request = Infallible;
    type Notice = OwnedObjectPath;
    type Event = Option<Fix>;
    type State = Option<Fix>;

    const THREAD_NAME: &'static str = "rikka-geoclue";
    const UNAVAILABLE: &'static str = "this machine cannot say where it is";

    fn open(
        request: Request,
        notifier: Notifier<GeoClue>,
        report: &EventSink<Option<Fix>>,
    ) -> Result<GeoClue, StartError> {
        let connection = Connection::system()?;
        let manager = ManagerProxy::new(&connection)?;
        let path = manager.get_client()?;
        let client = ClientProxy::builder(&connection)
            .path(path)?
            .cache_properties(CacheProperties::No)
            .build()?;
        client.set_desktop_id(&request.desktop)?;
        client.set_requested_accuracy_level(request.accuracy.level())?;
        client.set_distance_threshold(request.accuracy.threshold())?;

        let signals = Signals::new()
            .rule(
                Rule::new()
                    .sender(SERVICE)
                    .path(client.inner().path().as_str())
                    .interface(CLIENT)
                    .member(UPDATED),
            )
            .each(
                &connection,
                |message| {
                    let (_, new): (OwnedObjectPath, OwnedObjectPath) =
                        message.body().deserialize().ok()?;
                    Some(new)
                },
                move |new| notifier.notify(new),
            )?;
        client.start()?;

        let geoclue = GeoClue {
            connection,
            client,
            _signals: signals,
        };
        if let Ok(path) = geoclue.client.location() {
            report.emit(geoclue.read(&path));
        }
        Ok(geoclue)
    }

    fn handle_request(&mut self, request: Infallible, _report: &EventSink<Option<Fix>>) {
        match request {}
    }

    fn handle_notice(&mut self, path: OwnedObjectPath, report: &EventSink<Option<Fix>>) {
        report.emit(self.read(&path));
    }
}

impl GeoClue {
    fn read(&self, path: &OwnedObjectPath) -> Option<Fix> {
        if path.as_str() == NOWHERE {
            return None;
        }
        let location = Properties::read(&self.connection, SERVICE, path.as_str(), LOCATION)
            .inspect_err(|error| tracing::debug!("failed to read GeoClue location: {error}"))
            .ok()?;
        let (seconds, micros) = location
            .value::<(u64, u64)>("Timestamp")
            .unwrap_or_default();
        Some(Fix {
            latitude: location.double("Latitude")?,
            longitude: location.double("Longitude")?,
            accuracy: location.double("Accuracy").unwrap_or(f64::INFINITY),
            altitude: location
                .double("Altitude")
                .filter(|altitude| *altitude > UNKNOWN_ALTITUDE),
            description: location
                .string_ref("Description")
                .filter(|description| !description.is_empty())
                .map(str::to_owned),
            at: SystemTime::UNIX_EPOCH
                + Duration::from_secs(seconds)
                + Duration::from_micros(micros),
        })
    }
}

impl Drop for GeoClue {
    fn drop(&mut self) {
        if let Err(error) = self.client.stop() {
            tracing::debug!("failed to stop GeoClue client: {error}");
        }
        if let Err(error) = self.connection.clone().close() {
            tracing::debug!("failed to close GeoClue connection: {error}");
        }
    }
}
