use std::sync::Arc;

use animate_cast::Subject;
use animate_wind::WindMotion;

use crate::animate::Animate;

impl Animate {
    pub(crate) fn wind_motion(&self) -> Arc<WindMotion> {
        Arc::clone(&self.cast.wind().motion)
    }

    pub(super) fn update_wind(&mut self, change: impl FnOnce(&mut WindMotion)) {
        self.update::<WindMotion>(Subject::Wind, change);
    }
}
