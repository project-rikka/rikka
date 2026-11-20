use rikka_proton_api::offload;
use rikka_proton_drive::{DriveClient, Folder, ShareKeys, VolumeType};

use crate::drive_cache::{CachedRoot, DriveCache};
use crate::error::{Error, Result};
use crate::order::Order;
use crate::page::Page;

#[derive(Debug)]
pub struct Mounted {
    pub share: ShareKeys,
    pub root: Folder,
    pub listed: Option<Page>,
}

pub async fn mount(client: DriveClient, drive_cache: DriveCache, order: Order) -> Result<Mounted> {
    let cached_root = {
        let drive_cache = drive_cache.clone();
        offload(move || drive_cache.cached_root()).await
    };
    if let Some(cached) = cached_root
        && let Some(mounted) = open_cached_root(&client, &drive_cache, cached, order).await
    {
        return Ok(mounted);
    }

    let (share, root) = discover_root(&client).await?;
    let staged = drive_cache.stage_root(&share);
    offload(move || staged.write()).await;

    Ok(Mounted {
        share,
        root,
        listed: None,
    })
}

async fn open_cached_root(
    client: &DriveClient,
    drive_cache: &DriveCache,
    cached: CachedRoot,
    order: Order,
) -> Option<Mounted> {
    let opened = client
        .open_root(&cached.volume, &cached.share, &cached.root)
        .await
        .ok()?;
    let (share, root, nodes) = opened.into_parts();
    let (page, cached) = offload(move || {
        let page = Page::from_nodes(nodes, order);
        let cached = page.entries.clone();
        (page, cached)
    })
    .await;
    let staged = drive_cache.stage_entries(root.id(), cached);
    offload(move || staged.write()).await;

    Some(Mounted {
        share,
        root,
        listed: Some(page),
    })
}

async fn discover_root(client: &DriveClient) -> Result<(ShareKeys, Folder)> {
    let volumes = client.volumes().await?;
    let volume = volumes
        .iter()
        .find(|volume| volume.is_active() && volume.kind() == VolumeType::Regular)
        .or_else(|| volumes.iter().find(|volume| volume.is_active()))
        .ok_or(Error::NoVolume)?;

    let share = client.open_share(volume.share().id()).await?;
    let root = client.root(&share).await?;

    Ok((share, root))
}
