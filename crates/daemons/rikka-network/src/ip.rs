use rikka_bus::Properties;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Address {
    pub address: String,
    pub prefix: u32,
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.address, self.prefix)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Route {
    pub destination: String,
    pub prefix: u32,
    pub next_hop: String,
    pub metric: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IpFacts {
    pub addresses: Vec<Address>,
    pub gateway: String,
    pub nameservers: Vec<String>,
    pub domains: Vec<String>,
    pub searches: Vec<String>,
    pub routes: Vec<Route>,
}

impl IpFacts {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.addresses.is_empty() && self.gateway.is_empty() && self.nameservers.is_empty()
    }
}

pub(crate) fn addresses(properties: &Properties, key: &str) -> Vec<Address> {
    properties
        .maps(key)
        .unwrap_or_default()
        .iter()
        .filter_map(|address| {
            Some(Address {
                address: address.string("address")?,
                prefix: address.uint32("prefix").unwrap_or(0),
            })
        })
        .collect()
}

pub(crate) fn routes(properties: &Properties, key: &str) -> Vec<Route> {
    properties
        .maps(key)
        .unwrap_or_default()
        .iter()
        .filter_map(|route| {
            Some(Route {
                destination: route.string("dest")?,
                prefix: route.uint32("prefix").unwrap_or(0),
                next_hop: route.string("next-hop").unwrap_or_default(),
                metric: route.uint32("metric").filter(|metric| *metric > 0),
            })
        })
        .collect()
}
