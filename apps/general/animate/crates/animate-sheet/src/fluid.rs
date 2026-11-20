use animate_fluid::{Emitter, EmitterSection};

use crate::sheet::{Group, GroupKind, Lane, Pouring, Sheet, Track};

impl Sheet {
    #[must_use]
    pub fn fluid(emitter: &Emitter) -> Self {
        Self::new(
            EmitterSection::ALL
                .into_iter()
                .filter_map(|section| {
                    let tracks: Vec<Track> = section
                        .settings()
                        .filter(|setting| emitter.applies(*setting))
                        .map(|setting| {
                            Track::plain(
                                Lane::Fluid(setting),
                                emitter
                                    .tracks()
                                    .keys(&setting)
                                    .iter()
                                    .map(|key| key.frame)
                                    .collect(),
                            )
                        })
                        .collect();
                    (!tracks.is_empty()).then(|| Group::new(GroupKind::Fluid(section), tracks))
                })
                .collect(),
        )
        .with_pouring(Pouring {
            pour: emitter.pour,
            flows: emitter.shape.flows(),
        })
    }
}
