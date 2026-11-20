use std::cmp::Ordering;
use std::collections::HashSet;

use crate::config::Protocol;
use crate::servers::features::{ServerFeatures, Tier};
use crate::servers::logicals::LogicalServer;
use crate::servers::scoring::country_code;

const MAX_FALLBACKS: usize = 5;

const MATCHES_FEATURES: u32 = 1;
const MATCHES_TIER: u32 = 1 << 1;
const MATCHES_GATEWAY: u32 = 1 << 2;
const MATCHES_CITY: u32 = 1 << 3;
const MATCHES_STATE: u32 = 1 << 4;
const MATCHES_COUNTRY: u32 = 1 << 5;
const MATCHES_SECURE_CORE: u32 = 1 << 6;

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerQuery {
    server: Option<String>,
    gateway: Option<String>,
    country: Option<String>,
    city: Option<String>,
    state: Option<String>,
    entry_country: Option<String>,
    excluded_country: Option<String>,
    secure_core: bool,
    features: ServerFeatures,
    protocol: Protocol,
}

impl Default for ServerQuery {
    fn default() -> ServerQuery {
        ServerQuery {
            server: None,
            gateway: None,
            country: None,
            city: None,
            state: None,
            entry_country: None,
            excluded_country: None,
            secure_core: false,
            features: ServerFeatures::default(),
            protocol: Protocol::WireGuardUdp,
        }
    }
}

impl ServerQuery {
    pub fn fastest() -> ServerQuery {
        ServerQuery::default()
    }

    pub fn server(id: impl Into<String>) -> ServerQuery {
        ServerQuery {
            server: Some(id.into()),
            ..ServerQuery::default()
        }
    }

    pub fn gateway(name: impl Into<String>) -> ServerQuery {
        ServerQuery {
            gateway: Some(name.into()),
            ..ServerQuery::default()
        }
    }

    pub fn country(mut self, country: impl AsRef<str>) -> ServerQuery {
        self.country = Some(country_code(country.as_ref()));
        self
    }

    pub fn city(mut self, city: impl Into<String>) -> ServerQuery {
        self.city = Some(city.into());
        self
    }

    pub fn state(mut self, state: impl Into<String>) -> ServerQuery {
        self.state = Some(state.into());
        self
    }

    pub fn secure_core(mut self, secure_core: bool) -> ServerQuery {
        self.secure_core = secure_core;
        self
    }

    pub fn entry_country(mut self, country: impl AsRef<str>) -> ServerQuery {
        self.entry_country = Some(country_code(country.as_ref()));
        self
    }

    pub fn excluding_country(mut self, country: impl AsRef<str>) -> ServerQuery {
        self.excluded_country = Some(country_code(country.as_ref()));
        self
    }

    pub fn features(mut self, features: ServerFeatures) -> ServerQuery {
        self.features = features;
        self
    }

    pub fn protocol(mut self, protocol: Protocol) -> ServerQuery {
        self.protocol = protocol;
        self
    }

    #[must_use]
    pub fn selected_protocol(&self) -> Protocol {
        self.protocol
    }

    #[must_use]
    pub fn matches(&self, server: &LogicalServer) -> bool {
        if let Some(id) = &self.server {
            return &server.id == id;
        }
        let exit = server.exit_country_code();

        self.in_scope(server, self.secure_core)
            && server.visible
            && server.features.contains(self.features)
            && self.country.as_ref().is_none_or(|country| &exit == country)
            && self.excluded_country.as_ref() != Some(&exit)
            && self
                .city
                .as_ref()
                .is_none_or(|city| server.city.as_ref() == Some(city))
            && self
                .state
                .as_ref()
                .is_none_or(|state| server.state.as_ref() == Some(state))
            && self
                .entry_country
                .as_ref()
                .is_none_or(|entry| &country_code(&server.entry_country) == entry)
    }

    #[must_use]
    pub fn best<'a>(&self, servers: &'a [LogicalServer], tier: Tier) -> Option<&'a LogicalServer> {
        let eligible: Vec<&LogicalServer> = by_score(servers)
            .into_iter()
            .filter(|server| self.matches(server) && server.supports(self.protocol))
            .collect();

        eligible
            .iter()
            .find(|server| tier.includes(server.tier) && server.is_online())
            .or_else(|| eligible.iter().find(|server| tier.includes(server.tier)))
            .or_else(|| eligible.first())
            .copied()
    }

    #[must_use]
    pub fn fallbacks<'a>(
        &self,
        servers: &'a [LogicalServer],
        tier: Tier,
        failed: &LogicalServer,
    ) -> Vec<&'a LogicalServer> {
        let failed_ips: HashSet<String> = failed
            .entries(self.protocol)
            .map(|entry| entry.ip().to_owned())
            .collect();
        let usable = |server: &&LogicalServer| {
            tier.includes(server.tier)
                && server.is_online()
                && server.visible
                && server.supports(self.protocol)
        };

        let mut scored: Vec<&LogicalServer> = by_score(servers)
            .into_iter()
            .filter(usable)
            .filter(|server| self.in_scope(server, self.secure_core))
            .filter(|server| failed.features.is_tor() || !server.features.is_tor())
            .filter(|server| {
                server
                    .entries(self.protocol)
                    .any(|entry| !failed_ips.contains(entry.ip()))
            })
            .collect();
        scored.sort_by_key(|server| std::cmp::Reverse(self.compatibility(server, tier, failed)));

        let mut selected: Vec<&LogicalServer> = Vec::new();
        let mut entries: HashSet<String> = HashSet::new();
        for server in &scored {
            if selected.len() == MAX_FALLBACKS {
                break;
            }
            let Some(domain) = server
                .entries(self.protocol)
                .find(|entry| !failed_ips.contains(entry.ip()))
                .map(|entry| entry.domain().to_owned())
            else {
                continue;
            };
            if entries.insert(domain) {
                selected.push(server);
            }
        }

        let mut extra: Vec<&LogicalServer> = Vec::new();
        let countries: HashSet<String> = selected
            .iter()
            .map(|server| server.exit_country_code())
            .collect();
        if countries.len() == 1
            && let Some(country) = countries.iter().next()
        {
            let cities: HashSet<Option<&String>> =
                selected.iter().map(|server| server.city.as_ref()).collect();
            if cities.len() == 1
                && let Some(city) = cities.iter().next()
                && let Some(other) = scored.iter().find(|server| {
                    &server.exit_country_code() == country && server.city.as_ref() != *city
                })
            {
                extra.push(other);
            }
            if let Some(abroad) = scored
                .iter()
                .find(|server| &server.exit_country_code() != country)
            {
                extra.push(abroad);
            }
        }
        if self.secure_core
            && self.gateway.is_none()
            && let Some(plain) = by_score(servers)
                .into_iter()
                .filter(usable)
                .filter(|server| self.in_scope(server, false))
                .max_by_key(|server| self.compatibility(server, tier, failed))
        {
            extra.push(plain);
        }

        selected.truncate(MAX_FALLBACKS.saturating_sub(extra.len()));
        selected.extend(extra);
        selected
    }

    fn in_scope(&self, server: &LogicalServer, secure_core: bool) -> bool {
        match &self.gateway {
            Some(name) => server.gateway_name.as_deref() == Some(name.as_str()),
            None => {
                !server.features.is_restricted() && server.features.is_secure_core() == secure_core
            }
        }
    }

    fn compatibility(&self, server: &LogicalServer, tier: Tier, failed: &LogicalServer) -> u32 {
        let exit = server.exit_country_code();
        let mut score = 0;
        let (country, city) = match self.server.is_some() || self.gateway.is_some() {
            true => (Some(failed.exit_country_code()), failed.city.clone()),
            false => (self.country.clone(), self.city.clone()),
        };

        if country.as_ref().is_none_or(|country| country == &exit) {
            score |= MATCHES_COUNTRY;
        }
        if city
            .as_ref()
            .is_some_and(|city| server.city.as_ref() == Some(city))
        {
            score |= MATCHES_CITY;
        }
        if self
            .state
            .as_ref()
            .is_some_and(|state| server.state.as_ref() == Some(state))
        {
            score |= MATCHES_STATE;
        }
        if self
            .gateway
            .as_ref()
            .is_some_and(|name| server.gateway_name.as_ref() == Some(name))
        {
            score |= MATCHES_GATEWAY;
        }
        if tier.includes(Tier::PLUS) == server.tier.includes(Tier::PLUS) {
            score |= MATCHES_TIER;
        }
        if server.features.contains(self.features) {
            score |= MATCHES_FEATURES;
        }
        if server.features.is_secure_core() == self.secure_core {
            score |= MATCHES_SECURE_CORE;
        }
        score
    }
}

fn by_score(servers: &[LogicalServer]) -> Vec<&LogicalServer> {
    let mut sorted: Vec<&LogicalServer> = servers.iter().collect();
    sorted.sort_by(|left, right| match (left.score, right.score) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });
    sorted
}
