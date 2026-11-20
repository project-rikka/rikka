use futures_util::stream::BoxStream;
use futures_util::{Stream, StreamExt as _, TryStreamExt as _, stream};
use rikka_proton_pgp::PrivateKey;

use crate::client::DriveClient;
use crate::error::Result;
use crate::folder::Folder;
use crate::identifier::{LinkId, VolumeId};
use crate::link::{Link, LinkType};
use crate::node::Node;

const CONCURRENT_PAGES: usize = 2;

impl DriveClient {
    pub fn listing(&self, volume: &VolumeId, folder: &Folder) -> Listing {
        Listing {
            client: self.clone(),
            volume: volume.clone(),
            folder: folder.id().clone(),
            key: folder.keys().key().clone(),
            start: Start::First,
        }
    }

    pub(crate) async fn decrypt_page(
        &self,
        volume: &VolumeId,
        ids: Vec<LinkId>,
        key: PrivateKey,
    ) -> Result<Vec<Node>> {
        let links = self.links(volume, &ids).await?;
        let listed = links.into_iter().filter(is_listed).collect();

        self.decrypt_links_with_key(listed, key).await
    }
}

#[derive(Debug, Clone)]
enum Start {
    First,
    After(LinkId),
}

#[must_use]
#[derive(Debug, Clone)]
pub struct Listing {
    client: DriveClient,
    volume: VolumeId,
    folder: LinkId,
    key: PrivateKey,
    start: Start,
}

impl Listing {
    pub fn pages(self) -> impl Stream<Item = Result<Vec<Node>>> + Send + 'static {
        let Listing {
            client,
            volume,
            folder,
            key,
            start,
        } = self;
        let decrypt_client = client.clone();
        let listed_volume = volume.clone();

        child_pages(client, volume, folder, start)
            .map(move |ids| {
                let (decrypt_client, volume, key) =
                    (decrypt_client.clone(), listed_volume.clone(), key.clone());
                async move { decrypt_client.decrypt_page(&volume, ids?, key).await }
            })
            .buffered(CONCURRENT_PAGES)
    }

    pub async fn all(self) -> Result<Vec<Node>> {
        self.pages().try_concat().await
    }

    pub(crate) fn starting_after(mut self, anchor: LinkId) -> Listing {
        self.start = Start::After(anchor);
        self
    }
}

pub(crate) fn is_listed(link: &Link) -> bool {
    link.kind() != LinkType::File || link.active_revision().is_some()
}

struct ChildPageWalk {
    pages: BoxStream<'static, Result<Vec<LinkId>>>,
    collected: Option<Vec<LinkId>>,
}

fn child_pages(
    client: DriveClient,
    volume: VolumeId,
    folder: LinkId,
    start: Start,
) -> impl Stream<Item = Result<Vec<LinkId>>> + Send + 'static {
    let (after, collected) = match start {
        Start::First => (None, Some(Vec::new())),
        Start::After(anchor) => (Some(anchor), None),
    };
    let first = ChildPageWalk {
        pages: client.child_id_pages(&volume, &folder, after).boxed(),
        collected,
    };

    stream::unfold(Some(first), move |walk| {
        let (client, folder) = (client.clone(), folder.clone());
        async move {
            let mut walk = walk?;
            match walk.pages.next().await {
                Some(Ok(ids)) => {
                    if let Some(collected) = &mut walk.collected {
                        collected.extend(ids.iter().cloned());
                    }
                    Some((Ok(ids), Some(walk)))
                }
                Some(Err(error)) => Some((Err(error), None)),
                None => {
                    if let Some(collected) = walk.collected {
                        client.cache_children(&folder, &collected);
                    }
                    None
                }
            }
        }
    })
}
