use rikka_types::{Key, Modifiers, Point, PointerButton, Rect, Size};
use rikka_ui::text::{Label, TextStyled, text};
use rikka_ui::{
    Align, Behavior, Element, Env, Event, EventCtx, Inset, Layout, Length, PaintCtx, Quad, Style,
    Styled, View, Widget, drop_panel,
};

use crate::control::ControlSize;
use crate::navigation::menu::{
    Cascade, DEFAULT_WIDTH, Keyed, Laid, MenuItem, Panel, Pointed, Press, leaf, slide,
};

const DROP: f32 = 4.0;
const TITLE_INSET: f32 = 3.0;

#[must_use]
pub struct MenuBar {
    menus: Vec<BarMenu>,
    size: ControlSize,
    style: Style,
}

struct BarMenu {
    title: Label,
    items: Vec<MenuItem>,
}

pub fn menu_bar() -> MenuBar {
    MenuBar {
        menus: Vec::new(),
        size: ControlSize::Small,
        style: Style::default(),
    }
}

impl MenuBar {
    pub fn menu(
        mut self,
        title: impl Into<Label>,
        items: impl IntoIterator<Item = MenuItem>,
    ) -> Self {
        self.menus.push(BarMenu {
            title: title.into(),
            items: items.into_iter().collect(),
        });
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    fn padding(&self) -> f32 {
        self.size.padding_x() / 2.0
    }

    fn panel(&self, menu: usize) -> Option<Panel<'_>> {
        self.menus
            .get(menu)
            .map(|menu| Panel::new(&menu.items, self.size))
    }

    fn laid(&self, state: &mut MenuBarState, ctx: &EventCtx<'_>) -> Laid<'_> {
        let Some(panel) = self.panel(state.shown) else {
            return Vec::new();
        };
        let dropped = state.dropped.translate(ctx.bounds().origin);
        state
            .cascade
            .lay(panel, dropped, ctx.theme(), ctx.viewport())
    }

    fn open(&self, state: &mut MenuBarState, menu: usize, ctx: &mut EventCtx<'_>) {
        state.phase = Phase::Open;
        state.hovered = None;
        self.show(state, menu, ctx);
        ctx.capture_pointer_exclusively();
        ctx.request_focus();
    }

    fn show(&self, state: &mut MenuBarState, menu: usize, ctx: &mut EventCtx<'_>) {
        state.shown = menu;
        state.cascade.relist();
        ctx.request_paint();
    }

    fn fold(&self, state: &mut MenuBarState, phase: Phase, ctx: &mut EventCtx<'_>) {
        state.phase = phase;
        state.cascade.root_mut().highlight(None);
        ctx.request_paint();
    }

    fn close(&self, state: &mut MenuBarState, ctx: &mut EventCtx<'_>) {
        self.fold(state, Phase::Closed, ctx);
        ctx.release_pointer();
        ctx.return_focus();
    }

    fn run(&self, item: Option<&MenuItem>, ctx: &mut EventCtx<'_>) {
        if let Some(item) = item {
            item.pressed(ctx);
        }
    }

    fn pressed(
        &self,
        state: &mut MenuBarState,
        event: &Event,
        (button, at): (PointerButton, Point),
        ctx: &mut EventCtx<'_>,
    ) {
        let title = state.title_at(at - ctx.bounds().origin);
        if state.phase != Phase::Open {
            match (button, title) {
                (PointerButton::Primary, Some(menu)) => {
                    ctx.set_handled();
                    self.open(state, menu, ctx);
                }
                _ => ctx.keep_focus(),
            }
            return;
        }
        ctx.set_handled();
        match title {
            Some(menu) if menu == state.shown => self.fold(state, Phase::Releasing, ctx),
            Some(menu) => self.show(state, menu, ctx),
            None => {
                let laid = self.laid(state, ctx);
                let Some(Pointed::Pressed(press)) = state.cascade.pointer(&laid, event) else {
                    return;
                };
                match press {
                    Press::Away => self.fold(state, Phase::Releasing, ctx),
                    Press::Leaf { level, row } if button == PointerButton::Primary => {
                        self.fold(state, Phase::Releasing, ctx);
                        self.run(leaf(&laid, level, row), ctx);
                    }
                    Press::Slide { level, row, value } if button == PointerButton::Primary => {
                        slide(&laid, (level, row), value, ctx);
                        ctx.request_paint();
                    }
                    Press::Opened => ctx.request_paint(),
                    Press::Inert | Press::Leaf { .. } | Press::Slide { .. } => {}
                }
            }
        }
    }

    fn released(&self, state: &mut MenuBarState, event: &Event, ctx: &mut EventCtx<'_>) {
        if state.cascade.is_sliding() {
            let laid = self.laid(state, ctx);
            if state.cascade.pointer(&laid, event) == Some(Pointed::Changed) {
                ctx.request_paint();
            }
            ctx.set_handled();
            return;
        }
        match state.phase {
            Phase::Open => {}
            Phase::Releasing => self.close(state, ctx),
            Phase::Closed => return,
        }
        ctx.set_handled();
    }

    fn pointed(&self, state: &mut MenuBarState, event: &Event, at: Point, ctx: &mut EventCtx<'_>) {
        let title = state.title_at(at - ctx.bounds().origin);
        if state.phase != Phase::Open {
            if state.hovered != title {
                state.hovered = title;
                ctx.request_paint();
            }
            return;
        }
        ctx.set_handled();
        match title {
            Some(menu) if menu != state.shown && !state.cascade.is_sliding() => {
                self.show(state, menu, ctx);
            }
            Some(_) if !state.cascade.is_sliding() => {}
            _ => {
                let laid = self.laid(state, ctx);
                match state.cascade.pointer(&laid, event) {
                    Some(Pointed::Changed) => ctx.request_paint(),
                    Some(Pointed::Slid { level, row, value }) => {
                        slide(&laid, (level, row), value, ctx);
                        ctx.request_paint();
                    }
                    Some(Pointed::Kept | Pointed::Pressed(_)) | None => {}
                }
            }
        }
    }

    fn keyed(
        &self,
        state: &mut MenuBarState,
        key: Key,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) {
        let count = self.menus.len();
        let laid = self.laid(state, ctx);
        if laid.is_empty() {
            return;
        }
        match state.cascade.key(&laid, key, modifiers) {
            Keyed::Ignored => return,
            Keyed::Kept => {}
            Keyed::Changed => ctx.request_paint(),
            Keyed::Aside { forward: true } => self.show(state, (state.shown + 1) % count, ctx),
            Keyed::Aside { forward: false } => {
                self.show(state, (state.shown + count - 1) % count, ctx);
            }
            Keyed::Choose { level, row } => {
                self.close(state, ctx);
                self.run(leaf(&laid, level, row), ctx);
            }
            Keyed::Slid { level, row, value } => {
                slide(&laid, (level, row), value, ctx);
                ctx.request_paint();
            }
            Keyed::Close => self.close(state, ctx),
        }
        ctx.set_handled();
    }

    fn scrolled(&self, state: &mut MenuBarState, event: &Event, ctx: &mut EventCtx<'_>) {
        let laid = self.laid(state, ctx);
        if !laid.is_empty() && state.cascade.pointer(&laid, event) == Some(Pointed::Changed) {
            ctx.request_paint();
        }
    }

    fn paint_panel(&self, state: &mut MenuBarState, bounds: Rect, ctx: &mut PaintCtx<'_>) {
        let open = state.phase == Phase::Open;
        let reveal = state.cascade.root_mut().reveal(ctx, open);
        if reveal <= 0.0 && !open {
            return;
        }
        let (Some(panel), Some(title)) = (self.panel(state.shown), state.titles.get(state.shown))
        else {
            return;
        };
        let width = panel.natural_width(ctx.theme()).max(DEFAULT_WIDTH);
        let dropped = drop_panel(
            title.translate(bounds.origin),
            DROP,
            Size::new(width, panel.total_height()),
            ctx.viewport(),
        );
        state.dropped = dropped.rect().translate(-bounds.origin);
        if reveal <= 0.0 {
            return;
        }
        let laid = state
            .cascade
            .lay(panel, dropped.rect(), ctx.theme(), ctx.viewport());
        panel.paint(
            ctx,
            state.cascade.root_mut(),
            dropped.rect(),
            reveal,
            dropped.pivot(),
            |_, _| {},
        );
        state.cascade.paint_branches(ctx, &laid, reveal);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Closed,
    Open,
    Releasing,
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct MenuBarState {
    phase: Phase,
    shown: usize,
    hovered: Option<usize>,
    titles: Vec<Rect>,
    dropped: Rect,
    cascade: Cascade,
}

impl MenuBarState {
    fn title_at(&self, at: Point) -> Option<usize> {
        self.titles.iter().position(|title| title.contains(at))
    }
}

impl Widget for MenuBar {
    type State = MenuBarState;

    fn layout(&self) -> Layout {
        let padding = self.padding();
        Layout::row()
            .align_items(Align::Center)
            .gap_exact(padding * 2.0, 0.0)
            .padding(Inset::xy(padding, 0.0))
            .size(Length::HUG, Length::px(self.size.height()))
            .fixed()
            .with(&self.style)
    }

    fn surroundings(&self, env: &mut Env) {
        let theme = rikka_ui::theme();
        env.text_color = theme.palette().text;
        env.font_size = self.size.font_size(&theme);
    }

    fn paint(&self, state: &mut MenuBarState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let padding = self.padding();
        let height = (bounds.height() - TITLE_INSET * 2.0).max(0.0);
        let titles = (0..self.menus.len())
            .filter_map(|menu| ctx.child_rect(menu))
            .map(|title| {
                Rect::new(
                    title.x() - bounds.x() - padding,
                    TITLE_INSET,
                    title.width() + padding * 2.0,
                    height,
                )
            });
        state.titles.clear();
        state.titles.extend(titles);

        let lit = match state.phase {
            Phase::Open => Some((state.shown, ctx.theme().press_layer())),
            Phase::Closed | Phase::Releasing => {
                state.hovered.map(|menu| (menu, ctx.theme().hover_layer()))
            }
        };
        if let Some((menu, fill)) = lit
            && let Some(title) = state.titles.get(menu)
        {
            let radius = self.size.radius();
            ctx.draw_quad(
                Quad::new(title.translate(bounds.origin))
                    .fill(fill)
                    .radius(radius),
            );
        }
        self.paint_panel(state, bounds, ctx);
    }

    fn event(&self, state: &mut MenuBarState, event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerMoved(position) => self.pointed(state, event, position, ctx),
            Event::PointerExited => {
                if state.phase == Phase::Closed && state.hovered.take().is_some() {
                    ctx.request_paint();
                }
            }
            Event::PointerPressed {
                button, position, ..
            } => self.pressed(state, event, (button, position), ctx),
            Event::PointerReleased { .. } | Event::PointerCancelled => {
                self.released(state, event, ctx);
            }
            Event::Scroll { .. } if state.phase == Phase::Open => {
                ctx.set_handled();
                self.scrolled(state, event, ctx);
            }
            Event::KeyPressed { key, modifiers, .. }
                if state.phase == Phase::Open && ctx.focused() =>
            {
                self.keyed(state, key, modifiers, ctx);
            }
            Event::FocusLost if state.phase != Phase::Closed => self.close(state, ctx),
            _ => {}
        }
    }

    fn behavior(&self, state: &MenuBarState) -> Behavior {
        Behavior::OVERLAY.when(state.phase != Phase::Closed || state.cascade.root().shown() > 0.0)
            | Behavior::TRACKS_HOVER
            | Behavior::FOCUSABLE
            | Behavior::NO_TAB_STOP
    }
}

impl std::fmt::Debug for MenuBar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.menus.iter().map(|menu| &menu.title))
            .finish()
    }
}

impl View for MenuBar {
    fn build(self) -> Element {
        let titles = self
            .menus
            .iter()
            .map(|menu| text(menu.title.clone()).no_wrap().build())
            .collect();
        Element::with_children(self, titles)
    }
}

impl Styled for MenuBar {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
