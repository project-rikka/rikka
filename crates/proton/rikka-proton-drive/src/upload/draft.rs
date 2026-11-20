use std::collections::HashMap;
use std::time::Duration;

use futures_util::future::try_join_all;
use jiff::Timestamp;
use rikka_proton_pgp::{PrivateKey, SessionKey};

use crate::api_errors;
use crate::attributes::ExtendedAttributes;
use crate::client::DriveClient;
use crate::crypto::{self, SealedThumbnail};
use crate::error::{Error, Result};
use crate::file::File;
use crate::folder::Folder;
use crate::identifier::{LinkId, RevisionId};
use crate::link::LinkState;
use crate::revision::RevisionState;
use crate::share_keys::ShareKeys;
use crate::thumbnail::ThumbnailType;
use crate::upload::block_stream::{BlockSealer, SealedChunk, SealedRevision};
use crate::upload::photo::PhotoDetails;
use crate::upload::resume::{ResumePoint, UploadedBlock};
use crate::upload::verifier::Verifier;
use crate::upload::wire::{
    BlockRequest, BlocksRequest, BlocksResponse, CommitRequest, CreateFileRequest,
    CreateFileResponse, CreateRevisionRequest, CreateRevisionResponse, PhotoCommit,
    ThumbnailRequest, VerificationResponse, VerifierToken,
};

const DISCARD_ATTEMPTS: u32 = 3;
const DISCARD_RETRY_DELAY: Duration = Duration::from_millis(250);

pub(super) enum Target<'a> {
    File { name: &'a str, media_type: &'a str },
    Revision(&'a File),
}

pub(super) struct Commit<'a> {
    pub(super) sealed: SealedRevision,
    pub(super) attributes: ExtendedAttributes,
    pub(super) modified_at: Option<Timestamp>,
    pub(super) checksum_verified: bool,
    pub(super) photo: Option<&'a PhotoDetails>,
}

#[derive(Clone, Copy)]
pub(super) struct DraftContext<'a> {
    pub(super) client: &'a DriveClient,
    pub(super) share: &'a ShareKeys,
    pub(super) parent: &'a Folder,
}

pub(super) struct Draft<'a> {
    client: &'a DriveClient,
    share: &'a ShareKeys,
    parent: &'a Folder,
    link: LinkId,
    revision: RevisionId,
    new_file: bool,
    node_key: PrivateKey,
    content_key: SessionKey,
    signer: PrivateKey,
    email: String,
    verifier: Verifier,
}

struct Author {
    signer: PrivateKey,
    email: String,
}

impl Author {
    fn for_share(client: &DriveClient, share: &ShareKeys) -> Result<Author> {
        let address = share.address_id();
        Ok(Author {
            signer: crypto::signer(client.keys(), address)?,
            email: crypto::email(client.keys(), address)?,
        })
    }
}

struct CreatedDraft {
    link: LinkId,
    revision: RevisionId,
    node_key: PrivateKey,
    content_key: SessionKey,
    is_new_file: bool,
}

impl<'a> Draft<'a> {
    pub(super) async fn create(context: DraftContext<'a>, target: Target<'_>) -> Result<Draft<'a>> {
        let author = Author::for_share(context.client, context.share)?;
        let created = match target {
            Target::File { name, media_type } => {
                create_file_draft(context, &author, name, media_type).await?
            }
            Target::Revision(file) => {
                create_revision_draft(context.client, context.share, file).await?
            }
        };

        Draft::with_verifier(context, created, author).await
    }

    pub(super) async fn resume(
        context: DraftContext<'a>,
        target: Target<'_>,
        point: &ResumePoint,
    ) -> Result<Option<(Draft<'a>, HashMap<u32, UploadedBlock>)>> {
        match Draft::try_resume(context, target, point).await {
            Err(error) if cannot_resume(&error) => Ok(None),
            result => result,
        }
    }

    async fn try_resume(
        context: DraftContext<'a>,
        target: Target<'_>,
        point: &ResumePoint,
    ) -> Result<Option<(Draft<'a>, HashMap<u32, UploadedBlock>)>> {
        let DraftContext {
            client,
            share,
            parent,
        } = context;
        let volume = share.volume_id();
        let (node_key, content_key, is_new_file) = match target {
            Target::File { .. } => {
                let link = client.link(volume, point.link()).await?;
                if link.state() != LinkState::Draft || link.parent_id() != Some(parent.id()) {
                    return Ok(None);
                }
                let file = client
                    .decrypt_link(link, parent)?
                    .into_file()
                    .ok_or_else(|| Error::NotAFile(point.link().clone()))?;
                (file.keys().key().clone(), file.content_key()?.clone(), true)
            }
            Target::Revision(file) => {
                if file.id() != point.link() {
                    return Ok(None);
                }
                (
                    file.keys().key().clone(),
                    file.content_key()?.clone(),
                    false,
                )
            }
        };

        let info = client
            .revision_info(volume, point.link(), point.revision())
            .await?;
        if info.state() != RevisionState::Draft {
            return Ok(None);
        }
        let hashes_by_index: HashMap<u32, &str> = info
            .blocks()
            .iter()
            .filter_map(|block| block.hash().map(|hash| (block.index(), hash)))
            .collect();
        let uploaded = point
            .blocks()
            .iter()
            .filter(|block| {
                hashes_by_index.get(&block.index()).copied()
                    == Some(rikka_proton_api::base64::encode(block.hash()).as_str())
            })
            .map(|block| (block.index(), *block))
            .collect();

        let author = Author::for_share(client, share)?;
        let created = CreatedDraft {
            link: point.link().clone(),
            revision: point.revision().clone(),
            node_key,
            content_key,
            is_new_file,
        };
        let draft = Draft::with_verifier(context, created, author).await?;
        Ok(Some((draft, uploaded)))
    }

    async fn with_verifier(
        context: DraftContext<'a>,
        created: CreatedDraft,
        author: Author,
    ) -> Result<Draft<'a>> {
        let DraftContext {
            client,
            share,
            parent,
        } = context;
        let response: VerificationResponse = client
            .api()
            .get(format!(
                "drive/v2/volumes/{}/links/{}/revisions/{}/verification",
                share.volume_id(),
                created.link,
                created.revision
            ))
            .send()
            .await?;
        Ok(Draft {
            client,
            share,
            parent,
            link: created.link,
            revision: created.revision,
            new_file: created.is_new_file,
            verifier: Verifier::open(client.pgp(), &created.node_key, &response)?,
            node_key: created.node_key,
            content_key: created.content_key,
            signer: author.signer,
            email: author.email,
        })
    }

    pub(super) fn link(&self) -> &LinkId {
        &self.link
    }

    pub(super) fn revision(&self) -> &RevisionId {
        &self.revision
    }
    pub(super) fn sealer(&self) -> BlockSealer {
        BlockSealer {
            pgp: *self.client.pgp(),
            node: self.node_key.public_key(),
            content: self.content_key.clone(),
            signer: self.signer.clone(),
            verifier: self.verifier.clone(),
        }
    }

    pub(super) fn seal_thumbnail(
        &self,
        kind: ThumbnailType,
        picture: &[u8],
    ) -> Result<(ThumbnailType, SealedThumbnail)> {
        let sealed =
            crypto::seal_thumbnail(self.client.pgp(), &self.content_key, &self.signer, picture)?;
        Ok((kind, sealed))
    }

    pub(super) async fn send(
        &self,
        batch: Vec<SealedChunk>,
        thumbnails: Vec<(ThumbnailType, SealedThumbnail)>,
    ) -> Result<Vec<(ThumbnailType, [u8; 32])>> {
        let response = self.request_blocks(&batch, &thumbnails).await?;
        let links = response.upload_links;

        let mut uploads = batch
            .into_iter()
            .enumerate()
            .map(|(offset, sealed)| {
                let index = sealed.index;
                let link = links
                    .iter()
                    .find(|link| link.index == Some(index))
                    .or_else(|| links.get(offset))
                    .ok_or(Error::NoUploadLink { index })?;

                Ok(self
                    .client
                    .write_block(link.address(), &link.token, sealed.block.data))
            })
            .collect::<Result<Vec<_>>>()?;

        let mut thumbnail_hashes = Vec::with_capacity(thumbnails.len());
        for (offset, (kind, sealed)) in thumbnails.into_iter().enumerate() {
            let link = response
                .thumbnail_links
                .iter()
                .find(|link| link.thumbnail_type == Some(kind))
                .or_else(|| response.thumbnail_links.get(offset))
                .ok_or(Error::Incomplete("thumbnail upload address"))?;
            uploads.push(
                self.client
                    .write_block(link.address(), &link.token, sealed.data),
            );
            thumbnail_hashes.push((kind, sealed.hash));
        }

        try_join_all(uploads).await?;
        Ok(thumbnail_hashes)
    }

    pub(super) async fn commit(&self, commit: Commit<'_>) -> Result<()> {
        let pgp = self.client.pgp();
        let Commit {
            sealed,
            attributes,
            modified_at,
            checksum_verified,
            photo,
        } = commit;
        let mut attributes = attributes
            .with_size(sealed.sizes.iter().sum())
            .with_block_sizes(sealed.sizes)
            .with_sha1(sealed.sha1.clone());
        if let Some(at) = modified_at {
            attributes = attributes.with_modified_at(at);
        }

        let photo = match photo {
            Some(details) => Some(PhotoCommit {
                capture_time: details
                    .capture_time()
                    .or(modified_at)
                    .unwrap_or(Timestamp::UNIX_EPOCH),
                content_hash: self.parent.hash(&sealed.sha1)?,
                main_photo_link_id: details.main_photo_id(),
                exif: None,
                tags: details.tag_list(),
            }),
            None => None,
        };

        let manifest_signature = crypto::sign_manifest(pgp, &self.signer, &sealed.manifest)?;
        let armored_attributes =
            crypto::encrypt_attributes(pgp, &self.node_key, &self.signer, &attributes)?;
        let committed = CommitRequest {
            manifest_signature: &manifest_signature,
            signature_address: &self.email,
            extended_attributes: &armored_attributes,
            checksum_verified,
            photo,
        };

        self.client
            .api()
            .put(format!(
                "drive/v2/volumes/{}/files/{}/revisions/{}",
                self.share.volume_id(),
                self.link,
                self.revision
            ))
            .json(&committed)
            .send_unit()
            .await?;

        Ok(())
    }

    pub(super) async fn fetch_file(self) -> Result<File> {
        self.client
            .node(self.share.volume_id(), &self.link, self.parent)
            .await?
            .into_file()
            .ok_or(Error::NotAFile(self.link))
    }

    pub(super) async fn discard(&self) {
        for attempt in 0..DISCARD_ATTEMPTS {
            match self.delete().await {
                Ok(()) => return,
                Err(error) if attempt + 1 == DISCARD_ATTEMPTS => {
                    tracing::warn!(link = %self.link, revision = %self.revision, %error, "failed to delete draft");
                }
                Err(_) => tokio::time::sleep(DISCARD_RETRY_DELAY * 2u32.pow(attempt)).await,
            }
        }
    }

    async fn delete(&self) -> Result<()> {
        match self.new_file {
            true => discard_file(self.client, self.share, &self.link).await,
            false => discard_revision(self.client, self.share, &self.link, &self.revision).await,
        }
    }

    async fn request_blocks(
        &self,
        blocks: &[SealedChunk],
        thumbnails: &[(ThumbnailType, SealedThumbnail)],
    ) -> Result<BlocksResponse> {
        let request = BlocksRequest {
            address_id: self.share.address_id(),
            volume_id: self.share.volume_id(),
            link_id: &self.link,
            revision_id: &self.revision,
            block_list: blocks
                .iter()
                .map(|sealed| BlockRequest {
                    index: sealed.index,
                    size: sealed.block.data.len() as u64,
                    hash: rikka_proton_api::base64::encode(sealed.block.hash),
                    encrypted_signature: &sealed.block.armored_signature,
                    verifier: VerifierToken {
                        token: rikka_proton_api::base64::encode(&sealed.verifier_token),
                    },
                })
                .collect(),
            thumbnail_list: thumbnails
                .iter()
                .map(|(kind, sealed)| ThumbnailRequest {
                    kind: (*kind).into(),
                    size: sealed.data.len() as u64,
                    hash: rikka_proton_api::base64::encode(sealed.hash),
                })
                .collect(),
        };

        Ok(self
            .client
            .api()
            .post("drive/blocks")
            .json(&request)
            .send()
            .await?)
    }
}

async fn create_file_draft(
    context: DraftContext<'_>,
    author: &Author,
    name: &str,
    media_type: &str,
) -> Result<CreatedDraft> {
    let DraftContext {
        client,
        share,
        parent,
    } = context;
    let signer = &author.signer;
    let pgp = client.pgp();
    let parent_key = parent.keys().key().public_key();

    let new_key = crypto::generate_key(pgp, &parent_key, signer)?;
    let armored_name = crypto::encrypt_name(pgp, &parent_key, None, signer, name)?;
    let (content_key, packet, packet_signature) = crypto::generate_content_key(pgp, &new_key.key)?;
    let hash = parent.hash(name)?;
    let request = CreateFileRequest {
        name: &armored_name,
        hash: &hash,
        media_type,
        parent_link_id: parent.id(),
        node_key: &new_key.armored_key,
        node_passphrase: &new_key.armored_passphrase,
        node_passphrase_signature: &new_key.armored_signature,
        content_key_packet: &packet,
        content_key_packet_signature: &packet_signature,
        signature_address: &author.email,
        client_uid: client.client_uid(),
    };
    let path = format!("drive/v2/volumes/{}/files", share.volume_id());

    let created: CreateFileResponse = match client.api().post(&path).json(&request).send().await {
        Ok(created) => created,
        Err(error) => {
            let conflict = api_errors::conflict(&error);
            match conflict {
                Some(conflict) if is_own_draft(client, conflict.draft_client.as_deref()) => {
                    discard_file(client, share, &conflict.link).await?;
                    client.api().post(&path).json(&request).send().await?
                }
                Some(conflict) => {
                    return Err(Error::Conflict {
                        link: conflict.link,
                        draft: conflict.draft_revision,
                    });
                }
                None => return Err(error.into()),
            }
        }
    };

    Ok(CreatedDraft {
        link: created.file.id,
        revision: created.file.revision_id,
        node_key: new_key.key,
        content_key,
        is_new_file: true,
    })
}

async fn create_revision_draft(
    client: &DriveClient,
    share: &ShareKeys,
    file: &File,
) -> Result<CreatedDraft> {
    let request = CreateRevisionRequest {
        current_revision_id: file.active_revision().map(|revision| revision.id()),
        client_uid: client.client_uid(),
    };
    let path = format!(
        "drive/v2/volumes/{}/files/{}/revisions",
        share.volume_id(),
        file.id()
    );

    let created: CreateRevisionResponse = match client.api().post(&path).json(&request).send().await
    {
        Ok(created) => created,
        Err(error) => match api_errors::conflict(&error) {
            Some(conflict) => match conflict.draft_revision {
                Some(draft) if is_own_draft(client, conflict.draft_client.as_deref()) => {
                    discard_revision(client, share, file.id(), &draft).await?;
                    client.api().post(&path).json(&request).send().await?
                }
                draft => {
                    return Err(Error::Conflict {
                        link: conflict.link,
                        draft,
                    });
                }
            },
            None => return Err(error.into()),
        },
    };

    Ok(CreatedDraft {
        link: file.id().clone(),
        revision: created.revision.id,
        node_key: file.keys().key().clone(),
        content_key: file.content_key()?.clone(),
        is_new_file: false,
    })
}

fn is_own_draft(client: &DriveClient, draft_client: Option<&str>) -> bool {
    matches!((client.client_uid(), draft_client), (Some(own), Some(theirs)) if own == theirs)
}

async fn discard_file(client: &DriveClient, share: &ShareKeys, link: &LinkId) -> Result<()> {
    let outcomes = client
        .delete_drafts(share.volume_id(), std::slice::from_ref(link))
        .await?;
    match outcomes.iter().find(|outcome| !outcome.is_ok()) {
        Some(refused) => Err(Error::Refused {
            code: refused.code(),
            message: refused.error().unwrap_or_default().to_owned(),
        }),
        None => Ok(()),
    }
}

async fn discard_revision(
    client: &DriveClient,
    share: &ShareKeys,
    link: &LinkId,
    revision: &RevisionId,
) -> Result<()> {
    client
        .api()
        .delete(format!(
            "drive/v2/volumes/{}/files/{link}/revisions/{revision}",
            share.volume_id()
        ))
        .send_unit()
        .await?;
    Ok(())
}

fn cannot_resume(error: &Error) -> bool {
    matches!(
        error,
        Error::LinkNotFound(_)
            | Error::NotAFile(_)
            | Error::NoRevision(_)
            | Error::Api(rikka_proton_api::Error::Api(_) | rikka_proton_api::Error::Status { .. })
    )
}
