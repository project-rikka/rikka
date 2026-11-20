use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Instant;

use crate::reading;

const QUALITY_MAX: f32 = 70.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkKind {
    Ethernet,
    WiFi,
    Loopback,
    Virtual,
}

#[derive(Clone, Debug, Default)]
pub struct Link {
    pub mac: String,
    pub ipv4: Vec<Ipv4Addr>,
    pub ipv6: Vec<Ipv6Addr>,
}

#[derive(Clone, Debug)]
pub struct NetworkSample {
    pub name: String,
    pub kind: NetworkKind,
    pub up: bool,
    pub send: f32,
    pub receive: f32,
    pub speed_mbps: Option<u32>,
    pub signal: Option<f32>,
    pub link: Link,
}

#[derive(Clone, Debug)]
struct Identity {
    kind: NetworkKind,
    mac: String,
}

impl Identity {
    fn read(name: &str) -> Self {
        Self {
            kind: kind(name),
            mac: reading::text(format!("/sys/class/net/{name}/address")).unwrap_or_default(),
        }
    }
}

#[derive(Debug)]
pub struct Networks {
    previous: HashMap<String, (u64, u64)>,
    identities: HashMap<String, Identity>,
    read_at: Instant,
}

impl Default for Networks {
    fn default() -> Self {
        Self::new()
    }
}

impl Networks {
    #[must_use]
    pub fn new() -> Self {
        let mut probe = Self {
            previous: HashMap::new(),
            identities: HashMap::new(),
            read_at: Instant::now(),
        };
        probe.sample();
        probe
    }

    pub fn sample(&mut self) -> Vec<NetworkSample> {
        let seconds = self.read_at.elapsed().as_secs_f32();
        self.read_at = Instant::now();
        let addresses = addresses();
        let quality = wireless_quality();
        let dev = reading::text("/proc/net/dev").unwrap_or_default();

        let mut interfaces = Vec::new();
        let mut current = HashMap::new();
        for line in dev.lines().skip(2) {
            let Some((name, counters)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim().to_owned();
            let mut fields = reading::numbers(counters, 0);
            let (Some(received), Some(sent)) = (fields.next(), fields.nth(7)) else {
                continue;
            };
            current.insert(name.clone(), (received, sent));
            let identity = self
                .identities
                .entry(name.clone())
                .or_insert_with(|| Identity::read(&name));
            if identity.kind == NetworkKind::Loopback {
                continue;
            }
            let (before_received, before_sent) = self
                .previous
                .get(&name)
                .copied()
                .unwrap_or((received, sent));
            interfaces.push(NetworkSample {
                kind: identity.kind,
                up: is_up(&name),
                send: reading::rate(sent, before_sent, seconds),
                receive: reading::rate(received, before_received, seconds),
                speed_mbps: reading::number::<i64>(format!("/sys/class/net/{name}/speed"))
                    .filter(|speed| *speed > 0)
                    .map(|speed| speed as u32),
                signal: quality.get(&name).copied(),
                link: Link {
                    mac: identity.mac.clone(),
                    ipv4: addresses
                        .get(&name)
                        .map(|link| link.0.clone())
                        .unwrap_or_default(),
                    ipv6: addresses
                        .get(&name)
                        .map(|link| link.1.clone())
                        .unwrap_or_default(),
                },
                name,
            });
        }
        self.identities.retain(|name, _| current.contains_key(name));
        self.previous = current;
        interfaces.sort_by(|left, right| left.name.cmp(&right.name));
        interfaces
    }
}

fn is_up(name: &str) -> bool {
    match reading::text(format!("/sys/class/net/{name}/operstate")).as_deref() {
        Some("up") => true,
        Some("unknown") | None => {
            reading::number::<u8>(format!("/sys/class/net/{name}/carrier")) == Some(1)
        }
        Some(_) => false,
    }
}

fn kind(name: &str) -> NetworkKind {
    let path =
        |suffix: &str| std::path::Path::new(&format!("/sys/class/net/{name}/{suffix}")).exists();
    if reading::number::<u32>(format!("/sys/class/net/{name}/type")) == Some(772) {
        return NetworkKind::Loopback;
    }
    if path("wireless") || path("phy80211") {
        return NetworkKind::WiFi;
    }
    if path("device") {
        NetworkKind::Ethernet
    } else {
        NetworkKind::Virtual
    }
}

fn wireless_quality() -> HashMap<String, f32> {
    let Some(wireless) = reading::text("/proc/net/wireless") else {
        return HashMap::new();
    };
    wireless
        .lines()
        .skip(2)
        .filter_map(|line| {
            let (name, rest) = line.split_once(':')?;
            let quality: f32 = rest
                .split_whitespace()
                .nth(1)?
                .trim_end_matches('.')
                .parse()
                .ok()?;
            Some((
                name.trim().to_owned(),
                (quality / QUALITY_MAX).clamp(0.0, 1.0),
            ))
        })
        .collect()
}

fn addresses() -> HashMap<String, (Vec<Ipv4Addr>, Vec<Ipv6Addr>)> {
    let mut found: HashMap<String, (Vec<Ipv4Addr>, Vec<Ipv6Addr>)> = HashMap::new();
    for interface in rikka_interfaces::interface_addresses().unwrap_or_default() {
        let link = found.entry(interface.name).or_default();
        match interface.address {
            IpAddr::V4(address) => link.0.push(address),
            IpAddr::V6(address) => link.1.push(address),
        }
    }
    found
}
