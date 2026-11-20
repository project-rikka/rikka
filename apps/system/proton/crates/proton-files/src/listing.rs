use futures_util::stream::{self, BoxStream};
use futures_util::{Stream, StreamExt as _};
use rikka_proton_api::offload;
use rikka_proton_drive::{DriveClient, Folder, LinkId, Node, VolumeId};

use crate::drive_cache::DriveCache;
use crate::entry::Entry;
use crate::error::Error;
use crate::order::Order;
use crate::page::Page;

#[derive(Debug)]
pub enum Listed {
    Page(Page),
    Done,
    Failed(Error),
}

pub fn listing(
    client: &DriveClient,
    volume: &VolumeId,
    folder: &Folder,
    records: DriveCache,
    order: Order,
) -> impl Stream<Item = Listed> + Send + 'static {
    let reading = ListingState {
        pages: client.listing(volume, folder).pages().boxed(),
        at: folder.id().clone(),
        records,
        order,
        collected: Vec::new(),
    };

    stream::unfold(
        Some(reading),
        |reading| async move { reading?.step().await },
    )
}

struct ListingState {
    pages: BoxStream<'static, rikka_proton_drive::Result<Vec<Node>>>,
    at: LinkId,
    records: DriveCache,
    order: Order,
    collected: Vec<Entry>,
}

impl ListingState {
    async fn step(mut self) -> Option<(Listed, Option<ListingState>)> {
        match self.pages.next().await {
            Some(Ok(nodes)) => {
                let order = self.order;
                let (page, collected) = offload(move || {
                    let page = Page::from_nodes(nodes, order);
                    let collected = page.entries.clone();
                    (page, collected)
                })
                .await;
                self.collected.extend(collected);
                Some((Listed::Page(page), Some(self)))
            }
            Some(Err(failure)) => Some((Listed::Failed(failure.into()), None)),
            None => {
                let staged = self.records.stage_entries(&self.at, self.collected);
                offload(move || staged.write()).await;
                Some((Listed::Done, None))
            }
        }
    }
}
