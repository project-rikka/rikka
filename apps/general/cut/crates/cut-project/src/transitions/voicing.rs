use std::f64::consts::FRAC_PI_2;

use crate::alignment::Alignment;
use crate::clip::Clip;
use crate::project::Project;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudibleClip<'a> {
    pub layer: usize,
    pub clip: &'a Clip,
    pub gain: f32,
}

impl Project {
    #[must_use]
    pub fn voices_at(&self, at: f64) -> Vec<AudibleClip<'_>> {
        let frame = at.floor() as i64;
        let mut voices = Vec::new();
        for (layer, layer_ref) in self.layers().iter().enumerate() {
            if !layer_ref.is_shown() {
                continue;
            }
            let transition_span = self
                .transition_span_at(layer, frame)
                .filter(|transition_span| transition_span.transition.sound);
            let Some(transition_span) = transition_span else {
                if let Some(clip) = layer_ref
                    .clip_at(frame)
                    .filter(|clip| self.sounds(clip, at))
                {
                    voices.push(AudibleClip {
                        layer,
                        clip,
                        gain: 1.0,
                    });
                }
                continue;
            };
            if let (Some(out), Some(into)) = (transition_span.out, transition_span.into)
                && continues(out, into)
            {
                let clip = if frame < transition_span.cut {
                    out
                } else {
                    into
                };
                if self.sounds(clip, at) {
                    voices.push(AudibleClip {
                        layer,
                        clip,
                        gain: 1.0,
                    });
                }
                continue;
            }
            let turn = transition_span.progress(at) * FRAC_PI_2;
            let sides = [
                (transition_span.out, turn.cos()),
                (transition_span.into, turn.sin()),
            ];
            for (clip, gain) in sides {
                if let Some(clip) = clip.filter(|clip| self.sounds(clip, at)) {
                    voices.push(AudibleClip {
                        layer,
                        clip,
                        gain: gain as f32,
                    });
                }
            }
        }
        voices
    }

    fn sounds(&self, clip: &Clip, at: f64) -> bool {
        let Some(media) = clip.media() else {
            return false;
        };
        let source = clip.source_position(at);
        self.has_sound(clip) && source >= 0.0 && source < self.media_frames(media) as f64
    }
}

fn continues(out: &Clip, into: &Clip) -> bool {
    Alignment::for_clip(out)
        .zip(Alignment::for_clip(into))
        .is_some_and(|(out, into)| out.continues(&into))
}
