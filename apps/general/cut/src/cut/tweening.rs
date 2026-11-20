use cut_project::{Clip, ClipId, Edit, Tween, TweenChange};
use rikka_app::prelude::*;
use rikka_keyframe::Easing;

use super::{Cut, Popup};
use crate::easing::Picking;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Endpoint {
    Start,
    End,
}

impl Endpoint {
    pub(crate) const ALL: [Endpoint; 2] = [Endpoint::Start, Endpoint::End];

    pub(crate) fn frame(self, tween: &Tween) -> i64 {
        match self {
            Endpoint::Start => tween.start,
            Endpoint::End => tween.end,
        }
    }
}

impl Cut {
    pub(crate) fn set_tween(&mut self, on: bool) {
        let Some(clip) = self.selected_animated().map(Clip::id) else {
            skip_view();
            return;
        };
        let change = match on {
            true => TweenChange::On,
            false => TweenChange::Off {
                at: self.transport.frame(),
            },
        };
        self.change_tween(clip, change);
        if on {
            self.go_to_endpoint(Endpoint::End);
        }
    }

    pub(crate) fn ease_tween(&mut self, clip: ClipId, easing: Easing) {
        self.change_tween(clip, TweenChange::Ease(easing));
    }

    pub(crate) fn reverse_tween(&mut self) {
        match self.selected_animated().map(Clip::id) {
            Some(clip) => self.change_tween(clip, TweenChange::Reverse),
            None => skip_view(),
        }
    }

    pub(crate) fn go_to_endpoint(&mut self, endpoint: Endpoint) {
        match self.selected_animated().and_then(Clip::tween) {
            Some(tween) => self.go_to(endpoint.frame(&tween)),
            None => skip_view(),
        }
    }

    pub(crate) fn open_tween_easing(&mut self, anchor: Anchor) {
        match self
            .selected_animated()
            .filter(|clip| clip.tween().is_some())
        {
            Some(clip) => {
                self.session.popup = Some(Popup::Easing(Picking::tween(clip.id(), anchor)))
            }
            None => skip_view(),
        }
    }

    fn change_tween(&mut self, clip: ClipId, change: TweenChange) {
        self.commit_edits(vec![Edit::Tween { clip, change }]);
    }
}
