use music_spotify_wire::SpotifyId;
use prost::Message as _;
use rikka_net::Method;

use crate::error::{Error, Result};
use crate::messages::playlist::SelectedListContent;
use crate::spclient::{Call, SpClient};

const ROOTLIST_PAGE: usize = 300;
const DECORATION: &str = "revision,attributes,length,owner,capabilities,status_code";

impl SpClient {
    pub async fn rootlist(&self) -> Result<Vec<SelectedListContent>> {
        let mut parts = Vec::new();
        let mut from = 0;
        loop {
            let path = format!(
                "/playlist/v2/user/{}/rootlist?decorate={DECORATION}&from={from}&length={ROOTLIST_PAGE}",
                self.username()
            );
            let part = self.list(&path).await?;
            let (count, truncated) = part.contents.as_ref().map_or((0, false), |contents| {
                (contents.items.len(), contents.truncated)
            });
            parts.push(part);
            from += count;
            if count == 0 || !truncated {
                return Ok(parts);
            }
        }
    }

    pub async fn playlist(&self, playlist: SpotifyId) -> Result<SelectedListContent> {
        self.list(&format!("/playlist/v2/playlist/{playlist}"))
            .await
    }

    async fn list(&self, path: &str) -> Result<SelectedListContent> {
        let answer = self
            .call(Call {
                method: Method::GET,
                path,
                accept: None,
                body: None,
            })
            .await?;
        SelectedListContent::decode(answer).map_err(Error::decode("playlist"))
    }
}
