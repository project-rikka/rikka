use std::path::PathBuf;

use zbus::zvariant::Value;

use crate::access_point::ApMode;
use crate::metered::Metered;
use crate::profile::eap::{Eap, EapMethod, Phase2};
use crate::profile::ip::{IpMethod, IpProfile};
use crate::profile::mac::MacAddress;
use crate::profile::proxy::{ProxyMethod, ProxyProfile};
use crate::profile::wireless::{SecurityProfile, WirelessProfile};
use crate::profile::{Profile, ProfileKind};
use crate::security::Security;
use crate::ssid::Ssid;
use crate::values::{self, Draft, Settings};
use rikka_bus::Properties;

pub(crate) const CONNECTION: &str = "connection";
pub(crate) const WIRELESS: &str = "802-11-wireless";
pub(crate) const SECURITY: &str = "802-11-wireless-security";
pub(crate) const EAP: &str = "802-1x";
pub(crate) const IPV4: &str = "ipv4";
pub(crate) const IPV6: &str = "ipv6";
const VPN: &str = "vpn";
const PROXY: &str = "proxy";

const CERTIFICATE_SCHEME: &[u8] = b"file://";

pub(crate) fn read(settings: &Settings) -> Profile {
    let connection = settings.get(CONNECTION);
    let text = |key: &str| connection.and_then(|connection| connection.string(key));
    let kind = ProfileKind::read(
        connection
            .and_then(|connection| connection.string_ref("type"))
            .unwrap_or_default(),
        settings
            .get(VPN)
            .and_then(|vpn| vpn.string_ref("service-type"))
            .unwrap_or_default(),
    );
    let hardware = kind.hardware_setting().and_then(|name| settings.get(name));

    Profile {
        uuid: text("uuid").unwrap_or_default(),
        id: text("id").unwrap_or_default(),
        interface: text("interface-name").unwrap_or_default(),
        autoconnect: connection
            .and_then(|connection| connection.boolean("autoconnect"))
            .unwrap_or(true),
        autoconnect_priority: connection
            .and_then(|connection| connection.int64("autoconnect-priority"))
            .and_then(|priority| i32::try_from(priority).ok())
            .unwrap_or(0),
        metered: Metered::from_code(
            connection
                .and_then(|connection| connection.uint32("metered"))
                .unwrap_or(0),
        ),
        shared: connection
            .and_then(|connection| connection.strings("permissions"))
            .is_none_or(|permissions| permissions.is_empty()),
        timestamp: connection
            .and_then(|connection| connection.uint64("timestamp"))
            .unwrap_or(0),
        mtu: hardware
            .and_then(|hardware| hardware.uint32("mtu"))
            .unwrap_or(0),
        address: hardware.map(read_address).unwrap_or_default(),
        wireless: settings.get(WIRELESS).map(read_wireless),
        security: settings
            .get(SECURITY)
            .map(|security| read_security(security, settings.contains_key(EAP))),
        eap: settings.get(EAP).map(read_eap),
        ipv4: settings
            .get(IPV4)
            .map(|ip| read_ip(ip, false))
            .unwrap_or_default(),
        ipv6: settings
            .get(IPV6)
            .map(|ip| read_ip(ip, true))
            .unwrap_or_default(),
        proxy: settings.get(PROXY).map(read_proxy).unwrap_or_default(),
        kind,
    }
}

pub(crate) fn merge(draft: &mut Draft, profile: &Profile, user: &str) {
    values::put(draft, CONNECTION, "id", profile.id.clone());
    values::put(draft, CONNECTION, "autoconnect", profile.autoconnect);
    values::put(
        draft,
        CONNECTION,
        "autoconnect-priority",
        profile.autoconnect_priority,
    );
    values::put(draft, CONNECTION, "metered", profile.metered.to_code());
    values::put(
        draft,
        CONNECTION,
        "permissions",
        permissions(profile.shared, user),
    );

    if let Some(hardware) = profile.kind.hardware_setting() {
        values::put(draft, hardware, "mtu", profile.mtu);
        values::erase(draft, hardware, "cloned-mac-address");
        values::put(
            draft,
            hardware,
            "assigned-mac-address",
            profile.address.write().to_owned(),
        );
    }

    if let Some(wireless) = &profile.wireless {
        values::put(draft, WIRELESS, "ssid", wireless.ssid.as_bytes().to_vec());
        values::put(draft, WIRELESS, "hidden", wireless.hidden);
        values::put(draft, WIRELESS, "mode", mode_name(wireless.mode).to_owned());
    }
    if let Some(security) = &profile.security
        && let Some(passphrase) = &security.passphrase
    {
        write_passphrase(draft, security.security, passphrase);
    }
    if let Some(eap) = &profile.eap {
        write_eap(draft, eap);
    }

    write_ip(draft, IPV4, &profile.ipv4, false);
    write_ip(draft, IPV6, &profile.ipv6, true);
    write_proxy(draft, &profile.proxy);
}

pub(crate) fn permissions(shared: bool, user: &str) -> Vec<String> {
    if shared || user.is_empty() {
        return Vec::new();
    }
    vec![format!("user:{user}")]
}

pub(crate) fn write_passphrase(draft: &mut Draft, security: Security, passphrase: &str) {
    if security == Security::Wep {
        values::put(draft, SECURITY, "wep-key0", passphrase.to_owned());
        values::put(draft, SECURITY, "wep-key-type", wep_key_type(passphrase));
        return;
    }
    values::put(draft, SECURITY, "psk", passphrase.to_owned());
}

fn wep_key_type(key: &str) -> u32 {
    let hexadecimal = key.chars().all(|character| character.is_ascii_hexdigit());
    match key.len() {
        10 | 26 if hexadecimal => 1,
        5 | 13 | 16 => 1,
        _ => 2,
    }
}

fn read_wireless(wireless: &Properties) -> WirelessProfile {
    WirelessProfile {
        ssid: Ssid::new(wireless.list::<u8>("ssid").unwrap_or_default()),
        hidden: wireless.boolean("hidden").unwrap_or(false),
        mode: ApMode::from_name(wireless.string_ref("mode").unwrap_or_default()),
    }
}

fn mode_name(mode: ApMode) -> &'static str {
    match mode {
        ApMode::AdHoc => "adhoc",
        ApMode::Hotspot => "ap",
        ApMode::Mesh => "mesh",
        ApMode::Infrastructure | ApMode::Unknown => "infrastructure",
    }
}

fn read_security(security: &Properties, eap: bool) -> SecurityProfile {
    SecurityProfile {
        security: Security::from_key_management(
            security.string_ref("key-mgmt").unwrap_or_default(),
            eap,
        ),
        passphrase: None,
    }
}

fn read_address(hardware: &Properties) -> MacAddress {
    if let Some(assigned) = hardware.string_ref("assigned-mac-address") {
        return MacAddress::read(assigned);
    }
    let legacy = hardware
        .list::<u8>("cloned-mac-address")
        .unwrap_or_default();
    if legacy.is_empty() {
        return MacAddress::Unset;
    }
    MacAddress::Fixed(
        legacy
            .iter()
            .map(|octet| format!("{octet:02X}"))
            .collect::<Vec<String>>()
            .join(":"),
    )
}

fn read_eap(eap: &Properties) -> Eap {
    let methods = eap.strings("eap").unwrap_or_default();
    Eap {
        method: EapMethod::read(methods.first().map(String::as_str).unwrap_or_default()),
        identity: eap.string("identity").unwrap_or_default(),
        anonymous_identity: eap.string("anonymous-identity").unwrap_or_default(),
        authority: read_certificate(eap, "ca-cert"),
        domain_suffix: eap.string("domain-suffix-match").unwrap_or_default(),
        certificate: read_certificate(eap, "client-cert"),
        private_key: read_certificate(eap, "private-key"),
        private_key_password: None,
        phase2: Phase2::read(
            eap.string_ref("phase2-auth")
                .or_else(|| eap.string_ref("phase2-autheap"))
                .unwrap_or_default(),
        ),
        password: None,
    }
}

fn read_certificate(eap: &Properties, key: &str) -> Option<PathBuf> {
    let bytes = eap.list::<u8>(key).unwrap_or_default();
    let path = bytes.strip_prefix(CERTIFICATE_SCHEME)?;
    let path = path.strip_suffix(b"\0").unwrap_or(path);
    (!path.is_empty()).then(|| PathBuf::from(String::from_utf8_lossy(path).into_owned()))
}

pub(crate) fn write_eap(draft: &mut Draft, eap: &Eap) {
    values::put(draft, EAP, "eap", vec![eap.method.write().to_owned()]);
    values::put(draft, EAP, "identity", eap.identity.clone());
    values::put(
        draft,
        EAP,
        "anonymous-identity",
        eap.anonymous_identity.clone(),
    );
    values::put(draft, EAP, "domain-suffix-match", eap.domain_suffix.clone());
    write_certificate(draft, "ca-cert", eap.authority.as_deref());
    write_certificate(draft, "client-cert", eap.certificate.as_deref());
    write_certificate(draft, "private-key", eap.private_key.as_deref());

    if eap.method.is_tunnelled() {
        values::put(draft, EAP, "phase2-auth", eap.phase2.write().to_owned());
    } else {
        values::erase(draft, EAP, "phase2-auth");
    }
    values::erase(draft, EAP, "phase2-autheap");

    if let Some(password) = &eap.password {
        values::put(draft, EAP, "password", password.clone());
    }
    if let Some(password) = &eap.private_key_password {
        values::put(draft, EAP, "private-key-password", password.clone());
    }
}

fn write_certificate(draft: &mut Draft, key: &str, path: Option<&std::path::Path>) {
    let Some(path) = path else {
        values::erase(draft, EAP, key);
        return;
    };
    let mut value = CERTIFICATE_SCHEME.to_vec();
    value.extend_from_slice(path.as_os_str().as_encoded_bytes());
    value.push(0);
    values::put(draft, EAP, key, value);
}

fn read_ip(ip: &Properties, six: bool) -> IpProfile {
    IpProfile {
        method: IpMethod::read(ip.string_ref("method").unwrap_or_default()),
        addresses: crate::ip::addresses(ip, "address-data"),
        gateway: ip.string("gateway").unwrap_or_default(),
        nameservers: read_nameservers(ip, six),
        searches: ip.strings("dns-search").unwrap_or_default(),
        routes: crate::ip::routes(ip, "route-data"),
        ignore_automatic_dns: ip.boolean("ignore-auto-dns").unwrap_or(false),
        ignore_automatic_routes: ip.boolean("ignore-auto-routes").unwrap_or(false),
        never_default: ip.boolean("never-default").unwrap_or(false),
    }
}

fn read_nameservers(ip: &Properties, six: bool) -> Vec<String> {
    let modern = ip.strings("dns-data").unwrap_or_default();
    if !modern.is_empty() {
        return modern;
    }
    if six {
        return ip
            .list::<Vec<u8>>("dns")
            .unwrap_or_default()
            .iter()
            .filter_map(|address| values::address_of(address))
            .collect();
    }
    ip.list::<u32>("dns")
        .unwrap_or_default()
        .into_iter()
        .map(values::address_of_word)
        .collect()
}

pub(crate) fn write_ip(draft: &mut Draft, setting: &str, ip: &IpProfile, six: bool) {
    values::put(draft, setting, "method", ip.method.write().to_owned());
    values::put(draft, setting, "ignore-auto-dns", ip.ignore_automatic_dns);
    values::put(
        draft,
        setting,
        "ignore-auto-routes",
        ip.ignore_automatic_routes,
    );
    values::put(draft, setting, "never-default", ip.never_default);
    values::put(draft, setting, "dns-search", ip.searches.clone());
    values::put(draft, setting, "gateway", ip.gateway.clone());

    let addresses: Vec<std::collections::HashMap<String, Value<'static>>> = ip
        .addresses
        .iter()
        .map(|address| {
            std::collections::HashMap::from([
                ("address".to_owned(), Value::from(address.address.clone())),
                ("prefix".to_owned(), Value::from(address.prefix)),
            ])
        })
        .collect();
    values::erase(draft, setting, "addresses");
    values::put(draft, setting, "address-data", addresses);

    let routes: Vec<std::collections::HashMap<String, Value<'static>>> = ip
        .routes
        .iter()
        .map(|route| {
            let mut settings = std::collections::HashMap::from([
                ("dest".to_owned(), Value::from(route.destination.clone())),
                ("prefix".to_owned(), Value::from(route.prefix)),
            ]);
            if !route.next_hop.is_empty() {
                settings.insert("next-hop".to_owned(), Value::from(route.next_hop.clone()));
            }
            if let Some(metric) = route.metric {
                settings.insert("metric".to_owned(), Value::from(metric));
            }
            settings
        })
        .collect();
    values::erase(draft, setting, "routes");
    values::put(draft, setting, "route-data", routes);

    write_nameservers(draft, setting, &ip.nameservers, six);
}

fn write_nameservers(draft: &mut Draft, setting: &str, nameservers: &[String], six: bool) {
    values::erase(draft, setting, "dns-data");
    if six {
        let addresses: Vec<Vec<u8>> = nameservers
            .iter()
            .filter_map(|address| values::bytes_of_address(address))
            .collect();
        values::put(draft, setting, "dns", addresses);
        return;
    }
    let addresses: Vec<u32> = nameservers
        .iter()
        .filter_map(|address| values::word_of_address(address))
        .collect();
    values::put(draft, setting, "dns", addresses);
}

fn read_proxy(proxy: &Properties) -> ProxyProfile {
    ProxyProfile {
        method: ProxyMethod::read(proxy.string_ref("method").unwrap_or_default()),
        script_url: proxy.string("pac-url").unwrap_or_default(),
        browser_only: proxy.boolean("browser-only").unwrap_or(false),
    }
}

fn write_proxy(draft: &mut Draft, proxy: &ProxyProfile) {
    values::put(draft, PROXY, "method", proxy.method.write().to_owned());
    values::put(draft, PROXY, "pac-url", proxy.script_url.clone());
    values::put(draft, PROXY, "browser-only", proxy.browser_only);
}
