use std::time::{SystemTime, UNIX_EPOCH};

use music_spotify_wire::SpotifyId;
use prost::Message;
use rikka_net::{Bytes, Method};

use crate::error::{Error, Result};
use crate::messages::playlist::{
    Add, ChangeInfo, CreateListReply, Delta, Item, ItemAttributes, ListAttributes,
    ListAttributesPartialState, ListChanges, Op, OpList, Rem, SourceInfo, UpdateListAttributes,
};
use crate::spclient::{Call, PROTOBUF, SpClient};

const CREATE: &str = "/playlist/v2/playlist";
const ADD: i32 = 2;
const REM: i32 = 3;
const UPDATE_LIST_ATTRIBUTES: i32 = 6;
const WEBPLAYER: i32 = 5;

#[derive(Clone, Copy)]
enum Placement {
    First,
    Last,
}

impl SpClient {
    pub async fn create_playlist(&self, name: &str, description: &str) -> Result<String> {
        let made = OpList {
            ops: vec![attributing(ListAttributes {
                name: Some(name.to_owned()),
                description: (!description.is_empty()).then(|| description.to_owned()),
                ..ListAttributes::default()
            })],
        };
        let answer = self.post(CREATE, &made).await?;
        let uri = CreateListReply::decode(answer)
            .map_err(Error::decode("new playlist"))?
            .uri;
        let listed = changes(vec![adding(std::slice::from_ref(&uri), Placement::First)]);
        self.post(&self.rootlist_changes(), &listed).await?;
        Ok(uri)
    }

    pub async fn rename_playlist(&self, playlist: SpotifyId, name: &str) -> Result<()> {
        let renamed = changes(vec![attributing(ListAttributes {
            name: Some(name.to_owned()),
            ..ListAttributes::default()
        })]);
        self.post(&playlist_changes(playlist), &renamed)
            .await
            .map(drop)
    }

    pub async fn unfollow_playlist(&self, playlist_uri: &str) -> Result<()> {
        let removed = changes(vec![removing(&[playlist_uri.to_owned()])]);
        self.post(&self.rootlist_changes(), &removed)
            .await
            .map(drop)
    }

    pub async fn append_items(&self, playlist: SpotifyId, uris: &[String]) -> Result<()> {
        let added = changes(vec![adding(uris, Placement::Last)]);
        self.post(&playlist_changes(playlist), &added)
            .await
            .map(drop)
    }

    pub async fn remove_items(&self, playlist: SpotifyId, uris: &[String]) -> Result<()> {
        let removed = changes(vec![removing(uris)]);
        self.post(&playlist_changes(playlist), &removed)
            .await
            .map(drop)
    }

    pub async fn replace_items(
        &self,
        playlist: SpotifyId,
        current: &[String],
        replacement: &[String],
    ) -> Result<()> {
        let mut ops = Vec::new();
        if !current.is_empty() {
            ops.push(removing(current));
        }
        if !replacement.is_empty() {
            ops.push(adding(replacement, Placement::Last));
        }
        if ops.is_empty() {
            return Ok(());
        }
        self.post(&playlist_changes(playlist), &changes(ops))
            .await
            .map(drop)
    }

    fn rootlist_changes(&self) -> String {
        format!("/playlist/v2/user/{}/rootlist/changes", self.username())
    }

    async fn post(&self, path: &str, message: &impl Message) -> Result<Bytes> {
        self.call(Call {
            method: Method::POST,
            path,
            accept: Some(PROTOBUF),
            body: Some((PROTOBUF, message.encode_to_vec())),
        })
        .await
    }
}

fn playlist_changes(playlist: SpotifyId) -> String {
    format!("/playlist/v2/playlist/{playlist}/changes")
}

fn changes(ops: Vec<Op>) -> ListChanges {
    ListChanges {
        deltas: vec![Delta {
            ops,
            info: Some(ChangeInfo {
                source: Some(SourceInfo {
                    client: Some(WEBPLAYER),
                }),
            }),
        }],
    }
}

fn items(uris: &[String]) -> Vec<Item> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        });
    uris.iter()
        .map(|uri| Item {
            uri: uri.clone(),
            attributes: Some(ItemAttributes {
                timestamp: Some(timestamp),
            }),
        })
        .collect()
}

fn adding(uris: &[String], placement: Placement) -> Op {
    Op {
        kind: ADD,
        add: Some(Add {
            items: items(uris),
            add_last: matches!(placement, Placement::Last).then_some(true),
            add_first: matches!(placement, Placement::First).then_some(true),
        }),
        ..Op::default()
    }
}

fn removing(uris: &[String]) -> Op {
    Op {
        kind: REM,
        rem: Some(Rem {
            items: items(uris),
            items_as_key: Some(true),
        }),
        ..Op::default()
    }
}

fn attributing(values: ListAttributes) -> Op {
    Op {
        kind: UPDATE_LIST_ATTRIBUTES,
        update_list_attributes: Some(UpdateListAttributes {
            new_attributes: Some(ListAttributesPartialState {
                values: Some(values),
            }),
        }),
        ..Op::default()
    }
}
