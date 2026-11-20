use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use desk_layout::WidgetId;

#[must_use]
pub(crate) fn pick(
    pictures: &[PathBuf],
    widget: WidgetId,
    slot: u64,
    shuffled: bool,
) -> Option<&Path> {
    let count = pictures.len() as u64;
    if count == 0 {
        return None;
    }
    let place = (slot % count) as usize;
    if !shuffled {
        return Some(&pictures[place]);
    }
    let round = slot / count;
    let mut order: Vec<(u64, usize)> = (0..pictures.len())
        .map(|index| (scattered(widget, round, index), index))
        .collect();
    let (_, chosen, _) = order.select_nth_unstable(place);
    Some(&pictures[chosen.1])
}

fn scattered(widget: WidgetId, round: u64, index: usize) -> u64 {
    let mut hasher = DefaultHasher::new();
    (widget, round, index).hash(&mut hasher);
    hasher.finish()
}
