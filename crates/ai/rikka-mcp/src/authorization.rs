use std::net::IpAddr;

use rikka_net::Url;
use rikka_oauth::{Challenge, Metadata, ProtectedResource};

const OFFLINE_ACCESS: &str = "offline_access";

const LEGACY_AUTHORIZATION: &str = "/authorize";
const LEGACY_TOKEN: &str = "/token";
const LEGACY_REGISTRATION: &str = "/register";

#[derive(Debug)]
pub(crate) struct Authority {
    metadata: Option<Metadata>,
    authorization_endpoint: Url,
    token_endpoint: Url,
    registration_endpoint: Option<Url>,
    resource: String,
    scope: Option<String>,
}

impl Authority {
    pub(crate) async fn discover(
        server: &str,
        challenge: Option<&str>,
    ) -> Result<Authority, String> {
        let address =
            Url::parse(server).map_err(|error| format!("{server} is not an address: {error}"))?;
        let challenge = challenge.map(Challenge::parse).unwrap_or_default();

        let pointed = challenge
            .resource_metadata()
            .and_then(|metadata| same_origin(&address, metadata));
        let described = match pointed {
            Some(metadata) => ProtectedResource::fetch(metadata.as_str(), server).await,
            None => Ok(None),
        };
        let described = match described.map_err(undescribed)? {
            Some(described) => Some(described),
            None => ProtectedResource::discover(server)
                .await
                .map_err(undescribed)?,
        };

        let metadata = match &described {
            Some(described) => {
                let issuer = described
                    .authorization_servers()
                    .iter()
                    .find(|issuer| is_reachable_from(&address, issuer))
                    .ok_or("the server names nobody to sign in with")?;
                let metadata = Metadata::discover_issuer(issuer.as_str())
                    .await
                    .map_err(|error| {
                        format!("{issuer} could not be asked how to sign in: {error}")
                    })?
                    .ok_or_else(|| format!("{issuer} does not say how to sign in"))?;
                Some(metadata)
            }
            None => Metadata::discover_issuer(&address.origin().ascii_serialization())
                .await
                .map_err(|error| {
                    format!("the server could not be asked how to sign in: {error}")
                })?,
        };
        if metadata
            .as_ref()
            .is_some_and(|metadata| !metadata.supports_s256())
        {
            return Err(String::from(
                "the sign-in offered does not protect the code it hands back",
            ));
        }

        let authorization_endpoint = match &metadata {
            Some(metadata) => metadata
                .authorization_endpoint()
                .cloned()
                .ok_or("the sign-in offered has no page to open")?,
            None => at(&address, LEGACY_AUTHORIZATION),
        };
        Ok(Authority {
            authorization_endpoint,
            token_endpoint: match &metadata {
                Some(metadata) => metadata.token_endpoint().clone(),
                None => at(&address, LEGACY_TOKEN),
            },
            registration_endpoint: match &metadata {
                Some(metadata) => metadata.registration_endpoint().cloned(),
                None => Some(at(&address, LEGACY_REGISTRATION)),
            },
            resource: match &described {
                Some(described) => described.resource().to_string(),
                None => server.to_owned(),
            },
            scope: scope(&challenge, described.as_ref(), metadata.as_ref()),
            metadata,
        })
    }

    pub(crate) fn authorization_endpoint(&self) -> &Url {
        &self.authorization_endpoint
    }

    pub(crate) fn token_endpoint(&self) -> &Url {
        &self.token_endpoint
    }

    pub(crate) fn registration_endpoint(&self) -> Option<&Url> {
        self.registration_endpoint.as_ref()
    }

    pub(crate) fn resource(&self) -> &str {
        &self.resource
    }

    pub(crate) fn scope(&self) -> Option<&str> {
        self.scope.as_deref()
    }

    pub(crate) fn supports_secret_basic(&self) -> bool {
        self.metadata
            .as_ref()
            .is_none_or(Metadata::supports_secret_basic)
    }

    pub(crate) fn check_response_issuer(&self, received: Option<&str>) -> rikka_oauth::Result<()> {
        match &self.metadata {
            Some(metadata) => metadata.check_response_issuer(received),
            None => Ok(()),
        }
    }
}

fn undescribed(error: rikka_oauth::Error) -> String {
    format!("the server could not be asked who signs people in: {error}")
}

fn scope(
    challenge: &Challenge,
    described: Option<&ProtectedResource>,
    metadata: Option<&Metadata>,
) -> Option<String> {
    let mut scopes: Vec<&str> = match challenge.scope() {
        Some(scope) => scope.split_whitespace().collect(),
        None => described
            .map(ProtectedResource::scopes_supported)
            .unwrap_or_default()
            .iter()
            .map(String::as_str)
            .collect(),
    };
    if scopes.is_empty() {
        return None;
    }

    let renewable = metadata.is_some_and(|metadata| {
        metadata
            .scopes_supported()
            .iter()
            .any(|scope| scope == OFFLINE_ACCESS)
    });
    if renewable && !scopes.contains(&OFFLINE_ACCESS) {
        scopes.push(OFFLINE_ACCESS);
    }
    Some(scopes.join(" "))
}

fn at(server: &Url, path: &str) -> Url {
    let mut endpoint = server.clone();
    endpoint.set_query(None);
    endpoint.set_fragment(None);
    endpoint.set_path(path);
    endpoint
}

fn same_origin(server: &Url, pointed: &str) -> Option<Url> {
    let pointed = Url::parse(pointed).or_else(|_| server.join(pointed)).ok()?;
    if pointed.origin() == server.origin() {
        return Some(pointed);
    }
    tracing::warn!("ignoring sign-in metadata at {pointed}, which is not on {server}'s origin");
    None
}

fn is_reachable_from(server: &Url, issuer: &Url) -> bool {
    matches!(issuer.scheme(), "http" | "https")
        && (!is_internal(issuer) || (is_loopback(server) && is_loopback(issuer)))
}

fn is_loopback(address: &Url) -> bool {
    match numeric(address) {
        Some(numeric) => numeric.is_loopback(),
        None => address
            .host_str()
            .map(|host| host.trim_end_matches('.').to_ascii_lowercase())
            .is_some_and(|host| host == "localhost" || host.ends_with(".localhost")),
    }
}

fn is_internal(address: &Url) -> bool {
    match numeric(address) {
        Some(IpAddr::V4(numeric)) => is_internal_v4(numeric),
        Some(IpAddr::V6(numeric)) => match numeric.to_ipv4_mapped() {
            Some(mapped) => is_internal_v4(mapped),
            None => {
                numeric.is_loopback()
                    || numeric.is_unspecified()
                    || numeric.is_multicast()
                    || numeric.is_unique_local()
                    || numeric.is_unicast_link_local()
            }
        },
        None => is_loopback(address),
    }
}

fn is_internal_v4(numeric: std::net::Ipv4Addr) -> bool {
    numeric.is_private()
        || numeric.is_loopback()
        || numeric.is_link_local()
        || numeric.is_broadcast()
        || numeric.is_unspecified()
        || numeric.is_multicast()
}

fn numeric(address: &Url) -> Option<IpAddr> {
    address
        .host_str()?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse()
        .ok()
}
