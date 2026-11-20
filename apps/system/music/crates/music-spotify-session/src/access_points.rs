use std::time::Duration;

use serde::Deserialize;

const RESOLVER: &str = "https://apresolve.spotify.com/?type=accesspoint";
const FALLBACK: &str = "ap.spotify.com:443";
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AccessPoint {
    pub(crate) host: String,
    pub(crate) port: u16,
}

#[derive(Deserialize)]
struct Resolved {
    #[serde(default)]
    accesspoint: Vec<String>,
}

pub(crate) fn resolver() -> rikka_net::Client {
    rikka_net::Client::builder()
        .timeout(RESOLVE_TIMEOUT)
        .build()
        .unwrap_or_default()
}

pub(crate) async fn resolve(client: &rikka_net::Client) -> Vec<AccessPoint> {
    let mut points: Vec<AccessPoint> = match client.get_json::<Resolved>(RESOLVER).await {
        Ok(resolved) => resolved
            .accesspoint
            .iter()
            .filter_map(|address| parsed(address))
            .collect(),
        Err(error) => {
            tracing::warn!("failed to resolve Spotify access points, using {FALLBACK}: {error}");
            Vec::new()
        }
    };
    points.extend(parsed(FALLBACK));
    points
}

fn parsed(address: &str) -> Option<AccessPoint> {
    let (host, port) = address.rsplit_once(':')?;
    Some(AccessPoint {
        host: host.to_owned(),
        port: port.parse().ok()?,
    })
}
