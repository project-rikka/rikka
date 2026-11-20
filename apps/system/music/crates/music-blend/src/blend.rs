use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use music_core::{Artwork, Connection, Id, Service, Track};
use rand::Rng;

const UUID_LENGTH: usize = 36;
const HYPHENS: [usize; 4] = [8, 13, 18, 23];
const VERSION_AT: usize = 14;
const VARIANT_AT: usize = 19;

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlendId(Arc<str>);

impl BlendId {
    #[must_use]
    pub fn generate() -> BlendId {
        let mut bytes: [u8; 16] = rand::rng().random();
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let hex = rikka_types::hex::encode(bytes);
        BlendId(
            format!(
                "{}-{}-{}-{}-{}",
                &hex[..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..]
            )
            .into(),
        )
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<BlendId> {
        let text = text.trim().to_ascii_lowercase();
        let bytes = text.as_bytes();
        let shaped = bytes.len() == UUID_LENGTH
            && bytes
                .iter()
                .enumerate()
                .all(|(at, byte)| match HYPHENS.contains(&at) {
                    true => *byte == b'-',
                    false => byte.is_ascii_hexdigit(),
                })
            && bytes[VERSION_AT] == b'4'
            && matches!(bytes[VARIANT_AT], b'8' | b'9' | b'a' | b'b');
        shaped.then(|| BlendId(text.into()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for BlendId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, formatter)
    }
}

impl fmt::Display for BlendId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryKey(pub(crate) u64);

impl EntryKey {
    pub(crate) fn generate() -> EntryKey {
        EntryKey(rand::rng().random())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Match {
    Found(Id),
    Missing { checked: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub(crate) key: EntryKey,
    pub(crate) track: Track,
    pub(crate) matches: BTreeMap<&'static str, Match>,
}

impl Entry {
    pub(crate) fn new(track: Track) -> Entry {
        Entry {
            key: EntryKey::generate(),
            track,
            matches: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn key(&self) -> EntryKey {
        self.key
    }

    #[must_use]
    pub fn track(&self) -> &Track {
        &self.track
    }

    #[must_use]
    pub fn id_in(&self, service: &str) -> Option<&Id> {
        if self.track.service.slug == service {
            return Some(&self.track.id);
        }
        match self.matches.get(service) {
            Some(Match::Found(id)) => Some(id),
            _ => None,
        }
    }

    pub fn found(&self) -> impl Iterator<Item = (&'static str, &Id)> {
        self.matches
            .iter()
            .filter_map(|(service, status)| match status {
                Match::Found(id) => Some((*service, id)),
                Match::Missing { .. } => None,
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mirror {
    pub service: Service,
    pub account: String,
    pub playlist: Id,
    pub name: String,
    pub tracks: Vec<Id>,
}

impl Mirror {
    #[must_use]
    pub fn belongs_to(&self, connection: &Connection) -> bool {
        self.service.slug == connection.service.slug && self.account == connection.account.id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blend {
    pub(crate) id: BlendId,
    pub(crate) name: String,
    pub(crate) created: u64,
    pub(crate) entries: Vec<Entry>,
    pub(crate) mirrors: Vec<Mirror>,
    pub(crate) edited: u64,
    pub(crate) synced: u64,
}

impl Blend {
    #[must_use]
    pub fn new(name: &str) -> Blend {
        Blend {
            id: BlendId::generate(),
            name: name.to_owned(),
            created: now(),
            entries: Vec::new(),
            mirrors: Vec::new(),
            edited: 1,
            synced: 0,
        }
    }

    #[must_use]
    pub fn id(&self) -> &BlendId {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    #[must_use]
    pub fn mirrors(&self) -> &[Mirror] {
        &self.mirrors
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.edited == self.synced
    }

    #[must_use]
    pub fn tracks(&self) -> Vec<Track> {
        self.entries
            .iter()
            .map(|entry| entry.track.clone())
            .collect()
    }

    #[must_use]
    pub fn artwork(&self) -> Artwork {
        self.entries
            .iter()
            .map(|entry| &entry.track.artwork)
            .find(|artwork| !artwork.is_empty())
            .cloned()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn holds(&self, track: &Track) -> bool {
        self.entries.iter().any(|entry| entry.track.is(track))
    }

    #[must_use]
    pub fn mirrors_playlist(&self, service: &str, playlist: &Id) -> bool {
        self.mirrors
            .iter()
            .any(|mirror| mirror.service.slug == service && &mirror.playlist == playlist)
    }

    pub fn add(&mut self, tracks: &[Track]) -> usize {
        let before = self.entries.len();
        for track in tracks {
            if !self.holds(track) {
                self.entries.push(Entry::new(track.clone()));
            }
        }
        let added = self.entries.len() - before;
        if added > 0 {
            self.touch();
        }
        added
    }

    pub fn remove(&mut self, key: EntryKey) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.key != key);
        let removed = self.entries.len() != before;
        if removed {
            self.touch();
        }
        removed
    }

    pub fn rename(&mut self, name: &str) -> bool {
        if self.name == name {
            return false;
        }
        name.clone_into(&mut self.name);
        self.touch();
        true
    }

    pub(crate) fn mirror_of(&self, connection: &Connection) -> Option<&Mirror> {
        self.mirrors
            .iter()
            .find(|mirror| mirror.belongs_to(connection))
    }

    pub(crate) fn place(&mut self, after: Option<EntryKey>, entry: Entry) {
        let at = match after {
            None => 0,
            Some(key) => self
                .entries
                .iter()
                .position(|entry| entry.key == key)
                .map_or(self.entries.len(), |at| at + 1),
        };
        self.entries.insert(at, entry);
    }

    pub(crate) fn upsert_mirror(&mut self, mirror: Mirror) {
        self.mirrors.retain(|existing| {
            existing.service.slug != mirror.service.slug || existing.account != mirror.account
        });
        self.mirrors.push(mirror);
    }

    fn touch(&mut self) {
        self.edited += 1;
    }
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
