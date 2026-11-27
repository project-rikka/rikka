use std::net::IpAddr;

use crate::error::{Error, Result};
use crate::ip::Address;
use crate::profile::dbus;
use crate::profile::{IpMethod, IpProfile};
use crate::values::{self, Draft};

pub(crate) const EXCLUSIVE_DNS_PRIORITY: i32 = -1500;
const INTERFACE_LENGTH: usize = 15;

pub(crate) struct Addressing<'a> {
    pub(crate) addresses: &'a [Address],
    pub(crate) dns: &'a [String],
    pub(crate) search: &'static str,
    pub(crate) exclusive: bool,
}

pub(crate) fn parse(subject: &'static str, addresses: &[String]) -> Result<Vec<Address>> {
    addresses
        .iter()
        .map(|address| {
            parse_address(address)
                .ok_or_else(|| Error::invalid(subject, format!("\"{address}\" is not an address")))
        })
        .collect()
}

pub(crate) fn write(draft: &mut Draft, addressing: &Addressing<'_>) {
    write_family(draft, dbus::IPV4, addressing, false);
    write_family(draft, dbus::IPV6, addressing, true);
}

pub(crate) fn is_interface(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= INTERFACE_LENGTH
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_-.=+".contains(character))
}

pub(crate) fn interface_for(name: &str, fallback: &str) -> String {
    let interface: String = name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || "_-.=+".contains(*character))
        .skip_while(|character| !character.is_ascii_alphanumeric())
        .take(INTERFACE_LENGTH)
        .collect();
    match interface.is_empty() {
        true => fallback.to_owned(),
        false => interface,
    }
}

pub(crate) fn is_six(address: &str) -> bool {
    address.contains(':')
}

fn write_family(draft: &mut Draft, setting: &str, addressing: &Addressing<'_>, six: bool) {
    let addresses: Vec<Address> = addressing
        .addresses
        .iter()
        .filter(|address| is_six(&address.address) == six)
        .cloned()
        .collect();
    if addresses.is_empty() {
        values::put(draft, setting, "method", "disabled".to_owned());
        return;
    }
    let searches = match addressing.exclusive {
        true => vec![addressing.search.to_owned()],
        false => Vec::new(),
    };
    dbus::write_ip(
        draft,
        setting,
        &IpProfile {
            method: IpMethod::Manual,
            addresses,
            nameservers: addressing
                .dns
                .iter()
                .filter(|address| is_six(address) == six)
                .cloned()
                .collect(),
            searches,
            ignore_automatic_dns: addressing.exclusive,
            ..IpProfile::default()
        },
        six,
    );
    if addressing.exclusive {
        values::put(draft, setting, "dns-priority", EXCLUSIVE_DNS_PRIORITY);
    }
}

fn parse_address(address: &str) -> Option<Address> {
    let (host, prefix) = match address.split_once('/') {
        Some((host, prefix)) => (host, prefix.parse::<u32>().ok()?),
        None if is_six(address) => (address, 128),
        None => (address, 32),
    };
    let parsed = host.parse::<IpAddr>().ok()?;
    let widest = match parsed {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };
    (prefix <= widest).then(|| Address {
        address: host.to_owned(),
        prefix,
    })
}
