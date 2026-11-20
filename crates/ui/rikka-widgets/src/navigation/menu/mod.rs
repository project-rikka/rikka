mod cascade;
mod command;
mod context;
mod panel;
mod slider;
mod typeahead;

use std::ops::RangeInclusive;

use rikka_types::{Key, Modifiers, PointerButton, Rect};
use rikka_ui::text::Label;
use rikka_ui::{
    Action, Behavior, Command, Element, Event, EventCtx, IconSource, IntoHandler, Layout, Length,
    PaintCtx, Style, Styled, View, Widget, pop, send_command, theme,
};

use crate::control::ControlSize;
use crate::navigation::menu::slider::MenuSlider;

pub(crate) use cascade::{Cascade, Keyed, Laid, Pointed, Press, leaf, slide};
pub use command::{command_item, command_label};
pub use context::ContextMenu;
pub(crate) use panel::{DEFAULT_WIDTH, Listing, Panel};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Activation {
    Pointer,
    Keyboard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Item,
    Header,
    Separator,
}

#[must_use]
#[derive(Clone)]
pub struct MenuItem {
    label: Label,
    shortcut: Option<Label>,
    icon: Option<IconSource>,
    kind: Kind,
    submenu: bool,
    children: Vec<MenuItem>,
    selected: bool,
    enabled: bool,
    press: Action,
    command: Option<Command>,
    slider: Option<MenuSlider>,
}

pub fn menu_item(label: impl Into<Label>) -> MenuItem {
    MenuItem {
        label: label.into(),
        shortcut: None,
        icon: None,
        kind: Kind::Item,
        submenu: false,
        children: Vec::new(),
        selected: false,
        enabled: true,
        press: Action::default(),
        command: None,
        slider: None,
    }
}

pub fn menu_slider(label: impl Into<Label>, value: f32, range: RangeInclusive<f32>) -> MenuItem {
    MenuItem {
        slider: Some(MenuSlider::new(value, range)),
        ..menu_item(label)
    }
}

pub fn menu_header(label: impl Into<Label>) -> MenuItem {
    MenuItem {
        kind: Kind::Header,
        ..menu_item(label)
    }
}

pub fn menu_separator() -> MenuItem {
    MenuItem {
        kind: Kind::Separator,
        ..menu_item("")
    }
}

impl MenuItem {
    pub fn shortcut(mut self, shortcut: impl Into<Label>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn no_shortcut(mut self) -> Self {
        self.shortcut = None;
        self
    }

    pub fn icon(mut self, icon: impl Into<IconSource>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
        self.children = items.into_iter().collect();
        self
    }

    pub fn submenu(mut self, submenu: bool) -> Self {
        self.submenu = submenu;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn on_press<Marker>(mut self, press: impl IntoHandler<(), (), Marker>) -> Self {
        self.press = Action::new(press);
        self
    }

    pub fn choices<Marker>(
        mut self,
        options: impl IntoIterator<Item = impl Into<Label>>,
        chosen: Option<usize>,
        on_select: impl IntoHandler<(usize,), (), Marker>,
    ) -> Self {
        let select = on_select.into_handler();
        self.children = options
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                menu_item(label)
                    .selected(chosen == Some(index))
                    .on_press(select.clone().map(move |()| (index,)))
            })
            .collect();
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        debug_assert!(self.slider.is_some(), "step applies to a menu slider");
        if let Some(slider) = &mut self.slider {
            slider.step = Some(step);
        }
        self
    }

    pub fn on_change<Marker>(mut self, on_change: impl IntoHandler<(f32,), (), Marker>) -> Self {
        debug_assert!(self.slider.is_some(), "on_change applies to a menu slider");
        if let Some(slider) = &mut self.slider {
            slider.change = Action::new(on_change);
        }
        self
    }

    fn is_command(&self) -> bool {
        self.kind == Kind::Item && self.enabled
    }

    fn opens(&self) -> bool {
        self.kind == Kind::Item && (self.submenu || !self.children.is_empty())
    }

    fn acts(&self) -> bool {
        self.press.is_set()
            || self.command.is_some()
            || self
                .slider
                .as_ref()
                .is_some_and(|slider| slider.change.is_set())
            || self.children.iter().any(MenuItem::acts)
    }

    pub(crate) fn pressed(&self, ctx: &mut EventCtx<'_>) {
        ctx.act(&self.press, ());
        if let Some(command) = self.command {
            send_command(command);
        }
    }

    fn slid(&self, value: f32, ctx: &mut EventCtx<'_>) {
        if let Some(slider) = &self.slider
            && slider.value != value
        {
            ctx.act(&slider.change, (value,));
        }
    }

    fn height(&self, row: f32) -> f32 {
        match self.kind {
            Kind::Item => row,
            Kind::Header => row * 0.9,
            Kind::Separator => panel::SEPARATOR_HEIGHT,
        }
    }
}

#[must_use]
pub struct Menu {
    items: Vec<MenuItem>,
    size: ControlSize,
    still: bool,
    fits_content: bool,
    highlighted: Option<usize>,
    on_activate: Action<(usize,)>,
    on_choose: Action<(usize, Activation)>,
    on_highlight: Action<(Option<usize>,)>,
    on_open: Action<(usize, Rect)>,
    on_dismiss: Action,
    style: Style,
}

pub fn menu() -> Menu {
    Menu {
        items: Vec::new(),
        size: ControlSize::default(),
        still: false,
        fits_content: false,
        highlighted: None,
        on_activate: Action::default(),
        on_choose: Action::default(),
        on_highlight: Action::default(),
        on_open: Action::default(),
        on_dismiss: Action::default(),
        style: Style::default(),
    }
}

impl Menu {
    pub fn items(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
        self.items = items.into_iter().collect();
        self
    }

    pub fn item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn still(mut self) -> Self {
        self.still = true;
        self
    }

    pub fn on_activate<Marker>(
        mut self,
        on_activate: impl IntoHandler<(usize,), (), Marker>,
    ) -> Self {
        self.on_activate = Action::new(on_activate);
        self
    }

    pub fn on_highlight<Marker>(
        mut self,
        on_highlight: impl IntoHandler<(Option<usize>,), (), Marker>,
    ) -> Self {
        self.on_highlight = Action::new(on_highlight);
        self
    }

    pub fn on_open<Marker>(mut self, on_open: impl IntoHandler<(usize, Rect), (), Marker>) -> Self {
        self.on_open = Action::new(on_open);
        self
    }

    pub fn on_dismiss<Marker>(mut self, on_dismiss: impl IntoHandler<(), (), Marker>) -> Self {
        self.on_dismiss = Action::new(on_dismiss);
        self
    }

    pub fn on_choose<Marker>(
        mut self,
        on_choose: impl IntoHandler<(usize, Activation), (), Marker>,
    ) -> Self {
        self.on_choose = Action::new(on_choose);
        self
    }

    pub fn highlighted(mut self, row: Option<usize>) -> Self {
        self.highlighted = row;
        self
    }

    pub fn fit_content(mut self) -> Self {
        self.fits_content = true;
        self
    }

    fn panel(&self) -> Panel<'_> {
        Panel::new(&self.items, self.size)
    }

    fn dismiss(&self, ctx: &mut EventCtx<'_>) {
        ctx.act(&self.on_dismiss, ());
    }

    fn activates(&self) -> bool {
        self.on_activate.is_set()
            || self.on_choose.is_set()
            || self.items.iter().any(MenuItem::acts)
    }

    fn activate(&self, index: usize, activation: Activation, ctx: &mut EventCtx<'_>) {
        if let Some(item) = self.items.get(index) {
            item.pressed(ctx);
        }
        ctx.act(&self.on_activate, (index,));
        ctx.act(&self.on_choose, (index, activation));
    }

    fn choose(
        &self,
        state: &mut MenuState,
        laid: &Laid<'_>,
        (level, row): (usize, usize),
        activation: Activation,
        ctx: &mut EventCtx<'_>,
    ) {
        if level == 0 {
            self.activate(row, activation, ctx);
            return;
        }
        state.cascade.collapse();
        if let Some(item) = leaf(laid, level, row) {
            item.pressed(ctx);
        }
        self.dismiss(ctx);
    }

    fn preselect(&self, state: &mut MenuState, laid: &Laid<'_>) {
        if state.preselected == Some(self.highlighted) {
            return;
        }
        state.preselected = Some(self.highlighted);
        if let Some(row) = self.highlighted {
            state.cascade.preselect(laid, row);
        }
    }

    fn announce(&self, before: Option<usize>, state: &MenuState, ctx: &mut EventCtx<'_>) {
        let next = state.cascade.root().highlighted();
        if next == before {
            return;
        }
        ctx.act(&self.on_highlight, (next,));
        let panel = self.panel();
        if let (Some(on_open), Some(index)) = (self.on_open.handler(), next)
            && panel.opens(next)
        {
            let scrolled = state.cascade.root().scrolled();
            let rect = panel.row_rect(ctx.bounds(), scrolled, index);
            ctx.call(on_open, (index, rect));
        }
    }

    fn keyed(
        &self,
        state: &mut MenuState,
        laid: &Laid<'_>,
        (key, modifiers): (Key, Modifiers),
        ctx: &mut EventCtx<'_>,
    ) {
        match state.cascade.key(laid, key, modifiers) {
            Keyed::Ignored | Keyed::Aside { .. } => return,
            Keyed::Kept => {}
            Keyed::Changed => ctx.request_paint(),
            Keyed::Choose { level, row } => {
                self.choose(state, laid, (level, row), Activation::Keyboard, ctx);
                ctx.request_paint();
            }
            Keyed::Slid { level, row, value } => {
                slide(laid, (level, row), value, ctx);
                ctx.request_paint();
            }
            Keyed::Close => self.dismiss(ctx),
        }
        ctx.set_handled();
    }

    fn pressed(
        &self,
        state: &mut MenuState,
        laid: &Laid<'_>,
        (press, button): (Press, PointerButton),
        ctx: &mut EventCtx<'_>,
    ) {
        let primary = button == PointerButton::Primary;
        match press {
            Press::Away => {
                if primary {
                    ctx.set_handled();
                }
                state.cascade.collapse();
                ctx.request_paint();
                self.dismiss(ctx);
            }
            Press::Inert | Press::Opened if primary => {
                ctx.set_handled();
                ctx.request_paint();
            }
            Press::Leaf { level, row } if primary => {
                ctx.set_handled();
                self.choose(state, laid, (level, row), Activation::Pointer, ctx);
            }
            Press::Slide { level, row, value } if primary => {
                ctx.set_handled();
                slide(laid, (level, row), value, ctx);
                ctx.request_paint();
            }
            Press::Inert | Press::Opened | Press::Leaf { .. } | Press::Slide { .. } => {
                if state.cascade.branched() {
                    ctx.set_handled();
                }
            }
        }
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct MenuState {
    cascade: Cascade,
    preselected: Option<Option<usize>>,
}

impl Widget for Menu {
    type State = MenuState;

    fn layout(&self) -> Layout {
        Layout::row()
            .size(
                Length::px(DEFAULT_WIDTH),
                Length::px(self.panel().total_height()),
            )
            .fixed()
            .with(&self.style)
    }

    fn paint(&self, state: &mut MenuState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let laid = state
            .cascade
            .lay(self.panel(), bounds, ctx.theme(), ctx.viewport());
        self.preselect(state, &laid);
        self.panel().paint(
            ctx,
            state.cascade.root_mut(),
            bounds,
            1.0,
            bounds.center(),
            |_, _| {},
        );
        state.cascade.paint_branches(ctx, &laid, 1.0);
    }

    fn event(&self, state: &mut MenuState, event: &Event, ctx: &mut EventCtx<'_>) {
        let laid = state
            .cascade
            .lay(self.panel(), ctx.bounds(), ctx.theme(), ctx.viewport());
        self.preselect(state, &laid);
        let before = state.cascade.root().highlighted();

        match *event {
            Event::KeyPressed { key, modifiers, .. } if ctx.focused() => {
                self.keyed(state, &laid, (key, modifiers), ctx);
            }
            Event::PointerPressed { button, .. } => {
                if let Some(Pointed::Pressed(press)) = state.cascade.pointer(&laid, event) {
                    self.pressed(state, &laid, (press, button), ctx);
                }
            }
            Event::PointerReleased { .. } | Event::PointerCancelled => {
                if state.cascade.pointer(&laid, event) == Some(Pointed::Changed) {
                    ctx.set_handled();
                    ctx.request_paint();
                }
            }
            Event::PointerMoved(_) | Event::PointerExited | Event::Scroll { .. } => {
                if !matches!(event, Event::PointerExited) {
                    ctx.set_handled();
                }
                match state.cascade.pointer(&laid, event) {
                    Some(Pointed::Changed) => ctx.request_paint(),
                    Some(Pointed::Slid { level, row, value }) => {
                        slide(&laid, (level, row), value, ctx);
                        ctx.request_paint();
                    }
                    Some(Pointed::Kept | Pointed::Pressed(_)) | None => {}
                }
            }
            _ => {}
        }

        self.announce(before, state, ctx);
        let holds = state.cascade.branched() || state.cascade.is_sliding();
        match (holds, ctx.captured()) {
            (true, false) => ctx.capture_pointer_exclusively(),
            (false, true) => ctx.release_pointer(),
            _ => {}
        }
    }

    fn behavior(&self, state: &Self::State) -> Behavior {
        Behavior::MODAL.when(self.on_dismiss.is_set())
            | Behavior::OVERLAY.when(state.cascade.branched())
            | Behavior::TRACKS_HOVER
            | Behavior::FOCUSABLE.when(self.activates())
    }
}

impl std::fmt::Debug for MenuItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MenuItem")
            .field("label", &self.label)
            .field("kind", &self.kind)
            .field("children", &self.children.len())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Menu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Menu")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

impl View for Menu {
    fn build(mut self) -> Element {
        if self.fits_content {
            let width = self.panel().natural_width(&theme());
            self = self.width(width);
        }
        let still = self.still;
        let menu = Element::new(self);
        match still {
            true => menu,
            false => menu.transition(pop()),
        }
    }
}

impl Styled for Menu {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
