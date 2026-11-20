use std::collections::HashSet;

use rikka_proton_api::{Outcome, offload};
use serde::Serialize;

use crate::client::DriveClient;
use crate::crypto;
use crate::error::{Error, Result};
use crate::file::File;
use crate::folder::Folder;
use crate::identifier::{LinkId, VolumeId};
use crate::photos::albums::match_outcomes;
use crate::photos::library::PhotoLibrary;
use crate::photos::payload::{
    AlbumEntry, CopyRequest, FavoriteData, Payload, Sealing, TransferEntry,
};

const ENTRIES_PER_REQUEST: usize = 10;
const MISSING: &str = "Missing";

#[derive(Debug, Clone, Copy)]
enum Destination<'a> {
    Album(&'a Folder),
    Timeline,
}

struct Queued {
    photo: File,
    extra: Vec<LinkId>,
}

impl DriveClient {
    pub async fn add_to_album(
        &self,
        library: &PhotoLibrary,
        album: &Folder,
        from: &Folder,
        photos: &[File],
    ) -> Result<Vec<Outcome<LinkId>>> {
        self.transfer(library, Destination::Album(album), from, photos)
            .await
    }

    pub async fn save_to_timeline(
        &self,
        library: &PhotoLibrary,
        from: &Folder,
        photos: &[File],
    ) -> Result<Vec<Outcome<LinkId>>> {
        self.transfer(library, Destination::Timeline, from, photos)
            .await
    }

    pub async fn copy_photo(
        &self,
        library: &PhotoLibrary,
        target: &Folder,
        from: &Folder,
        photo: &File,
    ) -> Result<LinkId> {
        let queued = [Queued {
            photo: photo.clone(),
            extra: Vec::new(),
        }];
        let payload = self
            .seal_payloads(library, target, from, &queued)
            .await?
            .pop()
            .ok_or(Error::Incomplete("photo payload"))?;
        let email = crypto::email(self.keys(), library.share().address_id())?;

        self.copy_to_volume(library, target, &payload, &email).await
    }

    pub async fn favorite_photo(
        &self,
        library: &PhotoLibrary,
        from: &Folder,
        photo: &File,
    ) -> Result<()> {
        let volume = photo_volume(library, photo);
        let in_timeline =
            volume == *library.volume_id() && photo.parent_id() == Some(library.root().id());
        let payload = match in_timeline {
            true => None,
            false => {
                let queued = [Queued {
                    photo: photo.clone(),
                    extra: Vec::new(),
                }];
                self.seal_payloads(library, library.root(), from, &queued)
                    .await?
                    .pop()
            }
        };
        let email = crypto::email(self.keys(), library.share().address_id())?;

        self.api()
            .post(format!(
                "drive/photos/volumes/{volume}/links/{}/favorite",
                photo.id()
            ))
            .json(&FavoriteRequest {
                photo_data: payload
                    .as_ref()
                    .map(|payload| FavoriteData::new(payload, &email)),
            })
            .send_unit()
            .await?;

        Ok(())
    }

    async fn transfer(
        &self,
        library: &PhotoLibrary,
        destination: Destination<'_>,
        from: &Folder,
        photos: &[File],
    ) -> Result<Vec<Outcome<LinkId>>> {
        let target = match destination {
            Destination::Album(album) => album,
            Destination::Timeline => library.root(),
        };
        let email = crypto::email(self.keys(), library.share().address_id())?;
        let mut queue: Vec<Queued> = photos
            .iter()
            .map(|photo| Queued {
                photo: photo.clone(),
                extra: Vec::new(),
            })
            .collect();
        let mut retried = HashSet::new();
        let mut outcomes = Vec::with_capacity(photos.len());

        while !queue.is_empty() {
            let pending = std::mem::take(&mut queue);
            let payloads = self.seal_payloads(library, target, from, &pending).await?;
            let (same, cross): (Vec<Payload>, Vec<Payload>) = payloads
                .into_iter()
                .partition(|payload| payload.volume == *library.volume_id());

            let mut finished = Vec::new();
            for batch in payload_batches(&same) {
                let response = self
                    .transfer_within_volume(library, destination, &email, &batch)
                    .await?;
                finished.extend(match_outcomes(
                    batch.iter().map(|payload| &payload.main.link_id),
                    response,
                ));
            }
            for payload in &cross {
                let outcome = match self.copy_to_volume(library, target, payload, &email).await {
                    Ok(_) => Outcome::succeeded(payload.main.link_id.clone()),
                    Err(Error::Api(error)) => {
                        Outcome::refused(payload.main.link_id.clone(), &error)
                            .ok_or(Error::Api(error))?
                    }
                    Err(error) => return Err(error),
                };
                finished.push(outcome);
            }

            for outcome in finished {
                let missing = outcome.detail::<Vec<LinkId>>(MISSING).unwrap_or_default();
                let retry = match (outcome.id(), missing.is_empty()) {
                    (Some(id), false) if retried.insert(id.clone()) => pending
                        .iter()
                        .find(|queued| queued.photo.id() == id)
                        .map(|queued| Queued {
                            photo: queued.photo.clone(),
                            extra: missing,
                        }),
                    _ => None,
                };
                match retry {
                    Some(queued) => queue.push(queued),
                    None => outcomes.push(outcome),
                }
            }
        }

        Ok(outcomes)
    }

    async fn transfer_within_volume(
        &self,
        library: &PhotoLibrary,
        destination: Destination<'_>,
        email: &str,
        batch: &[&Payload],
    ) -> Result<Vec<Outcome<LinkId>>> {
        let volume = library.volume_id();
        let outcomes: Vec<Outcome<LinkId>> = match destination {
            Destination::Album(album) => {
                let entries: Vec<AlbumEntry<'_>> = batch
                    .iter()
                    .flat_map(|payload| payload.all())
                    .map(|sealed| AlbumEntry::new(sealed, email))
                    .collect();
                self.api()
                    .post(format!(
                        "drive/photos/volumes/{volume}/albums/{}/add-multiple",
                        album.id()
                    ))
                    .json(&AddRequest {
                        album_data: &entries,
                    })
                    .send_field_or_default("Responses")
                    .await?
            }
            Destination::Timeline => {
                let links: Vec<TransferEntry<'_>> = batch
                    .iter()
                    .flat_map(|payload| payload.all())
                    .map(TransferEntry::from)
                    .collect();
                self.api()
                    .put(format!(
                        "drive/photos/volumes/{volume}/links/transfer-multiple"
                    ))
                    .json(&TransferRequest {
                        parent_link_id: library.root().id(),
                        links: &links,
                        name_signature_email: email,
                        signature_email: None,
                    })
                    .send_field_or_default("Responses")
                    .await?
            }
        };

        Ok(outcomes)
    }

    async fn copy_to_volume(
        &self,
        library: &PhotoLibrary,
        target: &Folder,
        payload: &Payload,
        email: &str,
    ) -> Result<LinkId> {
        Ok(self
            .api()
            .post(format!(
                "drive/volumes/{}/links/{}/copy",
                payload.volume, payload.main.link_id
            ))
            .json(&CopyRequest::new(
                payload,
                library.volume_id(),
                target.id(),
                email,
            ))
            .send_field("LinkID")
            .await?)
    }

    async fn seal_payloads(
        &self,
        library: &PhotoLibrary,
        target: &Folder,
        from: &Folder,
        queued: &[Queued],
    ) -> Result<Vec<Payload>> {
        let mut collected = Vec::with_capacity(queued.len());
        for item in queued {
            let volume = photo_volume(library, &item.photo);
            let mut related: Vec<LinkId> = item
                .photo
                .photo()
                .map(|photo| photo.related_ids().to_vec())
                .unwrap_or_default();
            related.extend(item.extra.iter().cloned());
            related.retain(|id| id != item.photo.id());
            related.sort();
            related.dedup();

            let related = match related.is_empty() {
                true => Vec::new(),
                false => {
                    let links = self.links(&volume, &related).await?;
                    self.decrypt_links(links, from)
                        .await?
                        .into_iter()
                        .filter_map(|node| node.into_file())
                        .collect()
                }
            };
            collected.push((volume, item.photo.clone(), related));
        }

        let signer = crypto::signer(self.keys(), library.share().address_id())?;
        let target = target.clone();
        let pgp = *self.pgp();
        offload(move || {
            let sealing = Sealing {
                pgp,
                target: &target,
                signer: &signer,
            };
            collected
                .into_iter()
                .map(|(volume, photo, related)| {
                    Ok(Payload {
                        volume,
                        main: sealing.sealed(&photo)?,
                        related: related
                            .iter()
                            .map(|file| sealing.sealed(file))
                            .collect::<Result<Vec<_>>>()?,
                    })
                })
                .collect()
        })
        .await
    }
}

fn photo_volume(library: &PhotoLibrary, photo: &File) -> VolumeId {
    photo
        .link()
        .volume_id()
        .cloned()
        .unwrap_or_else(|| library.volume_id().clone())
}

fn payload_batches(payloads: &[Payload]) -> Vec<Vec<&Payload>> {
    let mut collected: Vec<Vec<&Payload>> = Vec::new();
    let mut size = 0;
    for payload in payloads {
        match collected.last_mut() {
            Some(batch) if size + payload.size() <= ENTRIES_PER_REQUEST => {
                batch.push(payload);
                size += payload.size();
            }
            _ => {
                collected.push(vec![payload]);
                size = payload.size();
            }
        }
    }
    collected
}

#[derive(Debug, Serialize)]
struct AddRequest<'a> {
    #[serde(rename = "AlbumData")]
    album_data: &'a [AlbumEntry<'a>],
}

#[derive(Debug, Serialize)]
struct TransferRequest<'a> {
    #[serde(rename = "ParentLinkID")]
    parent_link_id: &'a LinkId,
    #[serde(rename = "Links")]
    links: &'a [TransferEntry<'a>],
    #[serde(rename = "NameSignatureEmail")]
    name_signature_email: &'a str,
    #[serde(rename = "SignatureEmail")]
    signature_email: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct FavoriteRequest<'a> {
    #[serde(rename = "PhotoData")]
    photo_data: Option<FavoriteData<'a>>,
}
