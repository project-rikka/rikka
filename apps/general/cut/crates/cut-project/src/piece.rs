use std::sync::Arc;

use crate::animation_presets::AnimationPresets;
use crate::camera::Camera;
use crate::clip::{ClipId, Source};
use crate::effect_slot::EffectSlot;
use crate::footage::Treatment;
use crate::graphic::Graphic;
use crate::markers::Markers;
use crate::media::Media;
use crate::motion::Motion;
use crate::project::Project;
use crate::region::Region;
use crate::transitions::Ends;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Content {
    Footage {
        media: Media,
        source_in: i64,
        treatment: Treatment,
    },
    Graphic(Arc<Graphic>),
    Region(Arc<Region>),
    Camera(Arc<Camera>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub(crate) content: Content,
    pub(crate) layer: usize,
    pub(crate) start: i64,
    pub(crate) length: i64,
    pub(crate) motion: Motion,
    pub(crate) effects: Vec<EffectSlot>,
    pub(crate) animation_presets: AnimationPresets,
    pub(crate) transitions: Ends,
    pub(crate) markers: Markers,
}

impl Project {
    #[must_use]
    pub fn pieces(&self, clips: &[ClipId]) -> Vec<Piece> {
        let selected: Vec<_> = clips
            .iter()
            .filter_map(|id| self.locate(*id))
            .filter_map(|(layer, clip)| {
                let content = match &clip.source {
                    Source::Footage {
                        media,
                        source_in,
                        treatment,
                    } => Content::Footage {
                        media: self.media(*media)?.clone(),
                        source_in: *source_in,
                        treatment: *treatment,
                    },
                    Source::Graphic(graphic) => Content::Graphic(Arc::clone(graphic)),
                    Source::Region(region) => Content::Region(Arc::clone(region)),
                    Source::Camera(camera) => Content::Camera(Arc::clone(camera)),
                };
                Some((layer, clip, content))
            })
            .collect();
        let Some(first) = selected.iter().map(|(_, clip, _)| clip.start).min() else {
            return Vec::new();
        };
        let lowest = selected
            .iter()
            .map(|(layer, _, _)| *layer)
            .min()
            .unwrap_or(0);
        selected
            .into_iter()
            .map(|(layer, clip, content)| Piece {
                content,
                layer: layer - lowest,
                start: clip.start - first,
                length: clip.length,
                motion: clip.motion.clone(),
                effects: clip.effects.clone(),
                animation_presets: clip.animation_presets,
                transitions: clip.transitions,
                markers: clip.markers.clone(),
            })
            .collect()
    }
}
