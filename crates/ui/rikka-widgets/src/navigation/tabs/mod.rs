mod drag;
mod strip;
mod transfer;

use std::any::Any;

use rikka_text::IconSpec;
use rikka_types::{Color, Key, Point, PointerButton, Rect, Size};
use rikka_ui::{
    Action, Axis, Behavior, DRAG_SLOP, Event, EventCtx, IntoHandler, Layout, Length, PaintCtx,
    Quad, Style, Widget, motion,
};

use rikka_symbols::{IconGlyph, MaterialSymbol};

use crate::control::ControlSize;
use crate::lists::collection::KeyboardWalk;
use crate::navigation::title_bar::CAPTION_HEIGHT;
use crate::rows;
use rikka_ui::motion::indicator::{self, Indicator};
use rikka_ui::scroll::bar::Scroll;
use rikka_ui::text::{Label, TextRun};

use strip::{PADDING, Strip, TabEntrance};
use transfer::{Dragging, PendingDrag, Places, TransferPolicy};

pub use transfer::Transfer;
pub(crate) use transfer::{mime as group_mime, payload_handler};

const SEPARATOR_HEIGHT: f32 = 0.55;

#[must_use]
#[derive(Clone)]
pub struct Tab {
    label: Label,
    icon: Option<IconGlyph>,
    tint: Option<Color>,
    closable: bool,
    enabled: bool,
}

pub fn tab(label: impl Into<Label>) -> Tab {
    Tab {
        label: label.into(),
        icon: None,
        tint: None,
        closable: true,
        enabled: true,
    }
}

impl Tab {
    pub fn icon(mut self, glyph: impl Into<IconGlyph>) -> Self {
        self.icon = Some(glyph.into());
        self
    }

    pub fn tint(mut self, color: Color) -> Self {
        self.tint = Some(color);
        self
    }

    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn selectable(&self) -> bool {
        self.enabled
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spot {
    Tab(usize),
    Close(usize),
    New,
    Back,
    Forward,
}

impl Spot {
    fn tab(self) -> Option<usize> {
        match self {
            Self::Tab(index) | Self::Close(index) => Some(index),
            Self::New | Self::Back | Self::Forward => None,
        }
    }
}
#[must_use]
#[derive(Widget)]
pub struct Tabs {
    #[debug]
    selected: usize,
    tabs: Vec<Tab>,
    size: Option<ControlSize>,
    in_caption: bool,
    moving: TransferPolicy,
    on_select: Action<(usize,)>,
    on_close: Action<(usize,)>,
    on_new: Action,
    on_settled: Action,
    #[style]
    style: Style,
}

pub fn tabs(selected: usize) -> Tabs {
    Tabs {
        selected,
        tabs: Vec::new(),
        size: None,
        in_caption: false,
        moving: TransferPolicy::Stays,
        on_select: Action::default(),
        on_close: Action::default(),
        on_new: Action::default(),
        on_settled: Action::default(),
        style: Style::default(),
    }
}

impl Tabs {
    pub fn items(mut self, tabs: impl IntoIterator<Item = Tab>) -> Self {
        self.tabs = tabs.into_iter().collect();
        self
    }

    pub fn item(mut self, tab: Tab) -> Self {
        self.tabs.push(tab);
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = Some(size);
        self
    }

    pub fn in_caption(mut self) -> Self {
        self.in_caption = true;
        self
    }

    pub fn on_select<Marker>(mut self, on_select: impl IntoHandler<(usize,), (), Marker>) -> Self {
        self.on_select = Action::new(on_select);
        self
    }

    pub fn on_close<Marker>(mut self, on_close: impl IntoHandler<(usize,), (), Marker>) -> Self {
        self.on_close = Action::new(on_close);
        self
    }

    pub fn on_new<Marker>(mut self, on_new: impl IntoHandler<(), (), Marker>) -> Self {
        self.on_new = Action::new(on_new);
        self
    }

    pub fn transfers<T: Any>(mut self, transfer: Transfer<T>) -> Self {
        self.moving = TransferPolicy::Transfers(transfer.into_moves());
        self
    }

    pub fn accepting<T: Any, Marker>(
        mut self,
        group: impl Into<String>,
        land: impl IntoHandler<(usize, T), (), Marker>,
    ) -> Self {
        self.moving = TransferPolicy::Accepts {
            group: group.into(),
            land: transfer::payload_handler(land),
        };
        self
    }

    pub fn on_finished<Marker>(mut self, on_finished: impl IntoHandler<(), (), Marker>) -> Self {
        self.on_settled = Action::new(on_finished);
        self
    }

    fn step(&self) -> ControlSize {
        self.size.unwrap_or(if self.in_caption {
            ControlSize::Small
        } else {
            ControlSize::default()
        })
    }

    fn mime(&self) -> Option<String> {
        self.moving.group().map(transfer::mime)
    }

    fn slots(&self, state: &TabsState, drag_room: bool) -> usize {
        let gap = drag_room && state.dragging.is_none();
        self.tabs.len() + usize::from(gap)
    }

    fn strip(&self, state: &TabsState, bounds: Rect, offset: f32) -> Strip {
        self.laid_out(
            state,
            bounds,
            offset,
            self.slots(state, state.landing.is_some()),
        )
    }

    fn laid_out(&self, state: &TabsState, bounds: Rect, offset: f32, slots: usize) -> Strip {
        Strip::new(
            bounds,
            self.step(),
            slots,
            self.on_new.is_set(),
            self.in_caption,
            offset,
            state.opening,
        )
    }

    fn close_rect(&self, strip: &Strip, index: usize, at: f32) -> Option<Rect> {
        let tab = self.tabs.get(index)?;
        (tab.closable && self.on_close.is_set()).then(|| strip.close_rect(at))
    }

    fn spot_at(&self, state: &TabsState, bounds: Rect, point: Point) -> Option<Spot> {
        let strip = self.strip(state, bounds, state.scroll.rendered());
        if !strip.band().contains(point) {
            return None;
        }
        for (rect, spot) in [
            (strip.new_rect(), Spot::New),
            (strip.back_rect(), Spot::Back),
            (strip.forward_rect(), Spot::Forward),
        ] {
            if rect.is_some_and(|rect| rect.contains(point)) {
                return Some(spot);
            }
        }
        let index = strip.index_at(point)?;
        if self
            .close_rect(&strip, index, index as f32)
            .is_some_and(|close| close.contains(point))
        {
            return Some(Spot::Close(index));
        }
        Some(Spot::Tab(index))
    }

    fn selectable(&self, index: usize) -> bool {
        self.tabs.get(index).is_some_and(Tab::selectable)
    }

    fn selection(&self) -> Option<usize> {
        Some(self.selected).filter(|index| self.selectable(*index))
    }

    fn walk(&self, key: Key) -> Option<Option<usize>> {
        KeyboardWalk::strip(self.tabs.len())
            .to(key, self.selection(), |index| self.selectable(index))
    }

    fn close(&self, index: usize, ctx: &mut EventCtx<'_>) {
        if let Some(on_close) = self.on_close.handler()
            && self.tabs.get(index).is_some_and(|tab| tab.closable)
        {
            ctx.call(on_close, (index,));
        }
    }

    fn scroll_by(&self, state: &mut TabsState, bounds: Rect, travel: f32) -> bool {
        let strip = self.strip(state, bounds, state.scroll.target());
        state.scroll.nudge(travel, strip.max_offset())
    }

    fn scrolled(&self, state: &mut TabsState, ctx: &mut PaintCtx<'_>, bounds: Rect) -> Strip {
        self.arriving(state, ctx);
        let resting = self.strip(state, bounds, state.scroll.target());
        state.scroll.aim(resting.offset());
        if state.revealed != Some(self.selected) {
            state.revealed = Some(self.selected);
            if self.selected < self.tabs.len() {
                state.scroll.aim(resting.reveal(self.selected));
            }
        }
        let offset = state
            .scroll
            .advance(ctx, Axis::Horizontal, resting.max_offset());
        self.strip(state, bounds, offset)
    }

    fn arriving(&self, state: &mut TabsState, ctx: &mut PaintCtx<'_>) {
        let count = self.tabs.len();
        if state.count != count {
            let added = state.count + 1 == count && state.count > 0;
            state.opening = added.then(|| TabEntrance {
                index: if self.selected != state.selected && self.selected < count {
                    self.selected
                } else {
                    count - 1
                },
                grown: 0.0,
            });
            ctx.restart("growth");
            state.count = count;
        }
        state.selected = self.selected;
        let Some(opening) = &mut state.opening else {
            return;
        };
        opening.grown = ctx.play("growth", motion::ARRIVAL);
        if opening.grown >= 1.0 {
            state.opening = None;
        }
    }

    fn paint_tabs(
        &self,
        state: &mut TabsState,
        ctx: &mut PaintCtx<'_>,
        strip: &Strip,
        places: Places,
    ) {
        let palette = ctx.theme().palette();
        let (radius, font_size, hover_layer) = {
            let theme = ctx.theme();
            (
                self.step().radius(),
                self.step().font_size(theme),
                theme.hover_layer(),
            )
        };

        let selected = self.selection();
        let hovered = state
            .hot
            .and_then(Spot::tab)
            .filter(|index| self.selectable(*index) && selected != Some(*index));
        let hovering = state.hover.resolve(
            ctx,
            "hover",
            hovered.map(|index| (index, places.slot_position(index))),
            hover_layer,
            motion::INDICATOR,
        );
        if let Some((at, fill)) = hovering {
            ctx.draw_quad(Quad::new(strip.tab_rect(at)).fill(fill).radius(radius));
        }

        let resting = selected
            .filter(|index| places.dragged != Some(*index))
            .map(|index| (index, places.slot_position(index)));
        let selection = state.selection.resolve(
            ctx,
            "selection",
            resting,
            palette.selection,
            motion::INDICATOR,
        );
        if let Some((at, fill)) = selection {
            ctx.draw_quad(Quad::new(strip.tab_rect(at)).fill(fill).radius(radius));
        }

        let raised = selection.map(|(at, _)| at);
        let lit = |index: usize| {
            raised.map_or(0.0, |at| {
                indicator::emphasis(at, places.slot_position(index), 1.0)
            })
        };
        let busy = |index: usize| {
            let hover = hovering.map_or(0.0, |(at, _)| {
                indicator::emphasis(at, places.slot_position(index), 1.0)
            });
            lit(index).max(hover)
        };

        let band = strip.band();
        let height = band.height() * SEPARATOR_HEIGHT;
        if places.is_at_rest() {
            for boundary in strip.visible().filter(|boundary| *boundary > 0) {
                let clear = 1.0 - busy(boundary - 1).max(busy(boundary));
                if clear <= 0.0 {
                    continue;
                }
                let x = strip.tab_rect(boundary as f32).x();
                ctx.draw_quad(
                    Quad::new(Rect::new(x, band.center().y - height / 2.0, 1.0, height))
                        .fill(palette.outline.scale_alpha(clear)),
                );
            }
        }

        let hot_close = match state.hot {
            Some(Spot::Close(index)) => Some(index),
            _ => None,
        };
        if hot_close.is_some() {
            state.closing = hot_close;
        }
        let close_layer = ctx.fade("close", hot_close.map(|_| hover_layer), motion::HOVER);

        let inset = self.step().gap();
        let icon_size = self.step().icon_size();
        let closing = state.closing;
        let arriving = state.opening;
        let showing = match places.is_at_rest() {
            true => strip.visible(),
            false => 0..self.tabs.len(),
        };
        for index in showing {
            if places.dragged == Some(index) {
                continue;
            }
            let tab = &self.tabs[index];
            let at = places.slot_position(index);
            let rect = strip.tab_rect(at);
            let alpha = match arriving {
                Some(opening) if opening.index == index => opening.grown,
                _ => 1.0,
            };
            if alpha <= 0.0 {
                continue;
            }
            let color = ctx.theme().dim(
                palette.text_muted.mix(palette.on_selection, lit(index)),
                !tab.enabled,
            );
            let hint = ctx.theme().dim(palette.text_muted, !tab.enabled);
            let close = self.close_rect(strip, index, at);

            ctx.with_opacity(alpha, |ctx| {
                let mut x = rect.x() + inset;
                if let Some(glyph) = tab.icon {
                    let tint = tab
                        .tint
                        .map_or(color, |tint| ctx.theme().dim(tint, !tab.enabled));
                    ctx.draw_icon(
                        rows::glyph_rect(x, rect.center().y, icon_size),
                        glyph.spec(icon_size),
                        tint,
                    );
                    x += icon_size + inset;
                }

                if let Some(close) = close {
                    if let Some(fill) = close_layer.filter(|_| closing == Some(index)) {
                        ctx.draw_circle(close.center(), close.width(), fill);
                    }
                    ctx.draw_icon(
                        Rect::from_center(close.center(), Size::new(icon_size, icon_size)),
                        IconSpec::new(MaterialSymbol::CLOSE.codepoint(), icon_size),
                        hint,
                    );
                }

                let end = close.map_or(rect.max_x() - inset, |close| close.min_x());
                let mut run = TextRun::new(tab.label.clone(), font_size).no_wrap();
                if selected == Some(index) {
                    run = run.medium();
                }
                rows::draw_cell(ctx, rect, x, end - x, run, color);
            });
        }
    }

    fn paint_buttons(&self, state: &TabsState, ctx: &mut PaintCtx<'_>, strip: &Strip) {
        let (step, hot) = (self.step(), state.hot);
        for (key, rect, symbol, spot, spent) in [
            (
                "back",
                strip.back_rect(),
                MaterialSymbol::CHEVRON_LEFT,
                Spot::Back,
                strip.offset() <= 0.0,
            ),
            (
                "forward",
                strip.forward_rect(),
                MaterialSymbol::CHEVRON_RIGHT,
                Spot::Forward,
                strip.offset() >= strip.max_offset(),
            ),
            (
                "new",
                strip.new_rect(),
                MaterialSymbol::ADD,
                Spot::New,
                false,
            ),
        ] {
            if let Some(rect) = rect {
                paint_button(ctx, key, rect, symbol, step, hot == Some(spot), spent);
            }
        }
    }
}

fn paint_button(
    ctx: &mut PaintCtx<'_>,
    key: &'static str,
    rect: Rect,
    symbol: MaterialSymbol,
    step: ControlSize,
    hot: bool,
    spent: bool,
) {
    let (radius, layer, color) = {
        let theme = ctx.theme();
        (
            step.radius(),
            theme.hover_layer(),
            theme.dim(theme.palette().text_muted, spent),
        )
    };
    if let Some(fill) = ctx.fade(key, (hot && !spent).then_some(layer), motion::HOVER) {
        ctx.draw_quad(Quad::new(rect).fill(fill).radius(radius));
    }
    let side = step.icon_size();
    ctx.draw_icon(
        Rect::from_center(rect.center(), Size::new(side, side)),
        IconSpec::new(symbol.codepoint(), side),
        color,
    );
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct TabsState {
    selection: Indicator,
    hover: Indicator,
    hot: Option<Spot>,
    closing: Option<usize>,
    scroll: Scroll,
    revealed: Option<usize>,
    count: usize,
    selected: usize,
    opening: Option<TabEntrance>,
    armed: Option<PendingDrag>,
    dragging: Option<Dragging>,
    landing: Option<usize>,
    gap_closed: bool,
}

impl TabsState {
    fn close_gap(&mut self) -> bool {
        self.gap_closed = true;
        self.landing.take().is_some()
    }
}

impl Widget for Tabs {
    type State = TabsState;

    fn layout(&self) -> Layout {
        let height = if self.in_caption {
            CAPTION_HEIGHT
        } else {
            self.step().height() + PADDING * 2.0
        };
        Layout::row()
            .size(Length::FILL, Length::px(height))
            .fixed()
            .with(&self.style)
    }

    fn paint(&self, state: &mut TabsState, ctx: &mut PaintCtx<'_>) {
        if self.tabs.is_empty() && !self.on_new.is_set() && state.landing.is_none() {
            return;
        }
        let bounds = ctx.bounds();
        if std::mem::take(&mut state.gap_closed) {
            ctx.restart("slide");
        }
        let strip = self.scrolled(state, ctx, bounds);
        let places = Places {
            dragged: state
                .dragging
                .filter(|dragging| !dragging.lifted && !dragging.follows)
                .map(|dragging| dragging.index),
            landing: state
                .landing
                .map(|at| ctx.animate_curve("slide", at as f32, motion::INDICATOR)),
        };

        if !self.in_caption {
            let fill = ctx.theme().palette().background;
            let radius = self.step().radius() + PADDING;
            ctx.draw_quad(Quad::new(bounds).fill(fill).radius(radius));
        }
        ctx.clipped(strip.lane(), |ctx| {
            self.paint_tabs(state, ctx, &strip, places);
        });
        self.paint_buttons(state, ctx, &strip);
    }

    fn event(&self, state: &mut TabsState, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        let spot = match event {
            Event::PointerPressed { position, .. } => self.spot_at(state, bounds, *position),
            _ => None,
        };
        if matches!(event, Event::PointerPressed { .. }) && spot.is_none() {
            ctx.keep_focus();
        }
        match event {
            Event::PointerMoved(position) => {
                let hot = self.spot_at(state, bounds, *position);
                if state.hot != hot {
                    state.hot = hot;
                    ctx.request_paint();
                }
                if let Some(armed) = state.armed
                    && position.distance(armed.origin) > DRAG_SLOP
                {
                    self.lift(state, ctx, bounds, armed, *position);
                }
            }
            Event::PointerExited if state.hot.is_some() => {
                state.hot = None;
                ctx.request_paint();
            }
            _ => {}
        }

        match event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                let Some(spot) = spot else {
                    return;
                };
                ctx.set_handled();
                match spot {
                    Spot::Close(index) => self.close(index, ctx),
                    Spot::Tab(index) => {
                        let chosen = self.selectable(index).then_some(index);
                        rows::select(ctx, &self.on_select, self.selection(), chosen);
                        if self.moving.transfer().is_some() && self.selectable(index) {
                            state.armed = Some(PendingDrag {
                                index,
                                origin: *position,
                            });
                            ctx.capture_pointer();
                        }
                    }
                    Spot::New => {
                        ctx.act(&self.on_new, ());
                    }
                    Spot::Back | Spot::Forward => {
                        let tab = self.strip(state, bounds, state.scroll.target()).width();
                        let travel = if spot == Spot::Back { -tab } else { tab };
                        if self.scroll_by(state, bounds, travel) {
                            ctx.request_paint();
                        }
                    }
                }
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => {
                if state.armed.take().is_some() {
                    ctx.release_pointer();
                    ctx.set_handled();
                }
            }
            Event::PointerPressed {
                button: PointerButton::Middle,
                ..
            } => {
                let Some(index) = spot.and_then(Spot::tab) else {
                    return;
                };
                ctx.set_handled();
                self.close(index, ctx);
            }
            Event::Scroll { delta, .. } => {
                if !self.strip(state, bounds, state.scroll.target()).scrolls() {
                    return;
                }
                ctx.set_handled();
                let offset = delta.offset();
                let travel = if offset.x != 0.0 { offset.x } else { offset.y };
                if self.scroll_by(state, bounds, travel) {
                    ctx.request_paint();
                }
            }
            Event::KeyPressed { key, .. } if ctx.focused() => {
                let Some(next) = self.walk(*key) else {
                    return;
                };
                ctx.set_handled();
                rows::select(ctx, &self.on_select, self.selection(), next);
            }
            Event::DragMoved { position, offer } => {
                self.drag_over(state, ctx, bounds, *position, offer);
            }
            Event::DragLeft => {
                if state.close_gap() {
                    ctx.request_paint();
                }
            }
            Event::DragDropped {
                position, offer, ..
            } => {
                self.dropped(state, ctx, bounds, *position, offer);
            }
            Event::DragTaken => self.lifted(state, ctx),
            Event::DragWithin(false) => self.went_outside(state, ctx),
            Event::DragReclaimed => self.reclaimed(ctx),
            Event::DragEnded(end) => self.drag_ended(state, ctx, *end),
            _ => {}
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER | Behavior::FOCUSABLE.when(self.on_select.is_set())
    }
}

impl std::fmt::Debug for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tab")
            .field("label", &self.label)
            .field("closable", &self.closable)
            .finish_non_exhaustive()
    }
}
