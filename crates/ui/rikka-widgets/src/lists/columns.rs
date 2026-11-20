use std::cell::RefCell;
use std::rc::Rc;

use rikka_ui::{
    Action, Behavior, Event, EventCtx, IconGlyph, IntoHandler, MaterialSymbol, PaintCtx,
    PointerButton, Quad, Rect, TextRun, Widget, measure_text, palette,
};

use crate::controls::grab::{Grab, GrabStep};

pub const COLUMN_PADDING: f32 = 10.0;

pub const COLUMN_HEADER: f32 = 30.0;

const GRIP: f32 = 4.0;

const HEADING: f32 = 12.0;

const MEASURE: f32 = 13.0;

const SLACK: f32 = 18.0;

#[derive(Clone, Debug)]
pub struct ColumnSpec {
    heading: String,
    floor: f32,
    ceiling: f32,
    right_aligned: bool,
}

pub fn column_spec(heading: impl Into<String>) -> ColumnSpec {
    ColumnSpec {
        heading: heading.into(),
        floor: 0.0,
        ceiling: f32::INFINITY,
        right_aligned: false,
    }
}

impl ColumnSpec {
    #[must_use]
    pub fn floor(mut self, floor: f32) -> ColumnSpec {
        self.floor = floor;
        self
    }

    #[must_use]
    pub fn ceiling(mut self, ceiling: f32) -> ColumnSpec {
        self.ceiling = ceiling;
        self
    }

    #[must_use]
    pub fn right_aligned(mut self, right_aligned: bool) -> ColumnSpec {
        self.right_aligned = right_aligned;
        self
    }

    #[must_use]
    pub fn heading(&self) -> &str {
        &self.heading
    }

    #[must_use]
    pub fn is_right_aligned(&self) -> bool {
        self.right_aligned
    }
}

#[derive(Clone, Debug, Default)]
pub struct ColumnWidths {
    shared_widths: Rc<RefCell<Vec<f32>>>,
}

impl ColumnWidths {
    #[must_use]
    pub fn new() -> ColumnWidths {
        ColumnWidths::default()
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.shared_widths.borrow().len()
    }

    #[must_use]
    pub fn at(&self, index: usize) -> f32 {
        self.shared_widths
            .borrow()
            .get(index)
            .copied()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn start(&self, index: usize) -> f32 {
        let shared_widths = self.shared_widths.borrow();

        shared_widths[..index.min(shared_widths.len())].iter().sum()
    }

    #[must_use]
    pub fn edge_at(&self, x: f32) -> Option<usize> {
        (0..self.count()).find(|index| (x - self.start(*index) - self.at(*index)).abs() <= GRIP)
    }

    pub fn fit(&self, room: f32, natural: &[f32], specs: &[ColumnSpec]) {
        let mut sized: Vec<f32> = specs
            .iter()
            .zip(natural)
            .map(|(spec, natural)| natural.clamp(spec.floor, spec.ceiling))
            .collect();
        let Some(first) = specs.first() else {
            return;
        };
        let others: f32 = sized[1..].iter().sum();
        sized[0] = (room - others).max(first.floor);
        self.shared_widths.replace(sized);
    }

    pub fn drag(&self, index: usize, width: f32, room: f32, specs: &[ColumnSpec]) {
        let mut shared_widths = self.shared_widths.borrow_mut();
        let Some(spec) = specs.get(index) else {
            return;
        };
        shared_widths[index] = width.max(spec.floor);
        if index != 0 {
            let others: f32 = shared_widths[1..].iter().sum();
            shared_widths[0] = (room - others).max(specs[0].floor);
        }
    }
}

#[derive(Widget)]
pub struct ColumnHeader {
    specs: Vec<ColumnSpec>,
    widths: ColumnWidths,
    #[debug]
    sorted: Option<usize>,
    descending: bool,
    gutter: f32,
    on_sort: Action<(usize,)>,
}

pub fn column_header(
    specs: impl IntoIterator<Item = ColumnSpec>,
    widths: &ColumnWidths,
) -> ColumnHeader {
    ColumnHeader {
        specs: specs.into_iter().collect(),
        widths: widths.clone(),
        sorted: None,
        descending: false,
        gutter: 0.0,
        on_sort: Action::default(),
    }
}

impl ColumnHeader {
    #[must_use]
    pub fn sorted(mut self, sorted: impl Into<Option<usize>>, descending: bool) -> ColumnHeader {
        self.sorted = sorted.into();
        self.descending = descending;
        self
    }

    #[must_use]
    pub fn gutter(mut self, gutter: f32) -> ColumnHeader {
        self.gutter = gutter;
        self
    }

    #[must_use]
    pub fn on_sort<Marker>(
        mut self,
        on_sort: impl IntoHandler<(usize,), (), Marker>,
    ) -> ColumnHeader {
        self.on_sort = Action::new(on_sort);
        self
    }

    fn fit(&self, shared_widths: &mut ColumnHeaderState, ctx: &mut PaintCtx<'_>) {
        let room = ctx.bounds().size.width;
        if shared_widths.grab.is_held() || (shared_widths.sized_for - room).abs() <= 0.5 {
            return;
        }
        let metrics = TextRun::new("", MEASURE).no_wrap();
        let natural: Vec<f32> = self
            .specs
            .iter()
            .enumerate()
            .map(|(index, spec)| {
                let gutter = match index == 0 {
                    true => self.gutter,
                    false => 0.0,
                };
                measure_text(&metrics.clone().content(spec.heading()), None).width
                    + SLACK
                    + COLUMN_PADDING * 2.0
                    + gutter
            })
            .collect();
        self.widths.fit(room, &natural, &self.specs);
        shared_widths.sized_for = room;
    }

    fn sort(&self, x: f32, ctx: &mut EventCtx<'_>) {
        let Some(on_sort) = self.on_sort.handler() else {
            return;
        };
        let column = (0..self.specs.len())
            .rev()
            .find(|index| x >= self.widths.start(*index))
            .unwrap_or_default();
        ctx.call(on_sort, (column,));
    }
}

#[derive(Clone, Copy, Debug)]
struct Resizing {
    index: usize,
    start: f32,
    original: f32,
}

#[derive(Debug, Default)]
pub struct ColumnHeaderState {
    grab: Grab<Resizing>,
    sized_for: f32,
}

impl Widget for ColumnHeader {
    type State = ColumnHeaderState;

    fn paint(&self, shared_widths: &mut ColumnHeaderState, ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let header = ctx.bounds();
        self.fit(shared_widths, ctx);

        ctx.draw_quad(Quad::new(header).fill(palette.surface));
        for (index, spec) in self.specs.iter().enumerate() {
            let start = self.widths.start(index);
            let width = self.widths.at(index);
            let at = Rect::new(
                header.origin.x + start + COLUMN_PADDING,
                header.origin.y + (COLUMN_HEADER - 17.0) / 2.0,
                width - COLUMN_PADDING * 2.0,
                17.0,
            );
            ctx.draw_text(
                at,
                TextRun::new(spec.heading(), HEADING),
                palette.text_muted,
            );
            if self.sorted == Some(index) {
                let arrow = Rect::new(
                    header.origin.x + start + width - 22.0,
                    header.origin.y + 6.0,
                    18.0,
                    18.0,
                );
                let symbol = match self.descending {
                    true => MaterialSymbol::ARROW_DOWNWARD,
                    false => MaterialSymbol::ARROW_UPWARD,
                };
                ctx.draw_icon(arrow, IconGlyph::from(symbol).spec(14.0), palette.accent);
            }
            if index + 1 < self.specs.len() {
                let edge = Rect::new(
                    header.origin.x + start + width - 0.5,
                    header.origin.y + 6.0,
                    1.0,
                    COLUMN_HEADER - 12.0,
                );
                ctx.draw_quad(Quad::new(edge).fill(palette.outline));
            }
        }
        let rule = Rect::new(
            header.origin.x,
            header.max_y() - 1.0,
            header.size.width,
            1.0,
        );
        ctx.draw_quad(Quad::new(rule).fill(palette.outline));
    }

    fn event(&self, shared_widths: &mut ColumnHeaderState, event: &Event, ctx: &mut EventCtx<'_>) {
        match shared_widths.grab.update(event, ctx) {
            Some(GrabStep::Moved { held, to }) => {
                let room = ctx.bounds().size.width;
                let width = held.original + (to.x - held.start);
                self.widths.drag(held.index, width, room, &self.specs);
                ctx.request_paint();
                return;
            }
            Some(GrabStep::Ended { .. } | GrabStep::Cancelled { .. }) => return,
            None => {}
        }
        if let Event::PointerPressed {
            position,
            button: PointerButton::Primary,
            ..
        } = *event
        {
            ctx.set_handled();
            let x = position.x - ctx.bounds().origin.x;
            match self.widths.edge_at(x) {
                Some(index) => shared_widths.grab.begin(
                    Resizing {
                        index,
                        start: position.x,
                        original: self.widths.at(index),
                    },
                    ctx,
                ),
                None => self.sort(x, ctx),
            }
        }
    }

    fn behavior(&self, _held: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER
    }
}
