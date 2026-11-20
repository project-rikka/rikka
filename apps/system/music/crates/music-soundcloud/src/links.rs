use rikka_sync::{Mutex, MutexGuard};
use std::collections::HashMap;

use music_core::Shared;
use serde_json::Value;

use crate::parse::{id, text};

const PLAYER: &str = "https://w.soundcloud.com/player/?url=https%3A//api.soundcloud.com";
const GENERATION: usize = 4096;

type Known = HashMap<(Kind, String), String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    Track,
    Playlist,
}

#[derive(Debug, Default)]
pub(crate) struct Links {
    known: Mutex<Generations>,
}

#[derive(Debug, Default)]
struct Generations {
    current: Known,
    previous: Known,
}

impl Links {
    pub(crate) fn absorb(&self, value: &Value) {
        let mut links = Vec::new();
        collect_links(value, &mut links);
        if links.is_empty() {
            return;
        }
        let mut generations = self.generations();
        for (key, link) in links {
            if generations.current.len() >= GENERATION {
                generations.previous = std::mem::take(&mut generations.current);
            }
            generations.current.insert(key, link);
        }
    }

    pub(crate) fn url_for(&self, shared: Shared<'_>) -> String {
        let (kind, id, path) = match shared {
            Shared::Track(id) => (Kind::Track, id, "tracks"),
            Shared::Playlist(id) => (Kind::Playlist, id, "playlists"),
        };
        let key = (kind, id.to_string());
        let generations = self.generations();
        generations
            .current
            .get(&key)
            .or_else(|| generations.previous.get(&key))
            .cloned()
            .unwrap_or_else(|| format!("{PLAYER}/{path}/{id}"))
    }

    fn generations(&self) -> MutexGuard<'_, Generations> {
        self.known.lock()
    }
}

fn collect_links(value: &Value, links: &mut Vec<((Kind, String), String)>) {
    match value {
        Value::Array(items) => items.iter().for_each(|item| collect_links(item, links)),
        Value::Object(fields) => {
            let kind = match text(value, "kind") {
                Some("track") => Some(Kind::Track),
                Some("playlist" | "system-playlist") => Some(Kind::Playlist),
                _ => None,
            };
            if let (Some(kind), Some(id), Some(link)) = (
                kind,
                value.get("id").and_then(id),
                text(value, "permalink_url"),
            ) {
                links.push(((kind, id.to_string()), link.to_owned()));
            }
            fields
                .values()
                .for_each(|field| collect_links(field, links));
        }
        _ => {}
    }
}
