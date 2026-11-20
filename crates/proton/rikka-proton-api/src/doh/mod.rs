mod dns;
mod providers;

use std::time::Duration;

use ::base64::Engine;
use ::base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::seq::IndexedRandom;
use rikka_net::Client;
use thiserror::Error;

use self::dns::DnsError;

const QUERY_TIMEOUT: Duration = Duration::from_secs(10);
const DNS_MESSAGE: &str = "application/dns-message";

#[derive(Debug, Error)]
enum QueryError {
    #[error(transparent)]
    Http(#[from] rikka_net::Error),
    #[error(transparent)]
    Dns(#[from] DnsError),
}

#[derive(Debug)]
pub(crate) struct Resolver {
    http: Client,
    providers: Option<Vec<String>>,
}

impl Resolver {
    pub(crate) fn new(http: Client, providers: Option<Vec<String>>) -> Resolver {
        Resolver { http, providers }
    }

    pub(crate) async fn resolve(&self, host: &str, session: Option<&str>) -> Vec<String> {
        let name = dns::alternative_name(host, session);
        let query = URL_SAFE_NO_PAD.encode(dns::txt_query(&name));

        let providers = match &self.providers {
            Some(providers) => providers.iter().map(String::as_str).collect(),
            None => default_providers(),
        };

        for provider in providers {
            match self.query(provider, &query).await {
                Ok(hosts) if !hosts.is_empty() => return hosts,
                Ok(_) => tracing::debug!(provider, %name, "no alternative route in DoH response"),
                Err(error) => tracing::debug!(provider, %error, "DoH query failed"),
            }
        }

        Vec::new()
    }

    async fn query(&self, provider: &str, query: &str) -> Result<Vec<String>, QueryError> {
        let response = self
            .http
            .get(provider.trim_end_matches('/'))
            .header("Accept", DNS_MESSAGE)
            .query(&[("dns", query)])
            .timeout(QUERY_TIMEOUT)
            .send()
            .await?
            .error_for_status()
            .await?
            .bytes()
            .await?;

        Ok(dns::txt_hostnames(&response)?)
    }
}

fn default_providers() -> Vec<&'static str> {
    let mut selected = providers::PRIMARY.to_vec();
    selected.extend(
        providers::SECONDARY
            .choose_multiple(&mut rand::rng(), providers::SECONDARY_SAMPLE_SIZE)
            .copied(),
    );
    selected
}
