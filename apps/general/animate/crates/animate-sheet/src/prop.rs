use std::sync::Arc;

use animate_body::Faces;
use animate_motion::Take;
use animate_prop::{PropMotion, PropSetting};

use crate::display::BoneDisplay;
use crate::figure::Listing;
use crate::sheet::{Group, GroupKind, Lane, Sheet, Track};

impl Sheet {
    #[must_use]
    pub fn prop(motion: &PropMotion, attaches: bool) -> Self {
        Self::new(vec![placement(motion, attaches)])
    }

    #[must_use]
    pub fn prop_with_take(
        motion: &PropMotion,
        attaches: bool,
        display: &BoneDisplay,
        faces: &Faces,
        take: &Take,
    ) -> Self {
        if take.is_still() {
            return Self::prop(motion, attaches);
        }
        let keyed = |name: &str| !take.bones.keys(name).is_empty();
        let listing = Listing::Bones {
            display,
            shown: &keyed,
        };
        let taken = Self::of_take(listing, faces, take);
        let groups = std::iter::once(placement(motion, attaches))
            .chain(taken.groups.into_iter().map(Arc::unwrap_or_clone))
            .collect();
        Self::new(groups)
    }
}

fn placement(motion: &PropMotion, attaches: bool) -> Group {
    let settings = PropSetting::ALL.iter().map(|setting| {
        Track::plain(
            Lane::Prop(*setting),
            motion
                .tracks()
                .keys(setting)
                .iter()
                .map(|key| key.frame)
                .collect(),
        )
    });
    let anchor = attaches.then(|| {
        Track::plain(
            Lane::Anchor,
            motion
                .anchors()
                .keys(&())
                .iter()
                .map(|key| key.frame)
                .collect(),
        )
    });
    Group::new(GroupKind::Prop, settings.chain(anchor).collect())
}
