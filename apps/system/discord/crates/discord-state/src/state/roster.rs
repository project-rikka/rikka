use std::collections::BTreeMap;

use discord_core::{RoleId, Roster, RosterGroup, RosterRow, UserId};

use super::permissions::bits;
use discord_wire::Overwrite;
use discord_wire::snowflake;
use discord_wire::{ListGroup, ListItem, ListOp};

const PAGE: u32 = 100;
const MAX_RANGES: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Row {
    Group(ListGroup),
    Member(String),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MemberList {
    groups: Vec<ListGroup>,
    rows: BTreeMap<u32, Row>,
}

impl MemberList {
    pub(crate) fn apply(&mut self, groups: &[ListGroup], ops: &[ListOp]) {
        if !groups.is_empty() {
            self.groups = groups.to_vec();
        }
        for op in ops {
            match op.op.as_str() {
                "SYNC" => {
                    if let Some([start, _]) = op.range {
                        for (offset, item) in op.items.iter().enumerate() {
                            if let Some(row) = row(item) {
                                self.rows.insert(start + offset as u32, row);
                            }
                        }
                    }
                }
                "INSERT" => {
                    if let (Some(index), Some(row)) = (op.index, op.item.as_ref().and_then(row)) {
                        let moved: Vec<(u32, Row)> =
                            self.rows.split_off(&index).into_iter().collect();
                        self.rows
                            .extend(moved.into_iter().map(|(at, row)| (at + 1, row)));
                        self.rows.insert(index, row);
                    }
                }
                "UPDATE" => {
                    if let (Some(index), Some(row)) = (op.index, op.item.as_ref().and_then(row)) {
                        self.rows.insert(index, row);
                    }
                }
                "DELETE" => {
                    if let Some(index) = op.index {
                        self.rows.remove(&index);
                        let moved: Vec<(u32, Row)> =
                            self.rows.split_off(&index).into_iter().collect();
                        self.rows
                            .extend(moved.into_iter().map(|(at, row)| (at - 1, row)));
                    }
                }
                "INVALIDATE" => {
                    if let Some([start, end]) = op.range {
                        let tail = self.rows.split_off(&start);
                        self.rows
                            .extend(tail.into_iter().filter(|(at, _)| *at > end));
                    }
                }
                _ => {}
            }
        }
    }

    pub(crate) fn snapshot(&self) -> Roster {
        let total: u32 = self.groups.iter().map(|group| group.count + 1).sum();
        let len = total.max(self.rows.keys().next_back().map_or(0, |last| last + 1));
        let known = self
            .rows
            .iter()
            .map(|(&at, row)| {
                let row = match row {
                    Row::Group(group) => RosterRow::Group {
                        label: label(&group.id),
                        count: self
                            .groups
                            .iter()
                            .find(|known| known.id == group.id)
                            .map_or(group.count, |known| known.count),
                    },
                    Row::Member(user) => RosterRow::Member(snowflake::id::<UserId>(user.as_str())),
                };
                (at, row)
            })
            .collect();
        Roster { len, known }
    }

    pub(crate) fn members(items: &[ListOp]) -> impl Iterator<Item = &ListItem> {
        items
            .iter()
            .flat_map(|op| op.items.iter().chain(op.item.iter()))
            .filter(|item| item.member.is_some())
    }
}

fn row(item: &ListItem) -> Option<Row> {
    if let Some(group) = &item.group {
        return Some(Row::Group(group.clone()));
    }
    let member = item.member.as_ref()?;
    member.user_id().map(|user| Row::Member(user.to_owned()))
}

fn label(id: &str) -> RosterGroup {
    match id {
        "online" => RosterGroup::Online,
        "offline" => RosterGroup::Offline,
        role => RosterGroup::Role(snowflake::id::<RoleId>(role)),
    }
}

pub(crate) fn list_id(overwrites: &[Overwrite], everyone: u64) -> String {
    let hidden = overwrites
        .iter()
        .any(|overwrite| overwrite.deny & bits::VIEW_CHANNEL != 0);
    if !hidden && everyone & bits::VIEW_CHANNEL != 0 {
        return "everyone".to_owned();
    }
    let mut inputs: Vec<String> = overwrites
        .iter()
        .filter_map(|overwrite| {
            if overwrite.allow & bits::VIEW_CHANNEL != 0 {
                Some(format!("allow:{}", overwrite.id))
            } else if overwrite.deny & bits::VIEW_CHANNEL != 0 {
                Some(format!("deny:{}", overwrite.id))
            } else {
                None
            }
        })
        .collect();
    inputs.sort();
    murmur3(inputs.join(",").as_bytes(), 0).to_string()
}

pub(crate) fn ranges(rows: std::ops::Range<u32>) -> Vec<[u32; 2]> {
    let mut ranges = vec![[0, PAGE - 1]];
    let last = rows.end.saturating_sub(1).max(rows.start);
    if last < PAGE {
        return ranges;
    }
    let first_page = rows.start / PAGE * PAGE;
    if first_page > 0 {
        ranges.push([first_page, first_page + PAGE - 1]);
    }
    let last_page = last / PAGE * PAGE;
    if last_page != first_page && last_page > 0 && ranges.len() < MAX_RANGES {
        ranges.push([last_page, last_page + PAGE - 1]);
    }
    ranges
}

fn murmur3(data: &[u8], seed: u32) -> u32 {
    const C1: u32 = 0xcc9e_2d51;
    const C2: u32 = 0x1b87_3593;
    let mut hash = seed;
    let (blocks, tail) = data.as_chunks::<4>();
    for block in blocks {
        let mut k = u32::from_le_bytes(*block);
        k = k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
        hash ^= k;
        hash = hash
            .rotate_left(13)
            .wrapping_mul(5)
            .wrapping_add(0xe654_6b64);
    }
    if !tail.is_empty() {
        let mut k = tail
            .iter()
            .rev()
            .fold(0u32, |acc, &byte| (acc << 8) | u32::from(byte));
        k = k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
        hash ^= k;
    }
    hash ^= data.len() as u32;
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85eb_ca6b);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xc2b2_ae35);
    hash ^ (hash >> 16)
}
