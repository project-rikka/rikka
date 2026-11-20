use std::path::{Path, PathBuf};

use rikka_proton_api::offload;
use rikka_proton_drive::{DriveClient, File, Folder, LinkId, Node, ShareKeys, VolumeId};

use crate::entry::Entry;
use crate::error::Result;
use crate::viewing::private_dir;

pub async fn rename(
    client: &DriveClient,
    share: &ShareKeys,
    parent: &Folder,
    id: &LinkId,
    name: &str,
) -> Result<()> {
    let node = client.node(share.volume_id(), id, parent).await?;
    client.rename(share, parent, &node, name).await?;
    Ok(())
}

pub async fn move_nodes(
    client: &DriveClient,
    share: &ShareKeys,
    parent: &Folder,
    id: &LinkId,
    into: &Folder,
) -> Result<()> {
    let node = client.node(share.volume_id(), id, parent).await?;
    client.move_node(share, &node, into).await?;
    Ok(())
}

pub async fn open_viewer(
    client: &DriveClient,
    volume: &VolumeId,
    file: &File,
    progress: impl FnMut(u64) + Send,
) -> Result<PathBuf> {
    let directory = offload(private_dir).await?;
    Ok(client
        .download(volume, file)
        .save_into(&directory, progress)
        .await?)
}

pub async fn upload(
    client: &DriveClient,
    share: &ShareKeys,
    parent: &Folder,
    path: &Path,
    preview: Option<Vec<u8>>,
) -> Result<Entry> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_owned());

    let mut upload = client.upload(share, parent, name);
    if let Some(preview) = preview {
        upload = upload.thumbnail(preview);
    }

    let file = upload.upload_file(path).await?;

    Ok(Entry::from_node(&Node::File(file)))
}
