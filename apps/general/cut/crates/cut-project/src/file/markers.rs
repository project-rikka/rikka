use serde::{Deserialize, Serialize};

use super::is_span;
use crate::markers::{Marker, MarkerColor, Markers};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredMarker {
    at: i64,
    #[serde(default, skip_serializing_if = "is_point")]
    length: i64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    note: String,
    color: StoredColor,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredColor {
    Green,
    Red,
    Purple,
    Orange,
    Yellow,
    White,
    Blue,
    Cyan,
}

fn is_point(length: &i64) -> bool {
    *length == 0
}

pub(super) fn stored_markers(markers: &Markers) -> Vec<StoredMarker> {
    markers
        .iter()
        .map(|(_, marker)| StoredMarker {
            at: marker.at,
            length: marker.length,
            name: marker.name.clone(),
            note: marker.note.clone(),
            color: match marker.color {
                MarkerColor::Green => StoredColor::Green,
                MarkerColor::Red => StoredColor::Red,
                MarkerColor::Purple => StoredColor::Purple,
                MarkerColor::Orange => StoredColor::Orange,
                MarkerColor::Yellow => StoredColor::Yellow,
                MarkerColor::White => StoredColor::White,
                MarkerColor::Blue => StoredColor::Blue,
                MarkerColor::Cyan => StoredColor::Cyan,
            },
        })
        .collect()
}

pub(super) fn restored_markers(stored: &[StoredMarker]) -> Result<Vec<Marker>, String> {
    stored
        .iter()
        .map(|marker| {
            if !is_span(marker.at)
                || !is_span(marker.at.saturating_add(marker.length))
                || marker.length < 0
            {
                return Err(format!("a marker at {} has no place in time", marker.at));
            }
            Ok(Marker {
                at: marker.at,
                length: marker.length,
                name: marker.name.clone(),
                note: marker.note.clone(),
                color: match marker.color {
                    StoredColor::Green => MarkerColor::Green,
                    StoredColor::Red => MarkerColor::Red,
                    StoredColor::Purple => MarkerColor::Purple,
                    StoredColor::Orange => MarkerColor::Orange,
                    StoredColor::Yellow => MarkerColor::Yellow,
                    StoredColor::White => MarkerColor::White,
                    StoredColor::Blue => MarkerColor::Blue,
                    StoredColor::Cyan => MarkerColor::Cyan,
                },
            })
        })
        .collect()
}
