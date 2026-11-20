use std::ops::Range;

use crate::track::Track;
use rikka_coded::Coded;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clip {
    pub pictures: Vec<Coded>,
    pub sounds: Vec<Vec<Coded>>,
    pub length_us: i64,
}

impl Clip {
    pub(crate) fn cut(pictures: &Track, sounds: &[Track], span: Range<i64>) -> Option<Clip> {
        let Range {
            start: from,
            end: to,
        } = span;
        if to <= from {
            return None;
        }
        let packets = pictures.packets();
        let start = pictures.keyframe_at_or_before(from).unwrap_or(0);
        let first = packets.get(start)?;
        if first.pts_us >= to {
            return None;
        }
        let unchanged = packets.get(start + 1).is_none_or(|next| next.pts_us > from);
        let origin = match first.pts_us < from && unchanged {
            true => from,
            false => first.pts_us,
        };
        let mut pictures: Vec<Coded> = packets
            .range(start..)
            .take_while(|packet| packet.pts_us < to)
            .map(|packet| packet.shifted(origin))
            .collect();
        let opening = pictures.first_mut()?;
        opening.pts_us = opening.pts_us.max(0);
        opening.dts_us = opening.dts_us.max(0);
        let length_us = to - origin;
        let closing = pictures.last_mut()?;
        closing.duration_us = (length_us - closing.pts_us).max(1);
        let sounds = sounds
            .iter()
            .map(|sound| {
                sound
                    .packets()
                    .iter()
                    .filter(|packet| packet.pts_us >= origin && packet.pts_us < to)
                    .map(|packet| packet.shifted(origin))
                    .collect()
            })
            .collect();
        Some(Clip {
            pictures,
            sounds,
            length_us,
        })
    }
}
