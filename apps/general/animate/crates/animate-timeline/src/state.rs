use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use animate_cast::{LiquidCache, Skeleton, Subject};
use animate_edit::KeyId;
use animate_hold::HoldId;
use animate_sheet::{Lane, Sheet};
use animate_transport::Peaks;
use rikka_timeline::{ScrollExtent, TimeScale, ZoomRange};
use rikka_types::Rect;
use rikka_ui::{Observed, Observers};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::layout::{
    self, Axis, Geometry, HoldRows, LEADING_INSET, NAMES_WIDTH, PourRow, RULER_HEIGHT, Row,
    SCROLLBAR_SIZE,
};

const RESTING_ZOOM: f64 = 4.0;
const ZOOMS: ZoomRange = ZoomRange {
    farthest: 0.05,
    nearest: 48.0,
};

#[derive(Clone, Debug, Default)]
pub struct Timeline(Rc<Inner>);

#[derive(Debug, Default)]
struct Inner {
    state: RefCell<TimelineState>,
    room: Cell<f32>,
    observers: Observers,
    selection: Observers,
}

#[derive(Debug)]
struct TimelineState {
    subject: Option<Subject>,
    sheet: Arc<Sheet>,
    open: FxHashMap<Subject, FxHashSet<usize>>,
    peaks: Option<Arc<Peaks>>,
    length: u32,
    liquid: LiquidCache,
    scale: TimeScale,
    top: f32,
    chosen: Option<Row>,
    hold: Option<HoldId>,
    lane: Option<Lane>,
    picked: FxHashSet<KeyId>,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            subject: None,
            sheet: Arc::default(),
            open: FxHashMap::default(),
            peaks: None,
            length: 0,
            liquid: LiquidCache::default(),
            scale: TimeScale::new(RESTING_ZOOM, ZOOMS),
            top: 0.0,
            chosen: None,
            hold: None,
            lane: None,
            picked: FxHashSet::default(),
        }
    }
}

impl TimelineState {
    fn open_groups(&self) -> Option<&FxHashSet<usize>> {
        self.open.get(&self.subject?)
    }

    fn sync_chosen(&mut self) {
        let Some(lane) = &self.lane else {
            return;
        };
        self.chosen = row_of(&self.sheet, lane);
        if let (Some(Row::Track(group, _)), Some(subject)) = (self.chosen, self.subject) {
            self.open.entry(subject).or_default().insert(group);
        }
    }
}

#[derive(Debug)]
pub struct Board {
    pub sheet: Arc<Sheet>,
    pub rows: Vec<Row>,
    pub peaks: Option<Arc<Peaks>>,
    pub length: u32,
    pub liquid: LiquidCache,
    pub chosen: Option<Row>,
    pub hold: Option<HoldId>,
    pub open: FxHashSet<usize>,
    pub picked: FxHashSet<KeyId>,
}

impl Timeline {
    fn changed(&self) {
        self.0.observers.changed();
    }

    pub fn show(&self, subject: Option<Subject>, sheet: Arc<Sheet>) {
        let mut state = self.0.state.borrow_mut();
        if state.subject == subject && Arc::ptr_eq(&state.sheet, &sheet) {
            return;
        }
        let moved = state.subject != subject;
        if moved {
            state.subject = subject;
            state.top = 0.0;
            state.chosen = None;
            state.hold = None;
            state.lane = None;
            state.picked.clear();
        }
        state.sheet = sheet;
        if !moved {
            state.sync_chosen();
        }
        let keeps_hold = state.hold.is_some_and(|hold| {
            state
                .sheet
                .holding()
                .iter()
                .flat_map(|row| &row.spans)
                .any(|span| span.id == hold)
        });
        if !keeps_hold {
            state.hold = None;
        }
        drop(state);
        self.changed();
        if moved {
            self.0.selection.changed();
        }
    }

    pub fn set_peaks(&self, peaks: Option<Arc<Peaks>>) {
        let mut state = self.0.state.borrow_mut();
        let same = match (&state.peaks, &peaks) {
            (Some(one), Some(two)) => Arc::ptr_eq(one, two),
            (one, two) => one.is_none() && two.is_none(),
        };
        if same {
            return;
        }
        state.peaks = peaks;
        drop(state);
        self.changed();
    }

    pub fn set_length(&self, length: u32) {
        let mut state = self.0.state.borrow_mut();
        if state.length == length {
            return;
        }
        state.length = length;
        state.scale.set_end(f64::from(length));
        drop(state);
        self.changed();
    }

    pub fn set_liquid(&self, liquid: LiquidCache) {
        let mut state = self.0.state.borrow_mut();
        if state.liquid == liquid {
            return;
        }
        state.liquid = liquid;
        drop(state);
        self.changed();
    }

    fn rows(&self) -> Vec<Row> {
        self.0.observers.read();
        let state = self.0.state.borrow();
        let closed = FxHashSet::default();
        let open = state.open_groups().unwrap_or(&closed);
        layout::rows(&state.sheet, open, state.peaks.is_some())
    }

    pub fn board(&self) -> Board {
        self.0.observers.read();
        self.0.selection.read();
        let state = self.0.state.borrow();
        let open = state.open_groups().cloned().unwrap_or_default();
        Board {
            rows: layout::rows(&state.sheet, &open, state.peaks.is_some()),
            sheet: Arc::clone(&state.sheet),
            peaks: state.peaks.clone(),
            length: state.length,
            liquid: state.liquid.clone(),
            chosen: state.chosen,
            hold: state.hold,
            open,
            picked: state.picked.clone(),
        }
    }

    pub fn pour_row(&self) -> Option<PourRow> {
        self.0.observers.read();
        let state = self.0.state.borrow();
        layout::pour_row(&state.sheet, state.peaks.is_some())
    }

    pub fn hold_rows(&self) -> HoldRows {
        self.0.observers.read();
        let state = self.0.state.borrow();
        layout::hold_rows(&state.sheet, state.peaks.is_some())
    }
    pub fn geometry(&self, bounds: Rect) -> Geometry {
        self.0.observers.read();
        let state = self.0.state.borrow();
        Geometry {
            bounds,
            scale: state.scale,
            top: state.top,
        }
    }

    pub fn measured(&self, bounds: Rect) {
        let width = (bounds.width() - NAMES_WIDTH - LEADING_INSET - SCROLLBAR_SIZE).max(1.0);
        self.0
            .room
            .set((bounds.height() - RULER_HEIGHT - SCROLLBAR_SIZE).max(0.0));
        self.0.state.borrow_mut().scale.set_width(width);
    }

    pub fn extent(&self, axis: Axis) -> ScrollExtent {
        self.0.observers.read();
        match axis {
            Axis::Time => self.0.state.borrow().scale.extent(),
            Axis::Rows => ScrollExtent {
                at: f64::from(self.0.state.borrow().top),
                shown: f64::from(self.0.room.get()),
                content: f64::from(layout::height(&self.rows())),
            },
        }
    }

    pub fn slide(&self, axis: Axis, at: f64) {
        let before = self.spot();
        let mut state = self.0.state.borrow_mut();
        match axis {
            Axis::Time => state.scale.scroll_to(at),
            Axis::Rows => state.top = at as f32,
        }
        drop(state);
        self.clamp_top();
        if self.spot() != before {
            self.changed();
        }
    }

    pub fn fit(&self) {
        self.0.state.borrow_mut().scale.fit();
        self.changed();
    }

    pub fn toggle(&self, group: usize) {
        let mut state = self.0.state.borrow_mut();
        let Some(subject) = state.subject else {
            return;
        };
        let open = state.open.entry(subject).or_default();
        if !open.remove(&group) {
            open.insert(group);
        }
        drop(state);
        self.clamp_top();
        self.changed();
    }

    pub fn choose(&self, row: Option<Row>) {
        let mut state = self.0.state.borrow_mut();
        let unheld = !matches!(row, Some(Row::Hold(_))) && state.hold.take().is_some();
        if state.chosen == row && !unheld {
            return;
        }
        state.chosen = row;
        let lane = row.and_then(|row| lane_of(&state.sheet, row));
        let relaned = state.lane != lane;
        state.lane = lane;
        drop(state);
        self.changed();
        if relaned || unheld {
            self.0.selection.changed();
        }
    }

    pub fn choose_lane(&self, lane: Lane) {
        let mut state = self.0.state.borrow_mut();
        if state.lane.as_ref() == Some(&lane) && state.chosen == row_of(&state.sheet, &lane) {
            return;
        }
        state.lane = Some(lane);
        state.sync_chosen();
        drop(state);
        self.reveal_chosen();
        self.changed();
        self.0.selection.changed();
    }

    fn reveal_chosen(&self) {
        let room = self.0.room.get();
        let rows = self.rows();
        let chosen = self.0.state.borrow().chosen;
        let Some(index) = chosen.and_then(|chosen| rows.iter().position(|row| *row == chosen))
        else {
            return;
        };
        let top = layout::height(&rows[..index]);
        let bottom = top + rows[index].height();
        let mut state = self.0.state.borrow_mut();
        if top < state.top {
            state.top = top;
        } else if bottom > state.top + room {
            state.top = (bottom - room).max(0.0);
        }
        drop(state);
        self.clamp_top();
    }

    pub fn chosen_bone(&self, skeleton: &Skeleton) -> Option<usize> {
        match self.chosen_lane()? {
            Lane::Bone(name) => skeleton.find(&name),
            Lane::Morph(_)
            | Lane::Camera
            | Lane::Part(_)
            | Lane::Look(_)
            | Lane::Fluid(_)
            | Lane::Wind(_) => None,
        }
    }

    pub fn chosen_lane(&self) -> Option<Lane> {
        self.0.selection.read();
        self.0.state.borrow().lane.clone()
    }

    pub fn chosen_hold(&self) -> Option<HoldId> {
        self.0.selection.read();
        self.0.state.borrow().hold
    }

    pub fn choose_hold(&self, lane: usize, hold: Option<HoldId>) {
        self.choose(Some(Row::Hold(lane)));
        let mut state = self.0.state.borrow_mut();
        if state.hold == hold {
            return;
        }
        state.hold = hold;
        drop(state);
        self.changed();
        self.0.selection.changed();
    }

    pub fn follow_hold(&self, hold: HoldId) {
        let lane = self
            .0
            .state
            .borrow()
            .sheet
            .holding()
            .iter()
            .position(|row| row.spans.iter().any(|span| span.id == hold));
        match lane {
            Some(lane) => self.choose_hold(lane, Some(hold)),
            None => self.forget_hold(),
        }
    }

    fn forget_hold(&self) {
        if self.0.state.borrow_mut().hold.take().is_some() {
            self.changed();
            self.0.selection.changed();
        }
    }

    pub fn has_picks(&self) -> bool {
        self.0.selection.read();
        !self.0.state.borrow().picked.is_empty()
    }

    pub fn picked(&self) -> FxHashSet<KeyId> {
        self.0.selection.read();
        self.0.state.borrow().picked.clone()
    }

    pub fn pick(&self, picked: FxHashSet<KeyId>) {
        let mut state = self.0.state.borrow_mut();
        if state.picked == picked {
            return;
        }
        state.picked = picked;
        drop(state);
        self.0.selection.changed();
    }

    pub fn zoom_notches(&self, notches: f32, x: f32, bounds: Rect) {
        let around = x - bounds.x() - NAMES_WIDTH - LEADING_INSET;
        self.0
            .state
            .borrow_mut()
            .scale
            .zoom_notches(notches, around);
        self.changed();
    }

    pub fn zoom_by(&self, factor: f64) {
        let mut state = self.0.state.borrow_mut();
        let middle = (state.scale.width() / 2.0) as f32;
        state.scale.zoom_at(factor, middle);
        drop(state);
        self.changed();
    }

    pub fn scroll(&self, x: f32, y: f32) {
        let before = self.spot();
        let mut state = self.0.state.borrow_mut();
        state.scale.scroll_by(x);
        state.top += y;
        drop(state);
        self.clamp_top();
        if self.spot() != before {
            self.changed();
        }
    }

    fn spot(&self) -> (f64, f32) {
        let state = self.0.state.borrow();
        (state.scale.first(), state.top)
    }

    pub fn follow(&self, frame: u32) {
        let before = self.spot();
        self.0.state.borrow_mut().scale.follow(f64::from(frame));
        if self.spot() != before {
            self.changed();
        }
    }

    fn clamp_top(&self) {
        let height = self.0.room.get();
        let content = layout::height(&self.rows());
        let mut state = self.0.state.borrow_mut();
        state.top = state.top.clamp(0.0, (content - height).max(0.0));
    }
}

fn row_of(sheet: &Sheet, lane: &Lane) -> Option<Row> {
    sheet
        .groups()
        .iter()
        .enumerate()
        .find_map(|(index, group)| match group.lane.as_ref() == Some(lane) {
            true => Some(Row::Group(index)),
            false => group
                .tracks
                .iter()
                .position(|track| &track.lane == lane)
                .map(|track| Row::Track(index, track)),
        })
}

fn lane_of(sheet: &Sheet, row: Row) -> Option<Lane> {
    match row {
        Row::Track(group, track) => {
            Some(sheet.groups().get(group)?.tracks.get(track)?.lane.clone())
        }
        Row::Group(group) => sheet.groups().get(group)?.lane.clone(),
        Row::Sound | Row::Pour | Row::Hold(_) => None,
    }
}

impl PartialEq for Timeline {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Observed for Timeline {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}
