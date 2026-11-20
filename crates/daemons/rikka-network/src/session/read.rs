use std::collections::BTreeMap;

use rikka_bus::{ManagedObjects, Properties};
use zbus::blocking::Connection;

use crate::access_point::{AccessPoint, ApId, ApMode};
use crate::active::{Active, ActiveId, ActiveState};
use crate::device::{Device, DeviceId, DeviceKind, DeviceState, Wired, Wireless};
use crate::failure::Failure;
use crate::ip::IpFacts;
use crate::metered::Metered;
use crate::profile::{self, Profile};
use crate::security::Security;
use crate::session::proxies::{self, SavedConnectionProxyBlocking};
use crate::snapshot::{Connectivity, Radio, Snapshot};
use crate::ssid::Ssid;
use crate::values;

const CAN_BE_ACCESS_POINT: u32 = 0x40;

pub(crate) type Saved = BTreeMap<String, Profile>;

pub(crate) fn snapshot(
    connection: &Connection,
    objects: &ManagedObjects,
    cache: &mut Saved,
) -> Snapshot {
    let manager = objects.get(proxies::MANAGER_PATH, proxies::MANAGER);
    let flag = |key: &str| manager.and_then(|manager| manager.boolean(key));
    let profiles = profiles(connection, objects, cache);
    let active = active(objects);
    let devices = devices(objects, &active);
    let access_points = access_points(objects, &devices, &profiles);

    Snapshot {
        networking: flag("NetworkingEnabled").unwrap_or(false),
        wireless: Radio {
            enabled: flag("WirelessEnabled").unwrap_or(false),
            hardware: flag("WirelessHardwareEnabled").unwrap_or(false),
        },
        connectivity: Connectivity::from_code(
            manager
                .and_then(|manager| manager.uint32("Connectivity"))
                .unwrap_or(0),
        ),
        hostname: objects
            .get(proxies::SETTINGS_PATH, proxies::SETTINGS)
            .and_then(|store| store.string("Hostname"))
            .unwrap_or_default(),
        devices,
        access_points,
        profiles,
        active,
    }
}

fn profiles(connection: &Connection, objects: &ManagedObjects, cache: &mut Saved) -> Vec<Profile> {
    let paths: Vec<&str> = objects
        .implementing(proxies::CONNECTION)
        .map(|(path, _)| path)
        .collect();
    cache.retain(|path, _| paths.contains(&path.as_str()));
    for path in paths {
        if cache.contains_key(path) {
            continue;
        }
        match read_profile(connection, path) {
            Ok(profile) if !profile.uuid.is_empty() => {
                cache.insert(path.to_owned(), profile);
            }
            Ok(_) => tracing::debug!(path, "saved connection has no uuid"),
            Err(error) => tracing::debug!(path, "failed to read saved connection: {error}"),
        }
    }
    cache.values().cloned().collect()
}

fn read_profile(connection: &Connection, path: &str) -> zbus::Result<Profile> {
    let saved = SavedConnectionProxyBlocking::builder(connection)
        .path(path.to_owned())?
        .build()?;
    Ok(profile::read(&values::settings(saved.get_settings()?)))
}

fn active(objects: &ManagedObjects) -> Vec<Active> {
    objects
        .implementing(proxies::ACTIVE)
        .map(|(path, active)| Active {
            id: ActiveId(path.to_owned()),
            uuid: active.string("Uuid").unwrap_or_default(),
            title: active.string("Id").unwrap_or_default(),
            state: ActiveState::from_code(active.uint32("State").unwrap_or(0)),
            devices: active
                .object_paths("Devices")
                .unwrap_or_default()
                .into_iter()
                .map(DeviceId)
                .collect(),
            default4: active.boolean("Default").unwrap_or(false),
            default6: active.boolean("Default6").unwrap_or(false),
            vpn: active.boolean("Vpn").unwrap_or(false),
            access_point: active
                .object_path("SpecificObject")
                .map(|path| ApId(path.to_owned())),
        })
        .collect()
}

fn devices(objects: &ManagedObjects, active: &[Active]) -> Vec<Device> {
    let mut devices: Vec<Device> = objects
        .implementing(proxies::DEVICE)
        .map(|(path, device)| {
            let id = DeviceId(path.to_owned());
            let running = active
                .iter()
                .find(|running| running.devices.contains(&id))
                .map(|running| running.uuid.clone());
            let (state, reason) = state_of(device);
            Device {
                kind: DeviceKind::from_code(device.uint32("DeviceType").unwrap_or(0)),
                interface: device.string("Interface").unwrap_or_default(),
                state,
                failure: Failure::from_code(reason),
                autoconnect: device.boolean("Autoconnect").unwrap_or(false),
                metered: Metered::from_code(device.uint32("Metered").unwrap_or(0)),
                mtu: device.uint32("Mtu").unwrap_or(0),
                driver: device.string("Driver").unwrap_or_default(),
                hardware_address: device.string("HwAddress").unwrap_or_default(),
                active: running,
                ip4: facts(objects, device.object_path("Ip4Config"), proxies::IP4),
                ip6: facts(objects, device.object_path("Ip6Config"), proxies::IP6),
                wired: objects.get(path, proxies::WIRED).map(wired),
                wireless: objects.get(path, proxies::WIRELESS).map(wireless),
                id,
            }
        })
        .collect();
    devices.sort_by(|left, right| left.interface.cmp(&right.interface));
    devices
}

fn state_of(device: &Properties) -> (DeviceState, u32) {
    let state = DeviceState::from_code(device.uint32("State").unwrap_or(0));
    let reason = device
        .value::<(u32, u32)>("StateReason")
        .map(|(_, reason)| reason)
        .unwrap_or(0);
    (state, reason)
}

fn wired(wired: &Properties) -> Wired {
    Wired {
        carrier: wired.boolean("Carrier").unwrap_or(false),
        speed: wired.uint32("Speed").unwrap_or(0),
    }
}

fn wireless(wireless: &Properties) -> Wireless {
    Wireless {
        bitrate: wireless.uint32("Bitrate").unwrap_or(0),
        access_point: wireless
            .object_path("ActiveAccessPoint")
            .map(|path| ApId(path.to_owned())),
        last_scan: wireless.int64("LastScan").unwrap_or(0),
        hotspot_capable: wireless.uint32("WirelessCapabilities").unwrap_or(0) & CAN_BE_ACCESS_POINT
            != 0,
        permanent_address: wireless.string("PermHwAddress").unwrap_or_default(),
    }
}

fn access_points(
    objects: &ManagedObjects,
    devices: &[Device],
    profiles: &[Profile],
) -> Vec<AccessPoint> {
    let owners: BTreeMap<String, &Device> = objects
        .implementing(proxies::WIRELESS)
        .filter_map(|(path, wireless)| {
            let device = devices.iter().find(|device| device.id.as_str() == path)?;
            Some(
                wireless
                    .object_paths("AccessPoints")
                    .unwrap_or_default()
                    .into_iter()
                    .map(move |point| (point, device)),
            )
        })
        .flatten()
        .collect();

    objects
        .implementing(proxies::ACCESS_POINT)
        .filter_map(|(path, point)| {
            let device = owners.get(path)?;
            let id = ApId(path.to_owned());
            let ssid = Ssid::new(point.list::<u8>("Ssid").unwrap_or_default());
            let saved = profiles
                .iter()
                .find(|profile| profile.ssid() == Some(&ssid))
                .map(|profile| profile.uuid.clone());
            Some(AccessPoint {
                in_use: device
                    .wireless
                    .as_ref()
                    .and_then(|wireless| wireless.access_point.as_ref())
                    == Some(&id),
                device: device.id.clone(),
                bssid: point.string("HwAddress").unwrap_or_default(),
                strength: u8::try_from(point.uint32("Strength").unwrap_or(0)).unwrap_or(0),
                frequency: point.uint32("Frequency").unwrap_or(0),
                max_bitrate: point.uint32("MaxBitrate").unwrap_or(0),
                security: Security::read(
                    point.uint32("Flags").unwrap_or(0),
                    point.uint32("WpaFlags").unwrap_or(0),
                    point.uint32("RsnFlags").unwrap_or(0),
                ),
                wps: Security::supports_wps(point.uint32("Flags").unwrap_or(0)),
                mode: ApMode::from_code(point.uint32("Mode").unwrap_or(0)),
                ssid,
                saved,
                id,
            })
        })
        .collect()
}

fn facts(objects: &ManagedObjects, path: Option<&str>, interface: &str) -> IpFacts {
    let Some(config) = path.and_then(|path| objects.get(path, interface)) else {
        return IpFacts::default();
    };
    IpFacts {
        addresses: crate::ip::addresses(config, "AddressData"),
        gateway: config.string("Gateway").unwrap_or_default(),
        nameservers: config
            .maps("NameserverData")
            .unwrap_or_default()
            .iter()
            .filter_map(|server| Some(server.string_ref("address")?.to_owned()))
            .collect(),
        domains: config.strings("Domains").unwrap_or_default(),
        searches: config.strings("Searches").unwrap_or_default(),
        routes: crate::ip::routes(config, "RouteData"),
    }
}
