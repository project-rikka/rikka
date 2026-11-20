use rikka_symbols::IconGlyph;
use rikka_types::{Key, Point, PointerButton, Rect, Size};
use rikka_ui::text::Label;
use rikka_ui::{
    Action, Behavior, Event, EventCtx, IntoHandler, Layout, Length, MeasureSpec, PaintCtx, Style,
    Tooltip, Widget,
};

use crate::control::ControlSize;
use crate::controls::track::{Facet, Track, TrackState};

#[must_use]
#[derive(PartialEq, Widget)]
pub struct Segmented {
    #[debug]
    selected: usize,
    segments: Vec<Label>,
    glyphs: Vec<IconGlyph>,
    labelled: bool,
    size: ControlSize,
    on_select: Action<(usize,)>,
    enabled: bool,
    focusless: bool,
    #[style]
    style: Style,
}

pub fn segmented(selected: usize) -> Segmented {
    Segmented {
        selected,
        segments: Vec::new(),
        glyphs: Vec::new(),
        labelled: false,
        size: ControlSize::default(),
        on_select: Action::default(),
        enabled: true,
        focusless: false,
        style: Style::default(),
    }
}

impl Segmented {
    pub fn segments<S: Into<Label>>(mut self, labels: impl IntoIterator<Item = S>) -> Self {
        self.segments = labels.into_iter().map(Into::into).collect();
        self
    }

    pub fn segment(mut self, label: impl Into<Label>) -> Self {
        self.segments.push(label.into());
        self
    }

    pub fn glyphs<G: Into<IconGlyph>>(mut self, glyphs: impl IntoIterator<Item = G>) -> Self {
        self.glyphs = glyphs.into_iter().map(Into::into).collect();
        self
    }

    pub fn labelled(mut self) -> Self {
        self.labelled = true;
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn on_select<Marker>(mut self, on_select: impl IntoHandler<(usize,), (), Marker>) -> Self {
        self.on_select = Action::new(on_select);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn focusless(mut self) -> Self {
        self.focusless = true;
        self
    }

    fn is_active(&self) -> bool {
        self.on_select.active(self.enabled) && self.segments.len() > 1
    }

    fn facet(&self, index: usize) -> Facet {
        let label = self.segments[index].clone();
        match (self.glyphs.get(index).copied(), self.labelled) {
            (Some(glyph), true) => Facet::Labelled(glyph, label),
            (Some(glyph), false) => Facet::Glyph(glyph),
            (None, _) => Facet::Lettered(label),
        }
    }

    fn track(&self) -> Track {
        Track::new(
            self.size,
            &rikka_ui::theme(),
            (0..self.segments.len()).map(|index| self.facet(index)),
        )
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct SegmentedState {
    track: TrackState,
}

impl Widget for Segmented {
    type State = SegmentedState;

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::px(Track::height(self.size)))
            .fixed()
            .with(&self.style)
    }

    fn measure(&self) -> Option<MeasureSpec> {
        let track = self.track();
        Some(MeasureSpec::Shrinkable {
            least: track.least_width(),
            natural: Size::new(track.natural_width(), Track::height(self.size)),
        })
    }

    fn paint(&self, state: &mut SegmentedState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        self.track().paint(
            ctx,
            &mut state.track,
            bounds,
            |index| self.facet(index),
            self.selected,
            !self.enabled,
        );
    }

    fn event(&self, state: &mut SegmentedState, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        if self.focusless && matches!(event, Event::PointerPressed { .. }) {
            ctx.keep_focus();
        }
        let at = |x: f32| self.track().spans(bounds).at(x);
        match event {
            Event::PointerMoved(position) => {
                let hovered = self.is_active().then(|| at(position.x));
                if state.track.hover(hovered) {
                    ctx.request_paint();
                }
            }
            Event::PointerExited if state.track.hover(None) => ctx.request_paint(),
            _ => {}
        }

        if !self.is_active() {
            return;
        }
        let last = self.segments.len() - 1;
        let selected = self.selected.min(last);
        let next = match event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                ctx.set_handled();
                at(position.x)
            }
            Event::KeyPressed { key, .. } if ctx.focused() => match key {
                Key::ArrowLeft | Key::ArrowUp => selected.saturating_sub(1),
                Key::ArrowRight | Key::ArrowDown => (selected + 1).min(last),
                Key::Home => 0,
                Key::End => last,
                _ => return,
            },
            _ => return,
        };
        ctx.set_handled();
        if next != selected {
            ctx.act(&self.on_select, (next,));
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER | Behavior::FOCUSABLE.when(self.is_active() && !self.focusless)
    }

    fn tooltip_at(
        &self,
        _state: &SegmentedState,
        bounds: Rect,
        point: Point,
    ) -> Option<(Tooltip, Rect)> {
        let track = self.track();
        let spans = track.spans(bounds);
        let index = spans.at(point.x);
        let label = self.segments.get(index)?;
        let iconic = self.glyphs.get(index).is_some() && !self.labelled;
        let hidden = iconic || spans.is_elided(index);
        hidden.then(|| (Tooltip::new(label.clone()), spans.segment(index as f32)))
    }

    fn same(&self, other: &Self) -> bool {
        self == other
    }
}
