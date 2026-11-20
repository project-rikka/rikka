use std::collections::{HashMap, HashSet};

use futures_util::future::{join, join_all};
use music_core::{Connection, Error, Id, PlaylistPage, Result, Service};

use crate::blend::{Blend, BlendId, Entry, EntryKey, Match, Mirror, now};
use crate::marker::{blend_name, description, mirror_name};
use crate::matching;

const RETRY: u64 = 7 * 24 * 60 * 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountError {
    pub service: Service,
    pub account: String,
    pub error: Error,
}

impl AccountError {
    pub(crate) fn new(link: &Connection, error: Error) -> AccountError {
        AccountError {
            service: link.service,
            account: link.account.name.clone(),
            error,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Remote {
    Remove(EntryKey),
    Insert {
        after: Option<EntryKey>,
        entry: Box<Entry>,
    },
    Rename(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Synced {
    id: BlendId,
    based_on: u64,
    remote: Vec<Remote>,
    matches: Vec<(EntryKey, &'static str, Match)>,
    mirrors: Vec<Mirror>,
    account_errors: Vec<AccountError>,
    gone: bool,
}

impl Synced {
    fn report(&mut self, link: &Connection, error: Error) {
        let account_error = AccountError::new(link, error);
        if !self.account_errors.iter().any(|known| {
            known.service == account_error.service && known.account == account_error.account
        }) {
            self.account_errors.push(account_error);
        }
    }

    #[must_use]
    pub fn id(&self) -> &BlendId {
        &self.id
    }

    #[must_use]
    pub fn account_errors(&self) -> &[AccountError] {
        &self.account_errors
    }

    #[must_use]
    pub fn is_gone(&self) -> bool {
        self.gone
    }
}

impl Blend {
    pub fn absorb(&mut self, synced: &Synced) {
        let untouched = self.edited == synced.based_on;
        for change in &synced.remote {
            match change {
                Remote::Remove(key) => self.entries.retain(|entry| entry.key != *key),
                Remote::Insert { after, entry } => {
                    if !self
                        .entries
                        .iter()
                        .any(|existing| existing.key == entry.key)
                    {
                        self.place(*after, (**entry).clone());
                    }
                }
                Remote::Rename(name) if untouched => name.clone_into(&mut self.name),
                Remote::Rename(_) => {}
            }
        }
        for (key, service, matched) in &synced.matches {
            if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == *key) {
                entry.matches.insert(service, matched.clone());
            }
        }
        for mirror in &synced.mirrors {
            self.upsert_mirror(mirror.clone());
        }
        if untouched && synced.account_errors.is_empty() {
            self.synced = self.edited;
        }
    }
}

enum Read {
    Unmirrored,
    Found(Box<PlaylistPage>),
    Vanished,
    Failed(Error),
}

struct Pushed {
    mirror: Mirror,
    refused: Vec<Id>,
}

pub async fn sync(blend: &Blend, links: &[Connection]) -> Synced {
    let mut working = blend.clone();
    let mut synced = Synced {
        id: blend.id.clone(),
        based_on: blend.edited,
        remote: Vec::new(),
        matches: Vec::new(),
        mirrors: Vec::new(),
        account_errors: Vec::new(),
        gone: false,
    };
    let (local, mirrored): (Vec<&Connection>, Vec<&Connection>) =
        links.iter().partition(|link| link.service.is_local());
    let read = join_all(mirrored.iter().map(|link| read(blend, link))).await;
    let mut ready: Vec<(&Connection, Option<PlaylistPage>)> = Vec::new();
    let (mut existed, mut vanished) = (0, 0);
    for (link, read) in mirrored.into_iter().zip(read) {
        match read {
            Read::Unmirrored => ready.push((link, None)),
            Read::Found(page) => {
                existed += 1;
                ready.push((link, Some(*page)));
            }
            Read::Vanished => {
                existed += 1;
                vanished += 1;
                ready.push((link, None));
            }
            Read::Failed(error) => synced.report(link, error),
        }
    }
    if existed > 0 && vanished == existed && blend.is_settled() && synced.account_errors.is_empty()
    {
        synced.gone = true;
        return synced;
    }

    for (link, page) in &ready {
        if let (Some(page), Some(mirror)) = (page, blend.mirror_of(link)) {
            pull(
                &mut working,
                link.service.slug,
                mirror,
                page,
                &mut synced.remote,
            );
        }
    }

    let mut matching = HashSet::new();
    let matchers: Vec<&Connection> = ready
        .iter()
        .map(|(link, _)| *link)
        .chain(local)
        .filter(|link| matching.insert(link.service.slug))
        .collect();
    let matches = join_all(matchers.iter().map(|link| matched(&working, link))).await;
    for (link, (matches, failure)) in matchers.iter().zip(matches) {
        for (key, decided) in matches {
            record_match(
                &mut working,
                key,
                link.service.slug,
                decided,
                &mut synced.matches,
            );
        }
        if let Some(error) = failure {
            synced.report(link, error);
        }
    }

    let pushed = join_all(
        ready
            .iter()
            .map(|(link, page)| push(&working, link, page.as_ref())),
    )
    .await;
    for ((link, _), pushed) in ready.iter().zip(pushed) {
        match pushed {
            Ok(pushed) => {
                let slug = link.service.slug;
                for refused in &pushed.refused {
                    let keys: Vec<EntryKey> = working
                        .entries
                        .iter()
                        .filter(|entry| {
                            entry.track.service.slug != slug && entry.id_in(slug) == Some(refused)
                        })
                        .map(|entry| entry.key)
                        .collect();
                    for key in keys {
                        let missing = Match::Missing { checked: now() };
                        record_match(&mut working, key, slug, missing, &mut synced.matches);
                    }
                }
                synced.mirrors.push(pushed.mirror);
            }
            Err(error) => synced.report(link, error),
        }
    }
    synced
}

async fn read(blend: &Blend, link: &Connection) -> Read {
    let Some(mirror) = blend.mirror_of(link) else {
        return Read::Unmirrored;
    };
    let (page, still_listed) = join(
        link.catalog.playlist(&mirror.playlist),
        link.catalog.keeps_playlist(&mirror.playlist),
    )
    .await;
    match (page, still_listed) {
        (Err(Error::Missing(_)), _) | (Ok(_), Ok(false)) => Read::Vanished,
        (Ok(page), Ok(true)) => Read::Found(Box::new(page)),
        (Err(error), _) | (_, Err(error)) => Read::Failed(error),
    }
}

fn is_whole(page: &PlaylistPage) -> bool {
    let read = page
        .tracks
        .iter()
        .filter(|track| !track.id.as_str().is_empty())
        .count();
    page.playlist.length <= read
}

pub(crate) fn listed(page: &PlaylistPage) -> Vec<Id> {
    page.tracks
        .iter()
        .map(|track| track.id.clone())
        .filter(|id| !id.as_str().is_empty())
        .collect()
}

fn pull(
    working: &mut Blend,
    service: &'static str,
    mirror: &Mirror,
    page: &PlaylistPage,
    remote: &mut Vec<Remote>,
) {
    let mut unclaimed: HashMap<&Id, usize> = HashMap::new();
    for id in &mirror.tracks {
        *unclaimed.entry(id).or_default() += 1;
    }
    let tracks: Vec<_> = page
        .tracks
        .iter()
        .filter(|track| !track.id.as_str().is_empty())
        .collect();
    let added: Vec<bool> = tracks
        .iter()
        .map(|track| match unclaimed.get_mut(&track.id) {
            Some(count) if *count > 0 => {
                *count -= 1;
                false
            }
            _ => true,
        })
        .collect();
    let whole = is_whole(page);
    for (id, count) in unclaimed {
        if count == 0 || !whole {
            continue;
        }
        let removed: Vec<EntryKey> = working
            .entries
            .iter()
            .filter(|entry| entry.id_in(service) == Some(id))
            .map(|entry| entry.key)
            .collect();
        for key in removed {
            working.entries.retain(|entry| entry.key != key);
            remote.push(Remote::Remove(key));
        }
    }
    for (at, track) in tracks.iter().enumerate() {
        if !added[at]
            || working
                .entries
                .iter()
                .any(|entry| entry.id_in(service) == Some(&track.id))
        {
            continue;
        }
        let after = tracks[..at].iter().rev().find_map(|before| {
            working
                .entries
                .iter()
                .find(|entry| entry.id_in(service) == Some(&before.id))
                .map(|entry| entry.key)
        });
        let entry = Entry::new((*track).clone());
        working.place(after, entry.clone());
        remote.push(Remote::Insert {
            after,
            entry: Box::new(entry),
        });
    }
    if page.playlist.name != mirror.name {
        let renamed = blend_name(&page.playlist.name);
        if !renamed.is_empty() && renamed != working.name {
            renamed.clone_into(&mut working.name);
            remote.push(Remote::Rename(renamed.to_owned()));
        }
    }
}

async fn matched(working: &Blend, link: &Connection) -> (Vec<(EntryKey, Match)>, Option<Error>) {
    let service = link.service.slug;
    let checked = now();
    let mut decided = Vec::new();
    for entry in &working.entries {
        let due = entry.track.service.slug != service
            && match entry.matches.get(service) {
                None => true,
                Some(Match::Missing { checked: when }) => checked.saturating_sub(*when) > RETRY,
                Some(Match::Found(_)) => false,
            };
        if !due {
            continue;
        }
        match matching::find(link.catalog.as_ref(), &entry.track).await {
            Ok(Some(track)) => decided.push((entry.key, Match::Found(track.id))),
            Ok(None) => decided.push((entry.key, Match::Missing { checked })),
            Err(error) => return (decided, Some(error)),
        }
    }
    (decided, None)
}

fn record_match(
    working: &mut Blend,
    key: EntryKey,
    service: &'static str,
    decided: Match,
    matches: &mut Vec<(EntryKey, &'static str, Match)>,
) {
    if let Some(entry) = working.entries.iter_mut().find(|entry| entry.key == key) {
        entry.matches.insert(service, decided.clone());
    }
    matches.retain(|(entry, slug, _)| *entry != key || *slug != service);
    matches.push((key, service, decided));
}

async fn push(working: &Blend, link: &Connection, page: Option<&PlaylistPage>) -> Result<Pushed> {
    let service = link.service.slug;
    let catalog = &link.catalog;
    let mut seen = HashSet::new();
    let desired: Vec<Id> = working
        .entries
        .iter()
        .filter_map(|entry| entry.id_in(service))
        .filter(|id| seen.insert(*id))
        .cloned()
        .collect();
    let name = mirror_name(&working.name);
    let (playlist, current, current_name, whole) = match (working.mirror_of(link), page) {
        (Some(mirror), Some(page)) => (
            mirror.playlist.clone(),
            listed(page),
            page.playlist.name.clone(),
            is_whole(page),
        ),
        _ => {
            let made = catalog
                .create_playlist(&name, &description(&working.id))
                .await?;
            (made.id, Vec::new(), made.name, true)
        }
    };
    if current_name != name {
        catalog.rename_playlist(&playlist, &name).await?;
    }
    let tracks = match whole && current == desired {
        true => current,
        false => {
            match whole && desired.starts_with(&current) {
                true => {
                    catalog
                        .add_to_playlist(&playlist, &desired[current.len()..])
                        .await?
                }
                false => catalog.arrange_playlist(&playlist, &desired).await?,
            }
            listed(&catalog.playlist(&playlist).await?)
        }
    };
    let track_ids: HashSet<&Id> = tracks.iter().collect();
    let refused = desired
        .iter()
        .filter(|id| !track_ids.contains(id))
        .cloned()
        .collect();
    Ok(Pushed {
        mirror: Mirror {
            service: link.service,
            account: link.account.id.clone(),
            playlist,
            name,
            tracks,
        },
        refused,
    })
}
