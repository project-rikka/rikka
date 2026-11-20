use rikka_image_edit::{Preview, Recipe};

use crate::develop::DevelopedLayer;
use crate::figure::Figure;
use crate::layer::LayerId;

#[derive(Debug)]
pub(crate) struct Previewing {
    pub(crate) layer: LayerId,
    pub(crate) revision: u64,
    pub(crate) preview: Preview,
    pub(crate) placed: Option<DevelopedLayer>,
    pub(crate) overlaid: bool,
    pub(crate) figures: Vec<Figure>,
}

impl Previewing {
    pub(crate) fn serves(&self, layer: LayerId, revision: u64) -> bool {
        self.layer == layer && self.revision == revision
    }
}

pub(crate) fn reshapes(recipe: &Recipe) -> bool {
    recipe
        .steps()
        .iter()
        .any(|step| step.operation().reshapes())
}
