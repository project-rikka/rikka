use rikka_theme::Theme;
use rikka_types::{Point, PointerButton, Rect, Size};
use rikka_ui::{Command, DRAG_SLOP, DropdownPlacement, Event, EventCtx, PaintCtx, point_panel};

use crate::control::ControlSize;
use crate::navigation::menu::{
    Activation, Cascade, Keyed, Kind, Laid, Listing, MenuItem, Panel, Pointed, Press, leaf, slide,
};

const SCOPE: &str = "context-menu";

const DROP_GAP: f32 = 4.0;

#[derive(Debug, Default)]
pub struct ContextMenu {
    items: Vec<MenuItem>,
    at: Point,
    size: Size,
    open: bool,
    press_origin: Option<PressOrigin>,
    swallowing: bool,
    cascade: Cascade,
}

#[derive(Clone, Copy, Debug)]
struct PressOrigin {
    from: Point,
    travelled: bool,
}

enum Outcome {
    Kept,
    Changed,
    Close,
    Dismiss,
    Reopen,
    Choose(usize, usize),
    Slide(usize, usize, f32),
}

impl ContextMenu {
    pub fn open(
        &mut self,
        items: impl IntoIterator<Item = MenuItem>,
        at: Point,
        ctx: &mut EventCtx<'_>,
    ) {
        if self.show(items, at, ctx) {
            self.press_origin = Some(PressOrigin {
                from: at,
                travelled: false,
            });
        }
    }

    pub fn open_by_keys(
        &mut self,
        items: impl IntoIterator<Item = MenuItem>,
        under: Option<Rect>,
        ctx: &mut EventCtx<'_>,
    ) {
        let bounds = ctx.bounds();
        let at = under.map_or(bounds.origin, |rect| {
            Point::new(bounds.x() + rect.x(), bounds.y() + rect.max_y())
        });
        if self.show(items, at, ctx) {
            self.highlight_first(ctx);
        }
    }

    pub(crate) fn drop_down(
        &mut self,
        items: impl IntoIterator<Item = MenuItem>,
        under: Rect,
        activation: Activation,
        ctx: &mut EventCtx<'_>,
    ) {
        let items = tidy(items);
        let width = Panel::new(&items, ControlSize::default())
            .size_beside(ctx.theme())
            .width;
        let at = Point::new(under.max_x() - width, under.max_y() + DROP_GAP);
        if !self.show(items, at, ctx) {
            return;
        }
        match activation {
            Activation::Pointer => {
                self.press_origin = Some(PressOrigin {
                    from: under.center(),
                    travelled: false,
                });
            }
            Activation::Keyboard => self.highlight_first(ctx),
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    fn highlight_first(&mut self, ctx: &EventCtx<'_>) {
        let dropped = self.placed(ctx.bounds().origin, ctx.viewport());
        let laid = lay(
            &self.items,
            &mut self.cascade,
            dropped,
            ctx.theme(),
            ctx.viewport(),
        );
        self.cascade.step(&laid, true);
    }

    fn close(&mut self, ctx: &mut EventCtx<'_>) {
        if !self.open {
            return;
        }
        self.fold();
        ctx.release_pointer();
        ctx.request_paint();
    }

    pub fn event(&mut self, event: &Event, ctx: &mut EventCtx<'_>) -> bool {
        if !self.open {
            return self.swallowed(event, ctx);
        }
        if let Event::FocusLost = event {
            self.close(ctx);
            return false;
        }
        match event {
            Event::PointerPressed { .. } => ctx.keep_focus(),
            Event::PointerReleased { .. }
            | Event::PointerCancelled
            | Event::PointerMoved(_)
            | Event::PointerExited
            | Event::Scroll { .. }
            | Event::KeyPressed { .. }
            | Event::Command(_) => {}
            _ => return false,
        }
        let dropped = self.placed(ctx.bounds().origin, ctx.viewport());
        let laid = lay(
            &self.items,
            &mut self.cascade,
            dropped,
            ctx.theme(),
            ctx.viewport(),
        );
        let outcome = match *event {
            Event::PointerMoved(to) => {
                if let Some(press_origin) = &mut self.press_origin {
                    press_origin.travelled |= to.distance(press_origin.from) > DRAG_SLOP;
                }
                pointed(&mut self.cascade, &laid, event)
            }
            Event::PointerPressed {
                button, position, ..
            } => {
                self.press_origin = None;
                match self.cascade.pointer(&laid, event) {
                    Some(Pointed::Pressed(Press::Away))
                        if button == PointerButton::Secondary
                            && ctx.bounds().contains(position) =>
                    {
                        Outcome::Reopen
                    }
                    Some(Pointed::Pressed(Press::Away)) => Outcome::Dismiss,
                    Some(Pointed::Pressed(Press::Opened | Press::Inert)) => Outcome::Changed,
                    Some(Pointed::Pressed(Press::Slide { level, row, value })) => {
                        Outcome::Slide(level, row, value)
                    }
                    _ => Outcome::Kept,
                }
            }
            Event::PointerReleased { .. } | Event::PointerCancelled
                if self.cascade.is_sliding() =>
            {
                pointed(&mut self.cascade, &laid, event)
            }
            Event::PointerReleased { position, .. } => {
                let opening = self
                    .press_origin
                    .take()
                    .is_some_and(|press_origin| !press_origin.travelled);
                match self.cascade.press(&laid, position) {
                    Press::Leaf { level, row } if !opening => Outcome::Choose(level, row),
                    Press::Opened | Press::Inert | Press::Slide { .. } => Outcome::Changed,
                    Press::Leaf { .. } | Press::Away => Outcome::Kept,
                }
            }
            Event::KeyPressed { key, modifiers, .. }
                if Command::for_key(key, modifiers) == Some(Command::ContextMenu) =>
            {
                Outcome::Close
            }
            Event::KeyPressed { key, modifiers, .. } => {
                match self.cascade.key(&laid, key, modifiers) {
                    Keyed::Changed => Outcome::Changed,
                    Keyed::Choose { level, row } => Outcome::Choose(level, row),
                    Keyed::Slid { level, row, value } => Outcome::Slide(level, row, value),
                    Keyed::Close => Outcome::Close,
                    Keyed::Ignored | Keyed::Kept | Keyed::Aside { .. } => Outcome::Kept,
                }
            }
            Event::PointerExited | Event::Scroll { .. } => pointed(&mut self.cascade, &laid, event),
            Event::Command(_) | Event::PointerCancelled => Outcome::Kept,
            _ => return false,
        };
        match outcome {
            Outcome::Kept => {}
            Outcome::Changed => ctx.request_paint(),
            Outcome::Close => self.close(ctx),
            Outcome::Dismiss => {
                self.fold();
                self.swallowing = true;
                ctx.request_paint();
            }
            Outcome::Reopen => {
                self.close(ctx);
                return false;
            }
            Outcome::Choose(level, row) => {
                if let Some(item) = leaf(&laid, level, row) {
                    item.pressed(ctx);
                }
                self.fold();
                ctx.release_pointer();
                ctx.request_paint();
            }
            Outcome::Slide(level, row, value) => {
                slide(&laid, (level, row), value, ctx);
                self.adopt(level, row, value);
                ctx.request_paint();
            }
        }
        ctx.set_handled();
        true
    }

    pub fn paint(&mut self, ctx: &mut PaintCtx<'_>) {
        if self.items.is_empty() {
            return;
        }
        let dropped = self.placed(ctx.bounds().origin, ctx.viewport());
        let open = self.open;
        let items = &self.items;
        let cascade = &mut self.cascade;
        let mut closed = false;
        ctx.topmost(|ctx| {
            ctx.motion_scope(SCOPE, |ctx| {
                let shown = cascade.root_mut().reveal(ctx, open);
                if shown <= 0.0 {
                    closed = !open;
                    return;
                }
                let laid = lay(items, cascade, dropped, ctx.theme(), ctx.viewport());
                let (panel, rect) = laid[0];
                panel.paint(
                    ctx,
                    cascade.root_mut(),
                    rect,
                    shown,
                    dropped.pivot(),
                    |_, _| {},
                );
                cascade.paint_branches(ctx, &laid, shown);
            });
        });
        if closed {
            self.items.clear();
        }
    }

    fn show(
        &mut self,
        items: impl IntoIterator<Item = MenuItem>,
        at: Point,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let items = tidy(items);
        if !items.iter().any(|item| item.kind == Kind::Item) {
            return false;
        }
        let origin = ctx.bounds().origin;
        self.size = Panel::new(&items, ControlSize::default()).size_beside(ctx.theme());
        self.items = items;
        self.at = Point::new(at.x - origin.x, at.y - origin.y);
        self.open = true;
        self.press_origin = None;
        self.swallowing = false;
        self.cascade = Cascade::default();
        *self.cascade.root_mut() = Listing::unseen();
        ctx.capture_pointer_exclusively();
        ctx.request_focus();
        ctx.request_paint();
        ctx.set_handled();
        true
    }

    fn fold(&mut self) {
        self.open = false;
        self.press_origin = None;
        self.cascade.collapse();
        self.cascade.root_mut().highlight(None);
    }

    fn adopt(&mut self, level: usize, row: usize, value: f32) {
        let mut items = &mut self.items;
        for opened_by in self.cascade.path(level) {
            items = &mut items[opened_by].children;
        }
        if let Some(slider) = items[row].slider.as_mut() {
            slider.value = value;
        }
    }

    fn swallowed(&mut self, event: &Event, ctx: &mut EventCtx<'_>) -> bool {
        if !self.swallowing {
            return false;
        }
        match event {
            Event::PointerReleased { .. } => {
                self.swallowing = false;
                ctx.release_pointer();
                ctx.set_handled();
                true
            }
            Event::FocusLost => {
                self.swallowing = false;
                ctx.release_pointer();
                false
            }
            _ => false,
        }
    }

    fn placed(&self, origin: Point, viewport: Size) -> DropdownPlacement {
        let at = Point::new(origin.x + self.at.x, origin.y + self.at.y);
        point_panel(at, self.size, viewport)
    }
}

fn lay<'a>(
    items: &'a [MenuItem],
    cascade: &mut Cascade,
    dropped: DropdownPlacement,
    theme: &Theme,
    viewport: Size,
) -> Laid<'a> {
    let panel = Panel::new(items, ControlSize::default());
    cascade.lay(panel, dropped.rect(), theme, viewport)
}

fn pointed(cascade: &mut Cascade, laid: &Laid<'_>, event: &Event) -> Outcome {
    match cascade.pointer(laid, event) {
        Some(Pointed::Slid { level, row, value }) => Outcome::Slide(level, row, value),
        Some(Pointed::Changed) => Outcome::Changed,
        _ => Outcome::Kept,
    }
}

fn tidy(items: impl IntoIterator<Item = MenuItem>) -> Vec<MenuItem> {
    let mut filtered: Vec<MenuItem> = Vec::new();
    for item in items {
        let after_separator = filtered
            .last()
            .is_none_or(|last| last.kind == Kind::Separator);
        if item.kind == Kind::Separator && after_separator {
            continue;
        }
        filtered.push(item);
    }
    while filtered
        .last()
        .is_some_and(|last| last.kind == Kind::Separator)
    {
        filtered.pop();
    }
    filtered
}
