use super::channel::Tracked;
use super::key::{Handle, Incoming, Key, Outgoing, Timing};
use super::kind::Kind;
use super::track::Track;

impl<K: Kind> Track<K> {
    pub(crate) fn retimed(&self, at: impl Fn(i64) -> i64, ratio: f64) -> Track<K> {
        let mut keys: Vec<Key<K>> = Vec::with_capacity(self.keys().len());
        for key in self.keys() {
            let moved = Key {
                at: at(key.at),
                timing: sped(key.timing, (1.0 / ratio) as f32),
                ..*key
            };
            match keys.last_mut() {
                Some(last) if last.at == moved.at => *last = moved,
                _ => keys.push(moved),
            }
        }
        Track::from_normalized(keys)
    }

    pub(crate) fn scaled(&self, factor: f32) -> Track<K> {
        let keys = self
            .keys()
            .iter()
            .map(|key| {
                let mut coords = K::coords(key.value);
                for coord in coords.iter_mut().take(K::DIMENSIONS) {
                    *coord *= f64::from(factor);
                }
                let mut bend = key.bend;
                if let Some(spatial) = K::bend_mut(&mut bend) {
                    spatial.incoming = spatial.incoming * factor;
                    spatial.outgoing = spatial.outgoing * factor;
                }
                Key {
                    value: K::from_coords(coords),
                    timing: sped(key.timing, factor),
                    bend,
                    ..*key
                }
            })
            .collect();
        Track::from_normalized(keys)
    }
}

impl Tracked {
    pub(crate) fn retimed(&self, at: impl Fn(i64) -> i64, ratio: f64) -> Tracked {
        match self {
            Tracked::Scalar(track) => Tracked::Scalar(track.retimed(at, ratio)),
            Tracked::Planar(track) => Tracked::Planar(track.retimed(at, ratio)),
            Tracked::Spatial(track) => Tracked::Spatial(track.retimed(at, ratio)),
            Tracked::Chroma(track) => Tracked::Chroma(track.retimed(at, ratio)),
        }
    }

    pub(crate) fn scaled(&self, factor: f32) -> Tracked {
        match self {
            Tracked::Scalar(track) => Tracked::Scalar(track.scaled(factor)),
            Tracked::Planar(track) => Tracked::Planar(track.scaled(factor)),
            Tracked::Spatial(track) => Tracked::Spatial(track.scaled(factor)),
            Tracked::Chroma(_) => self.clone(),
        }
    }
}

fn sped(timing: Timing, factor: f32) -> Timing {
    let handle = |handle: Handle| Handle {
        speed: handle.speed * factor,
        ..handle
    };
    Timing {
        incoming: match timing.incoming {
            Incoming::Bezier(bezier) => Incoming::Bezier(handle(bezier)),
            Incoming::Linear => Incoming::Linear,
        },
        outgoing: match timing.outgoing {
            Outgoing::Bezier(bezier) => Outgoing::Bezier(handle(bezier)),
            other => other,
        },
        ..timing
    }
}
