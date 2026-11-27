use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use google_account::{Grants, Service};
use prost::Message;
use rikka_net::{Client, StatusCode, Url, header};

use crate::error::{Error, Result};
use crate::proto::{ClientToServerMessage, ClientToServerResponse};

const GOOGLE: &str = "https://clients4.google.com/chrome-sync";
const COMMAND: &str = "command/";
const AGENT: &str = "Chrome ANDROID-PHONE 141.0.7390.122 \
    (b95610d5c4a562d9cd834bc0a098d3316e2f533f-refs/branch-heads/7390@{#2200}) channel(stable)";
const CLIENT: &str = "Google Chrome";
const CONTENT_TYPE: &str = "application/octet-stream";
const GZIP: &str = "gzip";

#[derive(Clone, Debug)]
pub struct Server {
    http: Client,
    command: Url,
}

impl Server {
    #[must_use]
    pub fn google(http: Client) -> Server {
        Server::new(http, GOOGLE).expect("the Google sync server is a URL")
    }

    pub fn new(http: Client, base: &str) -> Result<Server> {
        let base = format!("{}/", base.trim_end_matches('/'));
        let command = Url::parse(&base)
            .and_then(|base| base.join(COMMAND))
            .map_err(|_| Error::Protocol("invalid sync server address"))?;
        Ok(Server { http, command })
    }

    pub async fn exchange(
        &self,
        grants: &Grants,
        cache_guid: &str,
        message: &ClientToServerMessage,
    ) -> Result<ClientToServerResponse> {
        let mut url = self.command.clone();
        url.query_pairs_mut()
            .append_pair("client", CLIENT)
            .append_pair("client_id", cache_guid);
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&message.encode_to_vec())
            .map_err(Error::Compress)?;
        let body = encoder.finish().map_err(Error::Compress)?;
        let response = grants
            .send(Service::Sync, |token| {
                self.http
                    .post(url.clone())
                    .bearer_auth(token)
                    .header(header::USER_AGENT, AGENT)
                    .header(header::CONTENT_TYPE, CONTENT_TYPE)
                    .header(header::CONTENT_ENCODING, GZIP)
                    .body(body.clone())
            })
            .await?;
        match response.status() {
            status if status.is_success() => {
                Ok(ClientToServerResponse::decode(response.bytes().await?)?)
            }
            StatusCode::UNAUTHORIZED => Err(Error::Unauthorized),
            status => Err(Error::Status(status)),
        }
    }
}
