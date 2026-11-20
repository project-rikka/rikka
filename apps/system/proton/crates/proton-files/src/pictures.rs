use std::collections::HashMap;

use futures_util::stream::{self, BoxStream};
use futures_util::{Stream, StreamExt as _};
use rikka_proton_api::offload;
use rikka_proton_drive::{DriveClient, File, LinkId, ShareKeys};

use crate::error::{Error, Result};
use crate::thumbs::{Thumb, ThumbRequest};

const OPENING_AT_ONCE: usize = 4;

pub type Pictured = (ThumbRequest, Result<Option<Thumb>>);

pub fn pictures(
    client: &DriveClient,
    share: Option<&ShareKeys>,
    requests: Vec<(ThumbRequest, Option<File>)>,
) -> impl Stream<Item = Pictured> + Send + 'static {
    let mut unlisted = Vec::new();
    let mut listed = Vec::new();
    for (ask, file) in requests {
        match file {
            Some(file) => listed.push((ask, file)),
            None => unlisted.push((ask, Err(Error::Unlisted))),
        }
    }

    stream::iter(unlisted).chain(fetched(client, share, listed))
}

fn fetched(
    client: &DriveClient,
    share: Option<&ShareKeys>,
    listed: Vec<(ThumbRequest, File)>,
) -> BoxStream<'static, Pictured> {
    let Some(share) = share else {
        return stream::iter(
            listed
                .into_iter()
                .map(|(ask, _)| (ask, Err(Error::Unlisted))),
        )
        .boxed();
    };

    let mut asks: HashMap<LinkId, ThumbRequest> = HashMap::with_capacity(listed.len());
    let mut files = Vec::with_capacity(listed.len());
    for (ask, file) in listed {
        asks.insert(file.id().clone(), ask);
        files.push(file);
    }

    client
        .thumbnails(share.volume_id(), files)
        .filter_map(move |(id, read)| std::future::ready(asks.remove(&id).map(|ask| (ask, read))))
        .map(|(ask, read)| offload(move || opened(ask, read)))
        .buffer_unordered(OPENING_AT_ONCE)
        .boxed()
}

fn opened(ask: ThumbRequest, read: rikka_proton_drive::Result<Option<Vec<u8>>>) -> Pictured {
    match read {
        Ok(picture) => (ask, Ok(picture.and_then(Thumb::decode))),
        Err(failure) => (ask, Err(failure.into())),
    }
}
