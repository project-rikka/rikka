use rikka_bus::{ManagedObjects, Properties};

use crate::adapter::{Adapter, AdapterId};
use crate::device::{Device, DeviceId};
use crate::kind::DeviceKind;
use crate::session::proxies;
use crate::snapshot::Snapshot;

pub(crate) fn snapshot(objects: &ManagedObjects) -> Snapshot {
    let adapters = objects
        .implementing(proxies::ADAPTER)
        .map(|(path, adapter)| Adapter {
            id: AdapterId(path.to_owned()),
            address: adapter.string("Address").unwrap_or_default(),
            alias: adapter.string("Alias").unwrap_or_default(),
            powered: adapter.boolean("Powered").unwrap_or(false),
            discoverable: adapter.boolean("Discoverable").unwrap_or(false),
            pairable: adapter.boolean("Pairable").unwrap_or(false),
            discovering: adapter.boolean("Discovering").unwrap_or(false),
            discoverable_timeout: adapter.uint32("DiscoverableTimeout").unwrap_or(0),
        })
        .collect();

    let devices = objects
        .implementing(proxies::DEVICE)
        .filter_map(|(path, device)| {
            Some(Device {
                id: DeviceId(path.to_owned()),
                adapter: AdapterId(device.object_path("Adapter")?.to_owned()),
                address: device.string("Address").unwrap_or_default(),
                title: title(device),
                kind: kind(device),
                paired: device.boolean("Paired").unwrap_or(false),
                connected: device.boolean("Connected").unwrap_or(false),
                trusted: device.boolean("Trusted").unwrap_or(false),
                blocked: device.boolean("Blocked").unwrap_or(false),
                legacy_pairing: device.boolean("LegacyPairing").unwrap_or(false),
                signal: device.value::<i16>("RSSI"),
                battery: objects
                    .get(path, proxies::BATTERY)
                    .and_then(|battery| battery.uint32("Percentage"))
                    .and_then(|percentage| u8::try_from(percentage).ok()),
                services: device.strings("UUIDs").unwrap_or_default(),
            })
        })
        .collect();

    Snapshot { adapters, devices }
}

fn title(device: &Properties) -> String {
    ["Alias", "Name"]
        .into_iter()
        .find_map(|key| device.string(key).filter(|name| !name.is_empty()))
        .or_else(|| device.string("Address"))
        .unwrap_or_default()
}

fn kind(device: &Properties) -> DeviceKind {
    if let Some(kind) = device.string_ref("Icon").and_then(DeviceKind::from_icon) {
        return kind;
    }
    let class = device.uint32("Class").unwrap_or(0);
    if class != 0 {
        let kind = DeviceKind::from_class(class);
        if kind != DeviceKind::Unknown {
            return kind;
        }
    }
    device
        .uint32("Appearance")
        .and_then(|appearance| u16::try_from(appearance).ok())
        .map(DeviceKind::from_appearance)
        .unwrap_or_default()
}
