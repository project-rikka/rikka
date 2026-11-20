use futures_util::future::{self, Either};
use rikka_proton_api::offload;

use crate::client::DriveClient;
use crate::error::{Error, Result};
use crate::folder::Folder;
use crate::identifier::{LinkId, ShareId, VolumeId};
use crate::node::Node;
use crate::share_keys::ShareKeys;

#[derive(Debug)]
pub struct OpenedRoot {
    share: ShareKeys,
    root: Folder,
    children: Vec<Node>,
}

impl OpenedRoot {
    #[must_use]
    pub fn share(&self) -> &ShareKeys {
        &self.share
    }

    #[must_use]
    pub fn root(&self) -> &Folder {
        &self.root
    }

    #[must_use]
    pub fn children(&self) -> &[Node] {
        &self.children
    }

    #[must_use]
    pub fn into_parts(self) -> (ShareKeys, Folder, Vec<Node>) {
        (self.share, self.root, self.children)
    }
}

impl DriveClient {
    pub async fn open_root(
        &self,
        volume: &VolumeId,
        share: &ShareId,
        root: &LinkId,
    ) -> Result<OpenedRoot> {
        let (root_share, root_link, first) = future::try_join3(
            self.share(share),
            self.link(volume, root),
            self.child_page(volume, root, None),
        )
        .await?;
        if root_share.root_id() != root || root_share.volume_id() != volume {
            return Err(Error::MovedRoot(share.clone()));
        }

        let client = self.clone();
        let (keys, folder) = offload(move || {
            let keys = client.decrypt_share(root_share)?;
            let folder = client
                .decrypt_link_with_key(root_link, keys.key())?
                .into_folder()
                .ok_or_else(|| Error::NotAFolder(keys.root_id().clone()))?;
            Ok::<_, Error>((keys, folder))
        })
        .await?;

        let rest = match first.next() {
            Some(anchor) => Either::Left(
                self.listing(volume, &folder)
                    .starting_after(anchor.clone())
                    .all(),
            ),
            None => Either::Right(future::ok(Vec::new())),
        };
        let opening = self.decrypt_page(volume, first.ids, folder.keys().key().clone());
        let (mut children, rest) = future::try_join(opening, rest).await?;
        children.extend(rest);

        Ok(OpenedRoot {
            share: keys,
            root: folder,
            children,
        })
    }
}
