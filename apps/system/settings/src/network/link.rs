use rikka_app::prelude::*;

use rikka_network::Device;

use crate::info_row::info_row;
use crate::network::format;

pub(crate) fn link_details(device: &Device) -> Vec<Field> {
    let mut rows = Vec::new();
    if let Some(wireless) = &device.wireless
        && wireless.bitrate > 0
    {
        rows.extend(info_row(
            tr!("settings.network.detail.rate"),
            format::rate(wireless.bitrate),
        ));
    }
    if let Some(wired) = device.wired
        && wired.speed > 0
    {
        rows.extend(info_row(
            tr!("settings.network.detail.rate"),
            format::megabits(wired.speed),
        ));
    }
    rows.extend(info_row(
        tr!("settings.network.detail.ipv4-address"),
        format::addresses(&device.ip4),
    ));
    rows.extend(info_row(
        tr!("settings.network.detail.gateway"),
        device.ip4.gateway.clone(),
    ));
    rows.extend(info_row(
        tr!("settings.network.detail.ipv6-address"),
        format::addresses(&device.ip6),
    ));
    rows.extend(info_row(
        tr!("settings.network.detail.nameservers"),
        device.ip4.nameservers.join(", "),
    ));
    rows.extend(info_row(
        tr!("settings.network.detail.hardware"),
        device.hardware_address.clone(),
    ));
    rows
}
