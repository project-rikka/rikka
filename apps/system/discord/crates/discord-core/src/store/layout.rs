use std::cmp::{Ordering, Reverse};

use crate::id::ChannelId;
use crate::model::channel::{Abilities, Channel, ChannelKind};
use crate::model::space::{Folder, Space};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RailItem<'a> {
    Space(&'a Space),
    Folder {
        folder: &'a Folder,
        spaces: Vec<&'a Space>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section<'a> {
    pub category: Option<&'a Channel>,
    pub rows: Vec<Row<'a>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row<'a> {
    pub channel: &'a Channel,
    pub threads: Vec<&'a Channel>,
}

fn by_position(left: &Channel, right: &Channel) -> Ordering {
    left.position
        .cmp(&right.position)
        .then_with(|| left.id.cmp(&right.id))
}

fn voice_last(channel: &Channel) -> bool {
    channel.kind.is_voice()
}

pub(crate) fn sections<'a>(channels: impl Iterator<Item = &'a Channel>) -> Vec<Section<'a>> {
    let visible: Vec<&Channel> = channels
        .filter(|channel| channel.abilities.contains(Abilities::VIEW))
        .collect();
    let mut categories: Vec<&Channel> = visible
        .iter()
        .copied()
        .filter(|channel| channel.kind == ChannelKind::Category)
        .collect();
    categories.sort_by(|left, right| by_position(left, right));
    let threads: Vec<&Channel> = visible
        .iter()
        .copied()
        .filter(|channel| {
            channel.kind == ChannelKind::Thread
                && channel
                    .thread
                    .as_ref()
                    .is_some_and(|thread| thread.joined && !thread.archived)
        })
        .collect();
    let rows_under = |parent: Option<&ChannelId>| -> Vec<Row<'a>> {
        let mut listed: Vec<&Channel> = visible
            .iter()
            .copied()
            .filter(|channel| {
                !matches!(channel.kind, ChannelKind::Category | ChannelKind::Thread)
                    && channel.parent.as_ref() == parent
            })
            .collect();
        listed.sort_by(|left, right| {
            voice_last(left)
                .cmp(&voice_last(right))
                .then_with(|| by_position(left, right))
        });
        listed
            .into_iter()
            .map(|channel| {
                let mut nested: Vec<&Channel> = threads
                    .iter()
                    .copied()
                    .filter(|thread| thread.parent.as_ref() == Some(&channel.id))
                    .collect();
                nested.sort_by_key(|thread| Reverse(thread.id));
                Row {
                    channel,
                    threads: nested,
                }
            })
            .collect()
    };
    let mut sections = Vec::with_capacity(categories.len() + 1);
    let loose = rows_under(None);
    if !loose.is_empty() {
        sections.push(Section {
            category: None,
            rows: loose,
        });
    }
    for category in categories {
        let rows = rows_under(Some(&category.id));
        if !rows.is_empty() {
            sections.push(Section {
                category: Some(category),
                rows,
            });
        }
    }
    sections
}
