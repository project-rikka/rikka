use rikka_app::prelude::*;
use rikka_network::{Band, Connectivity, DeviceState, Failure, IpFacts, Security};

pub(crate) fn security(security: Security) -> Label {
    match security {
        Security::Open => tr!("settings.network.security.open"),
        Security::Enhanced => tr!("settings.network.security.enhanced"),
        Security::Wep => tr!("settings.network.security.wep"),
        Security::Personal => tr!("settings.network.security.personal"),
        Security::Wpa3Personal => tr!("settings.network.security.wpa3"),
        Security::Enterprise => tr!("settings.network.security.enterprise"),
        Security::Wpa3Enterprise => tr!("settings.network.security.wpa3-enterprise"),
    }
}

pub(crate) fn band(band: Band) -> Label {
    match band {
        Band::Legacy => tr!("settings.network.band.legacy"),
        Band::Fast => tr!("settings.network.band.fast"),
        Band::Wide => tr!("settings.network.band.wide"),
        Band::Unknown => tr!("settings.network.band.unknown"),
    }
}

pub(crate) fn state(state: DeviceState, failure: Failure) -> Label {
    if state == DeviceState::Failed || state == DeviceState::Disconnected {
        match failure {
            Failure::Secrets => return tr!("settings.network.state.password"),
            Failure::Missing => return tr!("settings.network.state.missing"),
            Failure::Handshake => return tr!("settings.network.state.handshake"),
            Failure::Address => return tr!("settings.network.state.address"),
            Failure::Carrier => return tr!("settings.network.state.unplugged"),
            Failure::Other | Failure::None => {}
        }
    }
    match state {
        DeviceState::Unknown => tr!("settings.network.state.unknown"),
        DeviceState::Unmanaged => tr!("settings.network.state.unmanaged"),
        DeviceState::Unavailable => tr!("settings.network.state.unavailable"),
        DeviceState::Disconnected => tr!("settings.network.state.disconnected"),
        DeviceState::Connecting => tr!("settings.network.state.connecting"),
        DeviceState::NeedsSecrets => tr!("settings.network.state.password"),
        DeviceState::Connected => tr!("settings.network.state.connected"),
        DeviceState::Disconnecting => tr!("settings.network.state.disconnecting"),
        DeviceState::Failed => tr!("settings.network.state.failed"),
    }
}

pub(crate) fn connectivity(connectivity: Connectivity) -> Option<Label> {
    match connectivity {
        Connectivity::Portal => Some(tr!("settings.network.connectivity.portal")),
        Connectivity::Limited => Some(tr!("settings.network.connectivity.limited")),
        Connectivity::None => Some(tr!("settings.network.connectivity.none")),
        Connectivity::Full | Connectivity::Unknown => None,
    }
}

pub(crate) fn rate(kilobits: u32) -> String {
    if kilobits >= 1_000_000 {
        return format!("{:.1} Gb/s", f64::from(kilobits) / 1_000_000.0);
    }
    if kilobits >= 1_000 {
        return format!("{} Mb/s", kilobits / 1_000);
    }
    format!("{kilobits} kb/s")
}

pub(crate) fn megabits(megabits: u32) -> String {
    if megabits >= 1_000 {
        return format!("{:.1} Gb/s", f64::from(megabits) / 1_000.0);
    }
    format!("{megabits} Mb/s")
}

pub(crate) fn addresses(ip: &IpFacts) -> String {
    ip.addresses
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<String>>()
        .join(", ")
}
