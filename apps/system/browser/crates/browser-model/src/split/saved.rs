use browser_store::{SessionSplit, SplitAxis, SplitGroupId, SplitMember};

use crate::TabId;
use crate::split::group::{Arrangement, Group};

pub(crate) struct SavedGroup {
    pub(crate) id: SplitGroupId,
    pub(crate) arrangement: Arrangement,
    pub(crate) panes: Vec<TabId>,
    pub(crate) shares: Vec<f32>,
}

pub(crate) fn member(group: &Group, index: usize) -> SplitMember {
    SplitMember {
        group: group.id,
        order: index as u32,
    }
}

pub(crate) fn split(group: &Group) -> SessionSplit {
    SessionSplit {
        group: group.id,
        axis: match group.arrangement {
            Arrangement::SideBySide => SplitAxis::Horizontal,
            Arrangement::Stacked => SplitAxis::Vertical,
        },
        shares: group.shares().to_vec(),
    }
}

pub(crate) fn gathered(
    members: &[(TabId, SplitMember)],
    splits: &[SessionSplit],
) -> Vec<SavedGroup> {
    splits
        .iter()
        .map(|split| {
            let mut panes: Vec<(u32, TabId)> = members
                .iter()
                .filter(|(_, member)| member.group == split.group)
                .map(|(tab, member)| (member.order, *tab))
                .collect();
            panes.sort_by_key(|(order, _)| *order);
            SavedGroup {
                id: split.group,
                arrangement: match split.axis {
                    SplitAxis::Horizontal => Arrangement::SideBySide,
                    SplitAxis::Vertical => Arrangement::Stacked,
                },
                panes: panes.into_iter().map(|(_, tab)| tab).collect(),
                shares: split.shares.clone(),
            }
        })
        .collect()
}
