use rikka_app::prelude::*;

#[derive(Clone, Debug, Default)]
pub(crate) struct Places {
    pub(in crate::project) threads: Place,
    pub(in crate::project) files: Place,
    pub(in crate::project) changes: Place,
    pub(in crate::project) history: Place,
    pub(in crate::project) diff: Place,
}
