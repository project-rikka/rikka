use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use zbus::blocking::Connection;
use zbus::message::Message;

use crate::device::{Device, DeviceId, DeviceKind, Level};
use crate::history::{self, Sample};
use crate::pack::{Pack, Technology, Threshold};
use crate::proxies::{
    BATTERY, DEVICE, DISPLAY_DEVICE, DeviceProxy, MANAGER_PATH, ManagerProxy, SERVICE,
};
use crate::reading::{Charge, Reading, Warning};
use crate::state::{Action, Event, State};

use rikka_bus::{
    Coalesce, EventSink, Handler, Notifier, Properties, Rule, Signals, StartError, Watch,
};

#[derive(Debug, Default)]
pub(crate) struct Changes {
    listing: bool,
    supply: bool,
    paths: BTreeSet<String>,
}

impl Coalesce for Changes {
    fn merge(&mut self, other: Changes) {
        self.listing |= other.listing;
        self.supply |= other.supply;
        self.paths.extend(other.paths);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Request {
    Preserve { pack: DeviceId, on: bool },
    History { span: Duration, points: u32 },
}

pub(crate) struct Upower {
    connection: Connection,
    manager: ManagerProxy<'static>,
    main: Option<Reading>,
    on_battery: bool,
    devices: BTreeMap<String, Device>,
    packs: BTreeMap<String, Pack>,
    history: Vec<Sample>,
    _signals: Option<Watch>,
}

impl Handler for Upower {
    type Config = ();
    type Request = Request;
    type Notice = Changes;
    type Event = Event;
    type State = State;

    const THREAD_NAME: &'static str = "rikka-upower";
    const UNAVAILABLE: &'static str = "this machine has no battery to show";

    fn open(
        (): (),
        notifier: Notifier<Upower>,
        report: &EventSink<Event>,
    ) -> Result<Upower, StartError> {
        let connection = Connection::system()?;
        let manager = ManagerProxy::builder(&connection)
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()?;
        let signals = watch(&connection, notifier)
            .inspect_err(|error| tracing::warn!("failed to watch UPower signals: {error}"))
            .ok();
        let mut upower = Upower {
            connection,
            manager,
            main: None,
            on_battery: false,
            devices: BTreeMap::new(),
            packs: BTreeMap::new(),
            history: Vec::new(),
            _signals: signals,
        };
        upower.handle_notice(
            Changes {
                listing: true,
                supply: true,
                paths: BTreeSet::from([DISPLAY_DEVICE.to_owned()]),
            },
            report,
        );
        Ok(upower)
    }

    fn handle_request(&mut self, request: Request, report: &EventSink<Event>) {
        match request {
            Request::Preserve { pack, on } => {
                let error = self.preserve(&pack, on).err().map(|error| {
                    tracing::warn!("failed to change the charge limit: {error}");
                    error.to_string()
                });
                self.reread(pack.as_str());
                report.emit(Event::Read(self.state()));
                report.emit(Event::Answered {
                    action: Action::Preserve,
                    error,
                });
            }
            Request::History { span, points } => {
                self.read_history(span, points);
                report.emit(Event::Read(self.state()));
            }
        }
    }

    fn handle_notice(&mut self, notice: Changes, report: &EventSink<Event>) {
        if notice.listing {
            self.relist();
        }
        if notice.supply {
            self.on_battery = self.manager.on_battery().unwrap_or(false);
        }
        for path in notice.paths {
            self.reread(&path);
        }
        report.emit(Event::Read(self.state()));
    }

    fn replaces(held: &Request, next: &Request) -> bool {
        matches!(
            (held, next),
            (Request::History { .. }, Request::History { .. })
        )
    }
}

impl Upower {
    fn state(&self) -> State {
        let mut devices: Vec<Device> = self.devices.values().cloned().collect();
        devices.sort_by(|left, right| (&left.name, &left.id).cmp(&(&right.name, &right.id)));
        State {
            main: self.main,
            on_battery: self.on_battery,
            devices,
            packs: self.packs.values().cloned().collect(),
            history: self.history.clone(),
            error: None,
        }
    }

    fn read_history(&mut self, span: Duration, points: u32) {
        let Some(pack) = self.packs.keys().next() else {
            self.history.clear();
            return;
        };
        self.history = history::read(&self.connection, pack, span, points)
            .inspect_err(|error| tracing::debug!("failed to read battery history: {error}"))
            .unwrap_or_default();
    }

    fn reread(&mut self, path: &str) {
        let read = self.properties(path);
        if path == DISPLAY_DEVICE {
            self.main = read.as_ref().and_then(main);
            return;
        }
        match read
            .as_ref()
            .and_then(|properties| device(path, properties))
        {
            Some(device) => {
                self.devices.insert(path.to_owned(), device);
            }
            None => {
                self.devices.remove(path);
            }
        }
        match read.as_ref().and_then(|properties| pack(path, properties)) {
            Some(pack) => {
                self.packs.insert(path.to_owned(), pack);
            }
            None => {
                self.packs.remove(path);
            }
        }
    }

    fn relist(&mut self) {
        let listed = self
            .manager
            .enumerate_devices()
            .inspect_err(|error| tracing::debug!("failed to enumerate UPower devices: {error}"))
            .unwrap_or_default();
        self.devices.clear();
        self.packs.clear();
        for path in listed {
            self.reread(path.as_str());
        }
    }

    fn preserve(&self, pack: &DeviceId, on: bool) -> zbus::Result<()> {
        DeviceProxy::builder(&self.connection)
            .path(pack.as_str().to_owned())?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()?
            .enable_charge_threshold(on)
    }

    fn properties(&self, path: &str) -> Option<Properties> {
        Properties::read(&self.connection, SERVICE, path, DEVICE)
            .inspect_err(|error| tracing::debug!("failed to read {path}: {error}"))
            .ok()
    }
}

fn watch(connection: &Connection, notifier: Notifier<Upower>) -> zbus::Result<Watch> {
    Signals::new()
        .rule(Rule::new().sender(SERVICE))
        .listen(connection, sort, move |notice| notifier.notify(notice))
}

fn sort(message: &Message) -> Option<Changes> {
    let header = message.header();
    let path = header.path()?.as_str();
    let mut notice = Changes::default();
    match (path, header.member()?.as_str()) {
        (_, "DeviceAdded" | "DeviceRemoved") => notice.listing = true,
        (MANAGER_PATH, _) => notice.supply = true,
        (path, _) => {
            notice.paths.insert(path.to_owned());
        }
    }
    Some(notice)
}

fn main(properties: &Properties) -> Option<Reading> {
    if properties.uint32("Type")? != BATTERY || !properties.boolean("IsPresent")? {
        return None;
    }
    let state = Charge::from_code(properties.uint32("State")?);
    let seconds = match state {
        Charge::Charging => properties.int64("TimeToFull"),
        _ => properties.int64("TimeToEmpty"),
    }
    .unwrap_or_default();
    Some(Reading {
        charge: (properties.double("Percentage")? / 100.0) as f32,
        state,
        left: (seconds > 0).then(|| Duration::from_secs(seconds as u64)),
        warning: Warning::from_code(properties.uint32("WarningLevel").unwrap_or_default()),
        rate: properties
            .double("EnergyRate")
            .filter(|watts| *watts > 0.0)
            .map(|watts| watts as f32),
    })
}

fn device(path: &str, properties: &Properties) -> Option<Device> {
    let kind = DeviceKind::from_code(properties.uint32("Type")?)?;
    if properties.boolean("PowerSupply") == Some(true) && kind != DeviceKind::Ups {
        return None;
    }
    let level = properties.uint32("BatteryLevel").and_then(Level::from_code);
    let percentage = properties.double("Percentage").unwrap_or_default();
    if level.is_none() && percentage <= 0.0 {
        return None;
    }
    Some(Device {
        id: DeviceId(path.to_owned()),
        kind,
        name: name(properties),
        charge: match level {
            Some(_) => None,
            None => Some((percentage / 100.0) as f32),
        },
        level: level.unwrap_or_default(),
        state: Charge::from_code(properties.uint32("State").unwrap_or_default()),
        warning: Warning::from_code(properties.uint32("WarningLevel").unwrap_or_default()),
        icon: text(properties, "IconName").map(str::to_owned),
    })
}

fn pack(path: &str, properties: &Properties) -> Option<Pack> {
    let is_pack = properties.uint32("Type") == Some(BATTERY)
        && properties.boolean("PowerSupply") == Some(true)
        && properties.boolean("IsPresent") == Some(true);
    if !is_pack {
        return None;
    }
    let positive = |key: &str| {
        properties
            .double(key)
            .filter(|measured| *measured > 0.0)
            .map(|measured| measured as f32)
    };
    let threshold =
        (properties.boolean("ChargeThresholdSupported") == Some(true)).then(|| Threshold {
            enabled: properties
                .boolean("ChargeThresholdEnabled")
                .unwrap_or(false),
            start: properties
                .uint32("ChargeStartThreshold")
                .unwrap_or_default(),
            end: properties.uint32("ChargeEndThreshold").unwrap_or_default(),
        });
    Some(Pack {
        id: DeviceId(path.to_owned()),
        name: name(properties),
        technology: Technology::from_code(properties.uint32("Technology").unwrap_or_default()),
        health: properties
            .double("Capacity")
            .filter(|capacity| *capacity > 0.0)
            .map(|capacity| (capacity / 100.0).clamp(0.0, 1.0) as f32),
        energy: positive("Energy"),
        energy_full: positive("EnergyFull"),
        energy_full_design: positive("EnergyFullDesign"),
        voltage: positive("Voltage"),
        cycles: properties
            .uint32("ChargeCycles")
            .filter(|cycles| *cycles > 0),
        threshold,
    })
}

fn name(properties: &Properties) -> String {
    match (text(properties, "Vendor"), text(properties, "Model")) {
        (Some(vendor), Some(model)) if !model.contains(vendor) => format!("{vendor} {model}"),
        (_, Some(model)) => model.to_owned(),
        (Some(vendor), None) => vendor.to_owned(),
        (None, None) => String::new(),
    }
}

fn text<'a>(properties: &'a Properties, key: &str) -> Option<&'a str> {
    properties
        .string_ref(key)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}
