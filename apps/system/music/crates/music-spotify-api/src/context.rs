use music_spotify_wire::SpotifyId;
use rikka_net::Method;
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::spclient::{Call, JSON, SpClient};

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Context {
    #[serde(default)]
    pub pages: Vec<ContextPage>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ContextPage {
    #[serde(default)]
    pub tracks: Vec<ContextTrack>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ContextTrack {
    #[serde(default)]
    pub uri: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Radio {
    #[serde(default)]
    media_items: Vec<Media>,
}

#[derive(Deserialize)]
struct Media {
    uri: String,
}

impl SpClient {
    pub async fn search(&self, query: &str) -> Result<Context> {
        let path = format!("/context-resolve/v1/spotify:search:{}", escaped(query));
        let answer = self.json(&path).await?;
        serde_json::from_slice(&answer).map_err(|_| Error::Malformed("search context"))
    }

    pub async fn radio(&self, track: SpotifyId) -> Result<Vec<String>> {
        let path = format!(
            "/inspiredby-mix/v2/seed_to_playlist/spotify:track:{track}?response-format=json"
        );
        let answer = self.json(&path).await?;
        let radio: Radio =
            serde_json::from_slice(&answer).map_err(|_| Error::Malformed("radio"))?;
        Ok(radio
            .media_items
            .into_iter()
            .map(|media| media.uri)
            .collect())
    }

    async fn json(&self, path: &str) -> Result<rikka_net::Bytes> {
        self.call(Call {
            method: Method::GET,
            path,
            accept: Some(JSON),
            body: None,
        })
        .await
    }
}

fn escaped(query: &str) -> String {
    query
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(byte).to_string()
            }
            b' ' => "+".to_owned(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}
