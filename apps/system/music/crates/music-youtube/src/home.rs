use std::sync::Arc;

use music_core::{Card, Shelf, Track};
use serde_json::Value;

use crate::json::Walk;
use crate::parse;

const QUICK_PICKS: &str = "Quick picks";

pub(crate) fn shelves(answer: &Value, into: &mut Vec<Shelf>) {
    for shelf in answer.every("musicCarouselShelfRenderer") {
        let title = shelf
            .message(&["header", "musicCarouselShelfBasicHeaderRenderer", "title"])
            .unwrap_or_default();
        let items = shelf.list(&["contents"]);
        let tracks: Vec<Track> = items
            .iter()
            .filter(|item| item.get("musicResponsiveListItemRenderer").is_some())
            .zip(1..)
            .filter_map(|(item, number)| parse::list_track(item, number))
            .filter(|track| track.playable)
            .collect();
        let shelved = match tracks.is_empty() {
            false if title == QUICK_PICKS => Shelf::QuickPicks(tracks.into()),
            false => Shelf::Songs {
                title,
                tracks: tracks.into(),
            },
            true => Shelf::Titled {
                title,
                cards: items
                    .iter()
                    .filter_map(parse::card)
                    .collect::<Arc<[Card]>>(),
            },
        };
        if !shelved.is_empty() {
            into.push(shelved);
        }
    }
}

pub(crate) fn continuation(answer: &Value) -> Option<String> {
    answer
        .every("nextContinuationData")
        .into_iter()
        .find_map(|data| data.text_at(&["continuation"]))
        .or_else(|| {
            answer
                .every("continuationCommand")
                .into_iter()
                .find_map(|command| command.text_at(&["token"]))
        })
        .map(str::to_owned)
}
