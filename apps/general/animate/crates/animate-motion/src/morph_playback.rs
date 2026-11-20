use std::sync::Arc;

use animate_body::{Expression, Faces, MorphWeights};
use rikka_model_core::Instance;

use crate::blink::Blinking;
use crate::take::Take;
use crate::tuning::{BAKE_TAIL_FRAMES, BLINK_HELD, FRAMES_PER_SECOND};

#[derive(Clone, Debug)]
pub struct MorphPlayback {
    faces: Arc<Faces>,
    take: Arc<Take>,
    resolved: Vec<Option<usize>>,
    blinking: Option<Blinking>,
}

impl MorphPlayback {
    #[must_use]
    pub fn new(faces: Arc<Faces>, take: Arc<Take>) -> Self {
        let blinking = take
            .human
            .as_ref()
            .map(|human| &human.settings)
            .filter(|settings| settings.blink && faces.has(Expression::Blink))
            .map(|settings| Blinking::new(settings.seed, take.last_frame() + BAKE_TAIL_FRAMES));
        let resolved = take
            .morphs
            .keyed()
            .map(|morph| faces.index(morph))
            .collect();
        Self {
            faces,
            take,
            resolved,
            blinking,
        }
    }

    pub fn write(&self, seconds: f32, over: &MorphWeights, instance: &mut Instance) {
        if self.take.morphs.is_empty() && self.blinking.is_none() && over.is_empty() {
            return;
        }
        let frame = (seconds * FRAMES_PER_SECOND).max(0.0);
        let faces = &self.faces;
        let mut amounts = faces.amounts();
        for (name, morph) in self.take.morphs.keyed().zip(&self.resolved) {
            if let Some(morph) = morph {
                amounts.set(*morph, self.take.morph_weight(name, frame));
            }
        }
        for (morph, weight) in over.iter() {
            if let Some(morph) = faces.index(morph) {
                amounts.set(morph, weight);
            }
        }
        if let Some(blinking) = &self.blinking {
            let closed = faces.strength_in(&amounts, Expression::Blink);
            let eyes_closed =
                closed > BLINK_HELD || faces.strength_in(&amounts, Expression::Happy) > BLINK_HELD;
            let blink = blinking.weight(frame);
            if !eyes_closed && blink > closed {
                faces.express_in(&mut amounts, Expression::Blink, blink);
            }
        }
        faces.write_amounts(&amounts, instance);
    }
}
