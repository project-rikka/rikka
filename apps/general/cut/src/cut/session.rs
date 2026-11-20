use std::collections::BTreeSet;
use std::sync::Arc;

use cut_project::{ClipId, EditPoint, Height};
use rikka_app::prelude::*;

use super::{Draft, Keyframing, Marking, Pointing, Popup, TransitionPreview};

#[derive(State, Default)]
pub(super) struct Session {
    pub(super) selected: Arc<BTreeSet<ClipId>>,
    pub(super) edit_point: Option<EditPoint>,
    pub(super) keyframing: Keyframing,
    pub(super) marking: Option<Marking>,
    pub(super) popup: Option<Popup>,
    pub(super) resizing: Option<(usize, Height)>,
    pub(super) pointing: Option<Pointing>,
    pub(super) writing: Option<Draft>,
    #[untracked]
    pub(super) transition_preview: Option<TransitionPreview>,
}
