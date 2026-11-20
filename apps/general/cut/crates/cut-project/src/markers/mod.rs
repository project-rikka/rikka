mod editing;

pub use editing::{MarkerChange, MarkerScope};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MarkerColor {
    #[default]
    Green,
    Red,
    Purple,
    Orange,
    Yellow,
    White,
    Blue,
    Cyan,
}

impl MarkerColor {
    pub const ALL: [MarkerColor; 8] = [
        MarkerColor::Green,
        MarkerColor::Red,
        MarkerColor::Purple,
        MarkerColor::Orange,
        MarkerColor::Yellow,
        MarkerColor::White,
        MarkerColor::Blue,
        MarkerColor::Cyan,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarkerId(pub(crate) u64);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marker {
    pub at: i64,
    pub length: i64,
    pub name: String,
    pub note: String,
    pub color: MarkerColor,
}

impl Marker {
    #[must_use]
    pub fn new(at: i64) -> Marker {
        Marker {
            at,
            ..Marker::default()
        }
    }

    #[must_use]
    pub fn end(&self) -> i64 {
        self.at + self.length
    }

    pub(crate) fn checked(self) -> Marker {
        Marker {
            at: self.at.max(0),
            length: self.length.max(0),
            ..self
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Markers {
    entries: Vec<(MarkerId, Marker)>,
}

impl Markers {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (MarkerId, &Marker)> {
        self.entries.iter().map(|(id, marker)| (*id, marker))
    }

    #[must_use]
    pub fn get(&self, id: MarkerId) -> Option<&Marker> {
        self.entries
            .iter()
            .find(|(marker_id, _)| *marker_id == id)
            .map(|(_, marker)| marker)
    }

    pub(crate) fn put(&mut self, id: MarkerId, marker: Marker) {
        self.entries.retain(|(marker_id, _)| *marker_id != id);
        let at = self
            .entries
            .partition_point(|(marker_id, other)| (other.at, *marker_id) < (marker.at, id));
        self.entries.insert(at, (id, marker));
    }

    pub(crate) fn take(&mut self, id: MarkerId) -> Option<Marker> {
        let at = self
            .entries
            .iter()
            .position(|(marker_id, _)| *marker_id == id)?;
        Some(self.entries.remove(at).1)
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(crate) fn shift_from(&mut self, frame: i64, by: i64) {
        for (_, marker) in &mut self.entries {
            if marker.at >= frame {
                marker.at = (marker.at + by).max(0);
            }
        }
        self.entries.sort_by_key(|(id, marker)| (marker.at, *id));
    }

    pub(crate) fn remapped(&self, mut change: impl FnMut(&Marker) -> Marker) -> Markers {
        let mut markers = Markers::default();
        for (id, marker) in &self.entries {
            markers.put(*id, change(marker).checked());
        }
        markers
    }
}
