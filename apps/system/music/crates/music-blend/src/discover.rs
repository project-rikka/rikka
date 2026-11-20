use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use futures_util::future::join_all;
use music_core::{Connection, Error, Playlist, PlaylistPage, Result};

use crate::blend::{Blend, BlendId, Entry, EntryKey, Match, Mirror, now};
use crate::marker::{blend_name, looks_mirrored, marked};
use crate::matching;
use crate::sync::{AccountError, listed};

#[derive(Clone, Debug)]
pub struct StoreSnapshot {
    pub(crate) blends: Vec<Arc<Blend>>,
    pub(crate) deleted: BTreeSet<BlendId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Discovered {
    blends: Vec<Blend>,
    adopted: Vec<(BlendId, Mirror)>,
    account_errors: Vec<AccountError>,
}

impl Discovered {
    #[must_use]
    pub fn blends(&self) -> &[Blend] {
        &self.blends
    }

    #[must_use]
    pub fn adopted(&self) -> &[(BlendId, Mirror)] {
        &self.adopted
    }

    #[must_use]
    pub fn account_errors(&self) -> &[AccountError] {
        &self.account_errors
    }
}

impl Blend {
    pub fn adopt(&mut self, mirror: &Mirror) -> bool {
        if self.mirrors.iter().any(|existing| {
            existing.service.slug == mirror.service.slug && existing.account == mirror.account
        }) {
            return false;
        }
        self.mirrors.push(mirror.clone());
        true
    }
}

struct FoundMirror<'a> {
    link: &'a Connection,
    id: BlendId,
    playlist: Playlist,
    page: Option<PlaylistPage>,
}

pub async fn discover(
    snapshot: StoreSnapshot,
    links: Vec<Connection>,
    order: Vec<String>,
) -> Discovered {
    let links: Vec<Connection> = links
        .into_iter()
        .filter(|link| !link.service.is_local())
        .collect();
    let mut discovered = Discovered::default();
    let looked = join_all(links.iter().map(|link| look(link, &snapshot))).await;
    let mut found_mirrors = Vec::new();
    for (link, looked) in links.iter().zip(looked) {
        match looked {
            Ok((mirrors, failed)) => {
                found_mirrors.extend(mirrors);
                let account_errors = failed
                    .into_iter()
                    .map(|error| AccountError::new(link, error));
                discovered.account_errors.extend(account_errors);
            }
            Err(error) => discovered
                .account_errors
                .push(AccountError::new(link, error)),
        }
    }
    let mut unknown: BTreeMap<BlendId, Vec<(&Connection, PlaylistPage)>> = BTreeMap::new();
    for sighting in found_mirrors {
        let link = sighting.link;
        if snapshot.deleted.contains(&sighting.id) {
            if let Err(error) = link.catalog.delete_playlist(&sighting.playlist).await {
                discovered
                    .account_errors
                    .push(AccountError::new(link, error));
            }
            continue;
        }
        let known = snapshot.blends.iter().find(|blend| blend.id == sighting.id);
        if known.is_some_and(|blend| blend.mirror_of(link).is_some())
            || discovered
                .adopted
                .iter()
                .any(|(id, mirror)| *id == sighting.id && mirror.belongs_to(link))
        {
            continue;
        }
        let page = match sighting.page {
            Some(page) => page,
            None => match link.catalog.playlist(&sighting.playlist.id).await {
                Ok(page) => page,
                Err(error) => {
                    discovered
                        .account_errors
                        .push(AccountError::new(link, error));
                    continue;
                }
            },
        };
        match known {
            Some(blend) => {
                let mirror = adopted(blend, link, &page);
                discovered.adopted.push((sighting.id, mirror));
            }
            None => {
                let pages = unknown.entry(sighting.id).or_default();
                if !pages.iter().any(|(page_link, _)| {
                    page_link.service.slug == link.service.slug
                        && page_link.account.id == link.account.id
                }) {
                    pages.push((link, page));
                }
            }
        }
    }
    discovered.blends = unknown
        .into_iter()
        .map(|(id, pages)| rebuilt(id, pages, &order))
        .collect();
    discovered
}

async fn look<'a>(
    link: &'a Connection,
    snapshot: &StoreSnapshot,
) -> Result<(Vec<FoundMirror<'a>>, Vec<Error>)> {
    let listed = link.catalog.playlists().await?;
    let unknown = listed.iter().filter(|playlist| {
        playlist.editable
            && !snapshot
                .blends
                .iter()
                .any(|blend| blend.mirrors_playlist(link.service.slug, &playlist.id))
    });
    let mut mirrors = Vec::new();
    let mut unread = Vec::new();
    for playlist in unknown {
        match marked(&playlist.description) {
            Some(id) => mirrors.push(FoundMirror {
                link,
                id,
                playlist: playlist.clone(),
                page: None,
            }),
            None if looks_mirrored(&playlist.name) => unread.push(playlist),
            None => {}
        }
    }
    let read = join_all(
        unread
            .iter()
            .map(|playlist| link.catalog.playlist(&playlist.id)),
    )
    .await;
    let mut failed = Vec::new();
    for (playlist, read) in unread.into_iter().zip(read) {
        match read {
            Ok(page) => {
                if let Some(id) = marked(&page.playlist.description) {
                    mirrors.push(FoundMirror {
                        link,
                        id,
                        playlist: playlist.clone(),
                        page: Some(page),
                    });
                }
            }
            Err(error) => failed.push(error),
        }
    }
    Ok((mirrors, failed))
}

fn adopted(blend: &Blend, link: &Connection, page: &PlaylistPage) -> Mirror {
    let service = link.service.slug;
    let tracks = listed(page)
        .into_iter()
        .filter(|id| {
            blend
                .entries
                .iter()
                .any(|entry| entry.id_in(service) == Some(id))
        })
        .collect();
    Mirror {
        service: link.service,
        account: link.account.id.clone(),
        playlist: page.playlist.id.clone(),
        name: page.playlist.name.clone(),
        tracks,
    }
}

fn rebuilt(id: BlendId, mut pages: Vec<(&Connection, PlaylistPage)>, order: &[String]) -> Blend {
    let rank = |slug: &str| {
        order
            .iter()
            .position(|preferred| preferred == slug)
            .unwrap_or(order.len())
    };
    pages.sort_by_key(|(link, _)| rank(link.service.slug));
    let name = pages
        .first()
        .map(|(_, page)| blend_name(&page.playlist.name).to_owned())
        .unwrap_or_default();
    let mut blend = Blend {
        id,
        name,
        created: now(),
        entries: Vec::new(),
        mirrors: Vec::new(),
        edited: 1,
        synced: 0,
    };
    for (link, page) in pages {
        let service = link.service.slug;
        let mut last: Option<EntryKey> = None;
        for track in page
            .tracks
            .iter()
            .filter(|track| !track.id.as_str().is_empty())
        {
            if let Some(entry) = blend
                .entries
                .iter()
                .find(|entry| entry.id_in(service) == Some(&track.id))
            {
                last = Some(entry.key);
                continue;
            }
            let aligned = blend
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| {
                    entry.track.service.slug != service && !entry.matches.contains_key(service)
                })
                .filter_map(|(at, entry)| {
                    matching::score(&entry.track, track).map(|score| (score, at))
                })
                .max_by(|(left, _), (right, _)| left.total_cmp(right))
                .map(|(_, at)| &mut blend.entries[at]);
            match aligned {
                Some(entry) => {
                    entry
                        .matches
                        .insert(service, Match::Found(track.id.clone()));
                    last = Some(entry.key);
                }
                None => {
                    let entry = Entry::new(track.clone());
                    let key = entry.key;
                    blend.place(last, entry);
                    last = Some(key);
                }
            }
        }
        blend.mirrors.push(Mirror {
            service: link.service,
            account: link.account.id.clone(),
            playlist: page.playlist.id.clone(),
            name: page.playlist.name.clone(),
            tracks: listed(&page),
        });
    }
    blend
}
