use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rikka_net::blocking::Client;
use rikka_net::{ClientBuilder, Flow, StatusCode, Url};

use crate::cancel::Cancel;
use crate::error::{Error, Result};
use crate::pin::{self, Pin};
use crate::stage::Stage;

const RELEASE_BUCKET: &str = "https://nix-releases.s3.amazonaws.com/";
const RELEASES: &str = "https://releases.nixos.org/";
const CHANNELS: &str = "https://channels.nixos.org/";
const INDEX: &str = "packages.json.br";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) type Report = Arc<dyn Fn(Stage) + Send + Sync>;

pub(crate) fn client() -> Result<Client> {
    let client = ClientBuilder::new()
        .compression(false)
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()?;
    Ok(client.blocking())
}

#[derive(Clone, Debug)]
pub(crate) struct Release {
    name: String,
    url: String,
}

impl Release {
    pub(crate) fn locate(client: &Client, pin: &Pin, cancel: &Cancel) -> Result<Self> {
        if let (Some(release), Some(commit)) = (pin.release(), pin.commit()) {
            match find_release(client, pin.channel(), release, commit, cancel) {
                Ok(Some(bucket)) => return Ok(bucket),
                Ok(None) => {
                    tracing::info!(release, commit, "release not listed; using channel head");
                }
                Err(Error::Network(error)) if error.status() == Some(StatusCode::NOT_FOUND) => {
                    tracing::info!(%error, release, "no release bucket; using channel head");
                }
                Err(error) => return Err(error),
            }
        }
        channel_head(client, pin.channel(), cancel)
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn commit(&self) -> &str {
        commit_of(&self.name)
    }

    pub(crate) fn fetch(
        &self,
        client: &Client,
        into: &Path,
        cancel: &Cancel,
        report: &Report,
    ) -> Result<()> {
        let progress = Arc::clone(report);
        client
            .download(self.url.as_str(), into)
            .cancel_token(cancel.token())
            .on_progress(move |done| {
                progress(Stage::Downloading {
                    received: done.transferred,
                    total: done.total,
                });
                Flow::Continue
            })
            .spawn()
            .join()?;
        Ok(())
    }
}

fn find_release(
    client: &Client,
    channel: &str,
    release: &str,
    commit: &str,
    cancel: &Cancel,
) -> Result<Option<Release>> {
    let prefix = format!("nixos/{channel}/nixos-{release}");
    let mut continuation: Option<String> = None;
    loop {
        let mut query = vec![
            ("list-type", "2".to_owned()),
            ("delimiter", "/".to_owned()),
            ("prefix", prefix.clone()),
        ];
        if let Some(token) = continuation.take() {
            query.push(("continuation-token", token));
        }
        let page = client
            .get(RELEASE_BUCKET)
            .query(&query)
            .cancel_token(cancel.token())
            .send()?
            .error_for_status()?
            .text()?;
        let listing = Listing::read(&page)?;

        let matched_prefix = listing
            .prefixes
            .iter()
            .filter_map(|path| path.strip_suffix('/'))
            .find(|path| pin::same_commit(commit_of(path), commit));
        if let Some(path) = matched_prefix {
            return Ok(Some(Release {
                name: leaf(path).to_owned(),
                url: format!("{RELEASES}{path}/{INDEX}"),
            }));
        }
        match listing.next {
            Some(token) => continuation = Some(token),
            None => return Ok(None),
        }
    }
}

struct Listing {
    prefixes: Vec<String>,
    next: Option<String>,
}

impl Listing {
    fn read(xml: &str) -> Result<Self> {
        let root = rikka_xml::parse(xml).map_err(Error::Listing)?;
        let prefixes = root
            .descendants()
            .filter(|element| element.name() == "CommonPrefixes")
            .flat_map(|common| common.children_named("Prefix"))
            .map(|prefix| prefix.text().into_owned())
            .filter(|prefix| !prefix.is_empty())
            .collect();
        let next = root
            .descendants()
            .find(|element| element.name() == "NextContinuationToken")
            .map(|token| token.text().into_owned())
            .filter(|token| !token.is_empty());
        Ok(Self { prefixes, next })
    }
}

fn channel_head(client: &Client, channel: &str, cancel: &Cancel) -> Result<Release> {
    let response = client
        .head(format!("{CHANNELS}nixos-{channel}/{INDEX}"))
        .cancel_token(cancel.token())
        .send()?
        .error_for_status()?;
    let url: &Url = response.url();
    let name = url
        .path_segments()
        .and_then(|segments| segments.rev().nth(1))
        .filter(|name| pin::is_commit(commit_of(name)))
        .ok_or_else(|| Error::NoRelease(url.to_string()))?;
    Ok(Release {
        name: name.to_owned(),
        url: url.to_string(),
    })
}

fn leaf(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn commit_of(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}
