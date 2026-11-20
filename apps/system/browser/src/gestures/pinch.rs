use browser_model::{TabId, ZOOM_STEPS};

const LEAST: f64 = ZOOM_STEPS[0];
const MOST: f64 = ZOOM_STEPS[ZOOM_STEPS.len() - 1];
const NOTICED: f64 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pinching {
    pub(crate) tab: TabId,
    pub(crate) from: f64,
    pub(crate) shown: f64,
}

impl Pinching {
    pub(crate) fn begin(tab: TabId, from: f64) -> Pinching {
        Pinching {
            tab,
            from,
            shown: from,
        }
    }

    pub(crate) fn follow(&mut self, scale: f32) -> Option<f64> {
        let zoom = following(self.from, scale);
        if (zoom / self.shown).ln().abs() < NOTICED {
            return None;
        }
        self.shown = zoom;
        Some(zoom)
    }

    pub(crate) fn settled(&self) -> f64 {
        snap(self.shown)
    }
}

pub(crate) fn following(from: f64, scale: f32) -> f64 {
    let scale = match scale.is_finite() && scale > 0.0 {
        true => f64::from(scale),
        false => 1.0,
    };
    (from * scale).clamp(LEAST, MOST)
}

pub(crate) fn snap(zoom: f64) -> f64 {
    ZOOM_STEPS
        .iter()
        .copied()
        .min_by(|left, right| {
            let distance = |step: f64| (step / zoom).ln().abs();
            distance(*left).total_cmp(&distance(*right))
        })
        .unwrap_or(1.0)
}
