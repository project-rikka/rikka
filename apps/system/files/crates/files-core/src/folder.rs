use crate::entry::Entry;
use crate::family::Family;
use crate::sort::{SortKey, Sorting};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ViewMode {
    Icons,
    #[default]
    Details,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FolderKind {
    #[default]
    General,
    Pictures,
    Music,
    Videos,
    Documents,
}

const MAJORITY_SHARE: f32 = 2.0 / 3.0;

const MIN_SAMPLE: usize = 4;

impl FolderKind {
    #[must_use]
    pub fn detect(entries: &[Entry]) -> Self {
        let mut counts = [0usize; 5];
        let mut total = 0usize;
        for entry in entries {
            if entry.is_directory() || entry.hidden {
                continue;
            }
            total += 1;
            let bucket = match entry.family() {
                Family::Image => 0,
                Family::Audio => 1,
                Family::Video => 2,
                Family::Document => 3,
                _ => 4,
            };
            counts[bucket] += 1;
        }
        if total < MIN_SAMPLE {
            return Self::General;
        }
        let needed = (total as f32 * MAJORITY_SHARE).ceil() as usize;
        [
            (counts[0], Self::Pictures),
            (counts[1], Self::Music),
            (counts[2], Self::Videos),
            (counts[3], Self::Documents),
        ]
        .into_iter()
        .find_map(|(count, kind)| (count >= needed).then_some(kind))
        .unwrap_or(Self::General)
    }

    #[must_use]
    pub fn view(self) -> ViewMode {
        match self {
            Self::Pictures | Self::Videos => ViewMode::Icons,
            Self::General | Self::Music | Self::Documents => ViewMode::Details,
        }
    }

    #[must_use]
    pub fn sorting(self) -> Sorting {
        match self {
            Self::Documents => Sorting::new().toggled(SortKey::Modified),
            _ => Sorting::new(),
        }
    }
}
