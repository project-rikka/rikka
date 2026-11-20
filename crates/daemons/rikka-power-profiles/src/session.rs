use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::proxy::CacheProperties;
use zbus::zvariant::OwnedValue;

use crate::profile::{Degraded, Hold, Profile};
use crate::proxies::{ADDRESSES, Address, ProfilesProxy};
use crate::state::{Event, Reading};
use rikka_bus::{
    EventSink, Fault, Handler, Notifier, Properties, Rule, Signals, StartError, Watch,
};

type Entry = HashMap<String, OwnedValue>;

pub(crate) struct Daemon {
    connection: Connection,
    address: &'static Address,
    profiles: ProfilesProxy<'static>,
    _signals: Option<Watch>,
}

pub(crate) enum Request {
    Set(Profile),
}

impl Handler for Daemon {
    type Config = ();
    type Request = Request;
    type Notice = ();
    type Event = Event;
    type State = crate::state::State;

    const THREAD_NAME: &'static str = "rikka-power-profiles";
    const UNAVAILABLE: &'static str = "this machine offers no power profiles";

    fn open(
        (): (),
        notifier: Notifier<Daemon>,
        report: &EventSink<Event>,
    ) -> Result<Daemon, StartError> {
        let connection = Connection::system()?;
        let (profiles, address) = ADDRESSES
            .iter()
            .find_map(|address| Some((connect_proxy(&connection, address)?, address)))
            .ok_or("neither power-profiles-daemon nor tuned-ppd answers")?;
        let signals = watch(&connection, address, notifier)
            .inspect_err(|error| tracing::warn!("failed to watch power profile signals: {error}"))
            .ok();
        let daemon = Daemon {
            connection,
            address,
            profiles,
            _signals: signals,
        };
        report.emit(Event::Read(daemon.read()));
        Ok(daemon)
    }

    fn handle_request(&mut self, request: Request, report: &EventSink<Event>) {
        match request {
            Request::Set(profile) => {
                let result = self
                    .profiles
                    .set_active_profile(profile.name())
                    .map_err(|error| Fault::from_zbus(rikka_bus::Daemon::PowerProfiles, &error));
                let error = result.err().map(|error| {
                    tracing::warn!("failed to set power profile: {error}");
                    error.to_string()
                });
                let failed = error.is_some();
                report.emit(Event::Answered { profile, error });
                if failed {
                    report.emit(Event::Read(self.read()));
                }
            }
        }
    }

    fn handle_notice(&mut self, (): (), report: &EventSink<Event>) {
        report.emit(Event::Read(self.read()));
    }

    fn replaces(_held: &Request, _next: &Request) -> bool {
        true
    }
}

impl Daemon {
    fn read(&self) -> Reading {
        let address = self.address;
        let read = Properties::read(
            &self.connection,
            address.service,
            address.path,
            address.interface,
        )
        .inspect_err(|error| tracing::debug!("{} is not responding: {error}", address.service))
        .unwrap_or_default();
        let entries = |key: &str| {
            read.value::<Vec<Entry>>(key)
                .unwrap_or_default()
                .into_iter()
                .map(Properties::from)
        };
        let mut profiles: Vec<Profile> = entries("Profiles")
            .filter_map(|entry| Profile::named(entry.string_ref("Profile")?))
            .collect();
        profiles.sort();
        profiles.dedup();
        Reading {
            active: read.string_ref("ActiveProfile").and_then(Profile::named),
            profiles,
            degraded: read
                .string_ref("PerformanceDegraded")
                .and_then(Degraded::named),
            holds: entries("ActiveProfileHolds")
                .filter_map(|entry| hold(&entry))
                .collect(),
        }
    }
}

fn connect_proxy(connection: &Connection, address: &Address) -> Option<ProfilesProxy<'static>> {
    let profiles = ProfilesProxy::builder(connection)
        .destination(address.service)
        .ok()?
        .path(address.path)
        .ok()?
        .interface(address.interface)
        .ok()?
        .cache_properties(CacheProperties::No)
        .build()
        .ok()?;
    profiles
        .active_profile()
        .inspect_err(|error| tracing::debug!("{} is not responding: {error}", address.service))
        .ok()?;
    Some(profiles)
}

fn watch(
    connection: &Connection,
    address: &Address,
    notifier: Notifier<Daemon>,
) -> zbus::Result<Watch> {
    Signals::new()
        .rule(Rule::new().sender(address.service).path(address.path))
        .listen(connection, |_| Some(()), move |()| notifier.notify(()))
}

fn hold(entry: &Properties) -> Option<Hold> {
    Some(Hold {
        application: entry.string_ref("ApplicationId")?.to_owned(),
        profile: Profile::named(entry.string_ref("Profile")?)?,
        reason: entry.string_ref("Reason").unwrap_or_default().to_owned(),
    })
}
