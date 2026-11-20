use std::sync::Arc;

use animate_sheet::{Group, GroupKind, Pouring, Sheet};
use rikka_timeline::{Orientation, TimeLane, TimeScale};
use rikka_types::Rect;
use rustc_hash::FxHashSet;

pub(crate) const NAMES_WIDTH: f32 = 220.0;
pub(crate) const RULER_HEIGHT: f32 = 26.0;
pub(crate) const ROW_HEIGHT: f32 = 22.0;
pub(crate) const SOUND_ROW_HEIGHT: f32 = 48.0;
pub(crate) const POUR_ROW_HEIGHT: f32 = 28.0;
pub(crate) const LEADING_INSET: f32 = 12.0;
pub(crate) const SCROLLBAR_SIZE: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Sound,
    Pour,
    Hold(usize),
    Group(usize),
    Track(usize, usize),
}

impl Row {
    pub fn height(self) -> f32 {
        match self {
            Self::Sound => SOUND_ROW_HEIGHT,
            Self::Pour | Self::Hold(_) => POUR_ROW_HEIGHT,
            Self::Group(_) | Self::Track(..) => ROW_HEIGHT,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PourRow {
    pub pouring: Pouring,
    pub offset: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HoldRows {
    pub sheet: Arc<Sheet>,
    pub offset: f32,
}

fn sound_offset(sound: bool) -> f32 {
    match sound {
        true => Row::Sound.height(),
        false => 0.0,
    }
}

pub(crate) fn pour_row(sheet: &Sheet, sound: bool) -> Option<PourRow> {
    let offset = sound_offset(sound);
    sheet.pouring().map(|pouring| PourRow { pouring, offset })
}

pub(crate) fn hold_rows(sheet: &Arc<Sheet>, sound: bool) -> HoldRows {
    let pour = match sheet.pouring() {
        Some(_) => Row::Pour.height(),
        None => 0.0,
    };
    HoldRows {
        sheet: Arc::clone(sheet),
        offset: sound_offset(sound) + pour,
    }
}

pub(crate) fn rows(sheet: &Sheet, open: &FxHashSet<usize>, sound: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    if sound {
        rows.push(Row::Sound);
    }
    if sheet.pouring().is_some() {
        rows.push(Row::Pour);
    }
    rows.extend((0..sheet.holding().len()).map(Row::Hold));
    for (index, group) in sheet.groups().iter().enumerate() {
        rows.push(Row::Group(index));
        if group.opens() && (open.contains(&index) || group.kind.stays_open()) {
            rows.extend((0..group.tracks.len()).map(|track| Row::Track(index, track)));
        }
    }
    rows
}

pub(crate) fn height(rows: &[Row]) -> f32 {
    rows.iter().map(|row| row.height()).sum()
}

pub(crate) fn is_muted(group: &Group) -> bool {
    group.kind == GroupKind::Unmatched
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Time,
    Rows,
}

impl Axis {
    pub fn orientation(self) -> Orientation {
        match self {
            Self::Time => Orientation::Horizontal,
            Self::Rows => Orientation::Vertical,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub bounds: Rect,
    pub scale: TimeScale,
    pub top: f32,
}

impl Geometry {
    pub fn ruler(&self) -> Rect {
        Rect::new(
            self.bounds.x() + NAMES_WIDTH,
            self.bounds.y(),
            (self.bounds.width() - NAMES_WIDTH - SCROLLBAR_SIZE).max(0.0),
            RULER_HEIGHT,
        )
    }

    pub fn keys(&self) -> Rect {
        Rect::new(
            self.bounds.x() + NAMES_WIDTH,
            self.bounds.y() + RULER_HEIGHT,
            (self.bounds.width() - NAMES_WIDTH - SCROLLBAR_SIZE).max(0.0),
            (self.bounds.height() - RULER_HEIGHT - SCROLLBAR_SIZE).max(0.0),
        )
    }

    pub fn scrollbar_track(&self, axis: Axis) -> Rect {
        let keys = self.keys();
        match axis {
            Axis::Time => Rect::new(keys.x(), keys.max_y(), keys.width(), SCROLLBAR_SIZE),
            Axis::Rows => Rect::new(keys.max_x(), keys.y(), SCROLLBAR_SIZE, keys.height()),
        }
    }

    pub fn lane(&self) -> TimeLane {
        TimeLane {
            area: self.keys(),
            origin: self.bounds.x() + NAMES_WIDTH + LEADING_INSET,
            scale: self.scale,
        }
    }

    pub fn zoom(&self) -> f32 {
        self.scale.per_frame() as f32
    }

    pub fn x_of(&self, frame: f32) -> f32 {
        self.lane().x_of(f64::from(frame))
    }

    pub fn frame_at(&self, x: f32) -> f32 {
        self.lane().frame_at(x) as f32
    }

    pub fn frames_shown(&self) -> (f32, f32) {
        let (from, to) = self.lane().frames_shown();
        (from as f32, to as f32)
    }

    pub fn row_top(&self, rows: &[Row], index: usize) -> f32 {
        self.top_at(height(&rows[..index]))
    }

    pub fn top_at(&self, offset: f32) -> f32 {
        self.bounds.y() + RULER_HEIGHT - self.top + offset
    }

    pub fn row_at(&self, rows: &[Row], y: f32) -> Option<usize> {
        let mut top = self.bounds.y() + RULER_HEIGHT - self.top;
        if y < self.bounds.y() + RULER_HEIGHT {
            return None;
        }
        for (index, row) in rows.iter().enumerate() {
            let bottom = top + row.height();
            if y >= top && y < bottom {
                return Some(index);
            }
            top = bottom;
        }
        None
    }
}
