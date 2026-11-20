use std::time::{SystemTime, UNIX_EPOCH};

use prost::Message as _;
use rikka_net::Method;

use crate::error::{Error, Result};
use crate::messages::collection::{CollectionItem, PageRequest, PageResponse, WriteRequest};
use crate::spclient::{Call, SpClient};

const PAGING: &str = "/collection/v2/paging";
const WRITE: &str = "/collection/v2/write";
const CONTENT: &str = "application/vnd.collection-v2.spotify.proto";
const PAGE: i32 = 300;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub uri: String,
    pub added: i64,
}

impl SpClient {
    pub async fn saved(&self, set: &str) -> Result<Vec<Saved>> {
        let mut saved = Vec::new();
        let mut token = String::new();
        loop {
            let request = PageRequest {
                username: self.username().to_owned(),
                set: set.to_owned(),
                pagination_token: token,
                limit: PAGE,
            };
            let answer = self.collection(PAGING, request.encode_to_vec()).await?;
            let page = PageResponse::decode(answer).map_err(Error::decode("collection page"))?;
            saved.extend(
                page.items
                    .into_iter()
                    .filter(|item| !item.is_removed)
                    .map(|item| Saved {
                        uri: item.uri,
                        added: i64::from(item.added_at),
                    }),
            );
            if page.next_page_token.is_empty() {
                return Ok(saved);
            }
            token = page.next_page_token;
        }
    }

    pub async fn set_saved(&self, set: &str, uri: &str, saved: bool) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Malformed("system clock"))?;
        let added_at = match saved {
            true => i32::try_from(now.as_secs()).map_err(|_| Error::Malformed("system clock"))?,
            false => 0,
        };
        let request = WriteRequest {
            username: self.username().to_owned(),
            set: set.to_owned(),
            items: vec![CollectionItem {
                uri: uri.to_owned(),
                added_at,
                is_removed: !saved,
            }],
            client_update_id: format!("rikka-{}-{}", now.as_nanos(), std::process::id()),
        };
        self.collection(WRITE, request.encode_to_vec())
            .await
            .map(drop)
    }

    async fn collection(&self, path: &str, body: Vec<u8>) -> Result<rikka_net::Bytes> {
        self.call(Call {
            method: Method::POST,
            path,
            accept: Some(CONTENT),
            body: Some((CONTENT, body)),
        })
        .await
    }
}
