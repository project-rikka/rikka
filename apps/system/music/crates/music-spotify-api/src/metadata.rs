use std::collections::HashMap;

use futures_util::future::try_join_all;
use prost::Message;
use rikka_net::Method;

use crate::error::{Error, Result};
use crate::messages::extended_metadata::{
    BatchedEntityRequest, BatchedExtensionResponse, EntityRequest, ExtensionQuery,
};
use crate::messages::metadata::{Album, Artist, Track};
use crate::spclient::{Call, PROTOBUF, SpClient};

const PATH: &str = "/extended-metadata/v0/extended-metadata";
const BATCH: usize = 500;
const ARTIST_V4: i32 = 8;
const ALBUM_V4: i32 = 9;
const TRACK_V4: i32 = 10;

pub trait Entity: Message + Default {
    const KIND: i32;
}

impl Entity for Track {
    const KIND: i32 = TRACK_V4;
}

impl Entity for Album {
    const KIND: i32 = ALBUM_V4;
}

impl Entity for Artist {
    const KIND: i32 = ARTIST_V4;
}

impl SpClient {
    pub async fn entities<M: Entity>(&self, uris: &[String]) -> Result<HashMap<String, M>> {
        let answers =
            try_join_all(uris.chunks(BATCH).map(|batch| self.batch(batch, M::KIND))).await?;
        let mut by_uri = HashMap::with_capacity(uris.len());
        for entity in answers
            .into_iter()
            .flat_map(|answer| answer.extended_metadata)
            .flat_map(|array| array.extension_data)
        {
            let Some(data) = entity.extension_data else {
                continue;
            };
            match M::decode(data.value.as_slice()) {
                Ok(message) => {
                    by_uri.insert(entity.entity_uri, message);
                }
                Err(error) => {
                    tracing::debug!(
                        "skipping unreadable Spotify entity {}: {error}",
                        entity.entity_uri
                    );
                }
            }
        }
        Ok(by_uri)
    }

    async fn batch(&self, uris: &[String], kind: i32) -> Result<BatchedExtensionResponse> {
        let request = BatchedEntityRequest {
            entity_request: uris
                .iter()
                .map(|uri| EntityRequest {
                    entity_uri: uri.clone(),
                    query: vec![ExtensionQuery {
                        extension_kind: kind,
                    }],
                })
                .collect(),
        };
        let answer = self
            .call(Call {
                method: Method::POST,
                path: PATH,
                accept: Some(PROTOBUF),
                body: Some((PROTOBUF, request.encode_to_vec())),
            })
            .await?;
        BatchedExtensionResponse::decode(answer).map_err(Error::decode("extended metadata"))
    }
}
