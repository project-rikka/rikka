use rikka_sync::Mutex;
use std::cell::{Cell, Ref, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use paint_document::{Area, Tile};
use rikka_gpu::ComputedFrame;
use rikka_types::Rect;
use rikka_ui::{ComputedImage, ImageSource, Observed, Observers};

use crate::tab::Detail;

#[derive(Clone, Debug)]
pub(crate) struct Piece {
    pub(crate) area: Rect,
    computed: ComputedImage,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Picture(Rc<Inner>);

#[derive(Debug, Default)]
struct Inner {
    state: RefCell<TileState>,
    observers: Observers,
    detail: RefCell<Option<Detail>>,
    wished: Cell<Option<u32>>,
}

#[derive(Debug, Default)]
struct TileState {
    drawn: Vec<Piece>,
    spare: Vec<Piece>,
    previewed: Option<Vec<Piece>>,
    incoming: Option<Incoming>,
    landed: Option<u64>,
}

const SWAP_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug)]
struct Incoming {
    generation: u64,
    pieces: Vec<Piece>,
    frames: Vec<ComputedFrame>,
    since: Instant,
    stage: Stage,
}

#[derive(Debug)]
enum Stage {
    Queued,
    Computing,
    Uploading(Arc<Countdown>),
}

enum Step {
    Wait,
    Offer,
    Swap,
}

pub(crate) struct SwapProgress {
    pub(crate) landed: Option<u64>,
    pub(crate) deadline: Option<Instant>,
}

impl Picture {
    pub(crate) fn same(&self, other: &Picture) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn drawn_by(&self, detail: Detail) {
        *self.0.detail.borrow_mut() = Some(detail);
    }

    pub(crate) fn wish(&self, level: u32) {
        if self.0.wished.replace(Some(level)) == Some(level) {
            return;
        }
        if let Some(detail) = self.0.detail.borrow().as_ref() {
            detail.want(level);
        }
    }

    pub(crate) fn show(&self, generation: u64, tiles: Vec<Tile>) {
        let mut state = self.0.state.borrow_mut();
        if state.drawn.is_empty() {
            refill(&mut state.drawn, tiles);
            state.land(generation);
            drop(state);
            self.0.observers.changed();
            return;
        }
        let (mut pieces, since) = match state.incoming.take() {
            Some(incoming) => (incoming.pieces, incoming.since),
            None => (std::mem::take(&mut state.spare), Instant::now()),
        };
        let frames = tiles.iter().map(|tile| tile.frame.clone()).collect();
        refill(&mut pieces, tiles);
        state.incoming = Some(Incoming {
            generation,
            pieces,
            frames,
            since,
            stage: Stage::Queued,
        });
    }

    pub(crate) fn unchanged(&self, generation: u64) {
        let mut state = self.0.state.borrow_mut();
        match state.incoming.as_mut() {
            Some(incoming) => incoming.generation = generation,
            None => state.land(generation),
        }
    }

    pub(crate) fn clear(&self) {
        let mut state = self.0.state.borrow_mut();
        state.drawn.clear();
        state.spare.clear();
        state.incoming = None;
        drop(state);
        self.0.observers.changed();
    }

    pub(crate) fn incoming(&self) -> Ref<'_, [Piece]> {
        self.0.observers.read();
        Ref::map(self.0.state.borrow(), |state| {
            state
                .incoming
                .as_ref()
                .filter(|incoming| incoming.is_offered())
                .map_or(&[][..], |incoming| incoming.pieces.as_slice())
        })
    }

    pub(crate) fn advance_swap(
        &self,
        visible_area: Rect,
        resume: impl FnOnce() + Send + 'static,
    ) -> SwapProgress {
        let mut state = self.0.state.borrow_mut();
        let Some(incoming) = state.incoming.as_mut() else {
            return SwapProgress {
                landed: state.landed.take(),
                deadline: None,
            };
        };
        let shown = match incoming.advance(visible_area, resume) {
            Step::Wait => false,
            Step::Offer => true,
            Step::Swap => {
                state.swap();
                true
            }
        };
        let progress = SwapProgress {
            landed: state.landed.take(),
            deadline: state
                .incoming
                .as_ref()
                .filter(|incoming| incoming.is_offered())
                .map(|incoming| incoming.since + SWAP_TIMEOUT),
        };
        drop(state);
        if shown {
            self.0.observers.changed();
        }
        progress
    }

    pub(crate) fn preview(&self, tiles: Vec<Tile>) {
        let mut state = self.0.state.borrow_mut();
        refill(state.previewed.get_or_insert_with(Vec::new), tiles);
        drop(state);
        self.0.observers.changed();
    }

    pub(crate) fn unpreview(&self) {
        if self.0.state.borrow_mut().previewed.take().is_some() {
            self.0.observers.changed();
        }
    }

    pub(crate) fn pieces(&self) -> Ref<'_, Vec<Piece>> {
        self.0.observers.read();
        Ref::map(self.0.state.borrow(), |state| {
            state.previewed.as_ref().unwrap_or(&state.drawn)
        })
    }

    pub(crate) fn is_shown(&self) -> bool {
        self.0.observers.read();
        !self.0.state.borrow().drawn.is_empty()
    }
}

impl TileState {
    fn land(&mut self, generation: u64) {
        self.landed = Some(
            self.landed
                .map_or(generation, |landed| landed.max(generation)),
        );
    }

    fn swap(&mut self) {
        let Some(incoming) = self.incoming.take() else {
            return;
        };
        let retired = std::mem::replace(&mut self.drawn, incoming.pieces);
        let areas = self.drawn.iter().map(|piece| piece.area);
        self.spare = match same_areas(&retired, areas) {
            true => retired,
            false => Vec::new(),
        };
        self.land(incoming.generation);
    }
}

fn same_areas(pieces: &[Piece], areas: impl ExactSizeIterator<Item = Rect>) -> bool {
    pieces.len() == areas.len()
        && pieces
            .iter()
            .zip(areas)
            .all(|(piece, area)| piece.area == area)
}

fn refill(pieces: &mut Vec<Piece>, tiles: Vec<Tile>) {
    if !same_areas(pieces, tiles.iter().map(|tile| rect_of(tile.area))) {
        *pieces = tiles
            .iter()
            .map(|tile| Piece::new(rect_of(tile.area)))
            .collect();
    }
    for (piece, tile) in pieces.iter().zip(tiles) {
        piece.computed.show(tile.frame);
    }
}

impl Incoming {
    fn advance(&mut self, visible_area: Rect, resume: impl FnOnce() + Send + 'static) -> Step {
        match &self.stage {
            Stage::Queued => {
                let computed = Countdown::start(self.frames.len(), resume);
                for frame in &self.frames {
                    let computed = Arc::clone(&computed);
                    frame.on_ready(move || computed.arrive());
                }
                self.stage = Stage::Computing;
                Step::Wait
            }
            Stage::Computing if !self.frames.iter().all(ComputedFrame::is_ready) => Step::Wait,
            Stage::Computing if self.is_overdue() => Step::Swap,
            Stage::Computing => {
                let visible: Vec<&Piece> = self
                    .pieces
                    .iter()
                    .filter(|piece| piece.area.intersection(&visible_area).is_some())
                    .collect();
                let uploaded = Countdown::start(visible.len(), resume);
                for piece in visible {
                    let uploaded = Arc::clone(&uploaded);
                    piece.computed.on_uploaded(move || uploaded.arrive());
                }
                self.stage = Stage::Uploading(uploaded);
                Step::Offer
            }
            Stage::Uploading(_) if self.is_overdue() => Step::Swap,
            Stage::Uploading(uploaded) if uploaded.is_done() => Step::Swap,
            Stage::Uploading(_) => Step::Wait,
        }
    }

    fn is_overdue(&self) -> bool {
        self.since.elapsed() >= SWAP_TIMEOUT
    }

    fn is_offered(&self) -> bool {
        matches!(self.stage, Stage::Uploading(_))
    }
}

struct Countdown {
    remaining: AtomicUsize,
    done: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl Countdown {
    fn start(count: usize, done: impl FnOnce() + Send + 'static) -> Arc<Countdown> {
        let countdown = Arc::new(Countdown {
            remaining: AtomicUsize::new(count + 1),
            done: Mutex::new(Some(Box::new(done))),
        });
        countdown.arrive();
        countdown
    }

    fn arrive(&self) {
        if self.remaining.fetch_sub(1, Ordering::AcqRel) == 1
            && let Some(done) = self.done.lock().take()
        {
            done();
        }
    }

    fn is_done(&self) -> bool {
        self.remaining.load(Ordering::Acquire) == 0
    }
}

impl std::fmt::Debug for Countdown {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Countdown")
            .field("remaining", &self.remaining)
            .finish_non_exhaustive()
    }
}

impl Piece {
    fn new(area: Rect) -> Piece {
        Piece {
            area,
            computed: ComputedImage::new(),
        }
    }

    pub(crate) fn source(&self) -> ImageSource {
        ImageSource::Computed(self.computed.clone())
    }
}

impl Observed for Picture {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}

pub(crate) fn rect_of(area: Area) -> Rect {
    Rect::new(
        area.left as f32,
        area.top as f32,
        area.width as f32,
        area.height as f32,
    )
}
