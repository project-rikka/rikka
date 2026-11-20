use std::collections::BTreeMap;

use animate_look::{Look, LookGroup, shared_library};

use crate::sheet::{Group, GroupKind, Lane, Sheet, Track};

impl Sheet {
    #[must_use]
    pub fn look(look: &Look) -> Self {
        let mut groups: BTreeMap<LookGroup, Vec<Track>> = BTreeMap::new();
        for (target, keyed) in look.tracks().tracks() {
            groups.entry(target.group()).or_default().push(Track::plain(
                Lane::Look(target.clone()),
                keyed.iter().map(|key| key.frame).collect(),
            ));
        }
        Self::new(
            groups
                .into_iter()
                .map(|(group, tracks)| {
                    let title = match &group {
                        LookGroup::Node { graph, scope, node } => {
                            look.node_title(shared_library(), graph, scope, *node)
                        }
                        LookGroup::Slot { .. } => None,
                    };
                    Group::new(GroupKind::Look { group, title }, tracks)
                })
                .collect(),
        )
    }
}
