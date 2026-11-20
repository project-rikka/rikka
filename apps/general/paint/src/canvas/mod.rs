mod ants;
mod drawing;
mod figures;
mod frames;
mod grip;
mod marquee;
mod overlay;
mod path;
mod selecting;
mod text;

use std::rc::Rc;

use paint_document::{Area, Combine, Layer, LayerId};
use rikka_app::prelude::*;
use rikka_figure::{Keys, constrain_angle};
use rikka_ui::ScrollSource;

pub(crate) use grip::Grip;

use crate::camera::Camera;
use crate::checker;
use crate::colors::{Colors, Slot};
use crate::figures::FigureSelection;
use crate::ink::Ink;
use crate::options::{Brushing, Filling, Letters};
use crate::paint::Paint;
use crate::picture::Picture;
use crate::pointer::Pointer;
use crate::selection::Selection;
use crate::tab::Tab;
use crate::tool::Tool;
use crate::typing::Typing;
use figures::{Band, Manipulating, PressOutcome};
use overlay::Overlay;
use path::Path;
use text::{Placing, TextOverlay};

pub(super) const CHECKER_SQUARE: f32 = 8.0;
const SHADOW: f32 = 10.0;
const SHADOW_ALPHA: f32 = 0.35;
const WHEEL_STEP: f32 = 1.25;
const PINCH: f32 = 1.2214028;
const BOX_SLOP: f32 = 4.0;
const BOX_STROKE: f32 = 1.0;
const BOX_TINT: f32 = 0.15;
const GUIDE_STEP: f32 = 8.0;
const GUIDE_DOT: f32 = 4.0;
const GUIDE_CORE: f32 = 2.0;
const HANDLE: f32 = 10.0;
const HANDLE_CORE: f32 = 6.0;

#[derive(Clone, Debug, PartialEq)]
struct Brush {
    colors: Colors,
    width: f32,
    filling: Filling,
    brushing: Option<Brushing>,
    antialiased: bool,
    transparent: bool,
    refusal: Option<Label>,
}

#[derive(Debug)]
struct FigureEdit {
    picked: FigureSelection,
    layers: Rc<Vec<Layer>>,
    active_layer: Option<LayerId>,
    editable: bool,
}

impl FigureEdit {
    fn same(&self, other: &FigureEdit) -> bool {
        self.picked.same(&other.picked)
            && self.active_layer == other.active_layer
            && self.editable == other.editable
            && Rc::ptr_eq(&self.layers, &other.layers)
    }
}

pub(crate) struct Canvas {
    camera: Camera,
    picture: Picture,
    pointer: Pointer,
    selection: Selection,
    ink: Ink,
    selectable: bool,
    selecting: Combine,
    smoothing: bool,
    grip: Grip,
    tool: Tool,
    brush: Brush,
    typing: Typing,
    letters: Letters,
    figures: FigureEdit,
}

impl Canvas {
    pub(crate) fn new(paint: &Paint, tab: &Tab) -> Canvas {
        let tool = paint.tool();
        Canvas {
            camera: tab.camera().clone(),
            picture: tab.picture().clone(),
            pointer: tab.pointer().clone(),
            selection: tab.selection().clone(),
            ink: tab.ink().clone(),
            selectable: tab.takes_selection(),
            selecting: paint.options().selecting,
            smoothing: paint.options().smoothing,
            grip: paint.grip().clone(),
            tool,
            brush: Brush {
                colors: paint.colors(),
                width: paint.options().width(tool).unwrap_or_default(),
                filling: paint.options().filling,
                brushing: paint.options().brushing(tool),
                antialiased: paint.options().antialiased,
                transparent: tab.holds_transparency(),
                refusal: tab.paint_refusal(tool, paint.options()),
            },
            typing: tab.typing().clone(),
            letters: paint.letters().clone(),
            figures: FigureEdit {
                picked: tab.picked().clone(),
                layers: tab.shared_layers(),
                active_layer: tab.active_layer().map(|layer| layer.id),
                editable: tab.figures_editable(),
            },
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct CanvasState {
    drag: Option<Drag>,
    overlay: Overlay,
    path: Option<Path>,
    lettered: TextOverlay,
    manipulating: Option<Manipulating>,
    band: Option<Band>,
    pointed: Point,
    menu: ContextMenu,
}

#[derive(Clone, Copy, Debug)]
enum Drag {
    Pan {
        last: Point,
    },
    Zoom {
        from: Point,
        to: Point,
        outward: bool,
    },
    Select {
        from: Point,
        combine: Combine,
        square: bool,
        oval: bool,
        marked: Area,
    },
    Lasso,
    Move {
        from: Point,
        base: (i32, i32),
        by: (i32, i32),
    },
    Draw {
        keys: Keys,
    },
    Gradient {
        from: Point,
        to: Point,
        slot: Slot,
        keys: Keys,
    },
    Sample {
        slot: Slot,
        at: (u32, u32),
    },
    Caret,
    Carry {
        grabbed: Point,
        from: (i32, i32),
    },
    Figure,
    Band,
}

impl Widget for Canvas {
    type State = CanvasState;

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, state: &mut CanvasState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        self.camera.meet(bounds.size);
        let extent = self.camera.extent();
        if extent.width > 0.0 && extent.height > 0.0 {
            let texels = self.camera.scale() * ctx.scale();
            self.picture.wish(paint_document::level_for(texels));
        }
        ctx.draw_quad(Quad::new(bounds).fill(palette().surface_variant));
        state.menu.paint(ctx);
        let pieces = self.picture.pieces();
        if pieces.is_empty() {
            return;
        }
        let scale = self.camera.scale();
        let device = ctx.scale();
        let snap = |value: f32| (value * device).round() / device;
        let origin = self.camera.origin();
        let origin = Point::new(bounds.origin.x + origin.x, bounds.origin.y + origin.y);
        let at =
            |x: f32, y: f32| Point::new(snap(origin.x + x * scale), snap(origin.y + y * scale));
        let place = |area: Rect| {
            let corner = at(area.min_x(), area.min_y());
            let opposite = at(area.max_x(), area.max_y());
            Rect::new(
                corner.x,
                corner.y,
                opposite.x - corner.x,
                opposite.y - corner.y,
            )
        };
        let extent = self.camera.extent();
        let whole = place(Rect::new(0.0, 0.0, extent.width, extent.height));
        let pixelated = scale * device >= 1.0;
        ctx.clipped(RoundedRect::new(bounds, 0.0), |ctx| {
            ctx.draw_quad(
                Quad::new(whole)
                    .fill(Color::rgba(0.0, 0.0, 0.0, SHADOW_ALPHA))
                    .blur(SHADOW),
            );
            if let Some(visible) = whole.intersection(&bounds) {
                checker::fill(ctx, visible, whole.origin, CHECKER_SQUARE);
            }
            for piece in self.picture.incoming().iter() {
                let rect = place(piece.area);
                if rect.intersection(&bounds).is_some() {
                    ctx.draw_sprite(Sprite::new(rect, piece.source()).tinted(Color::TRANSPARENT));
                }
            }
            for piece in pieces.iter() {
                let rect = place(piece.area);
                if rect.intersection(&bounds).is_none() {
                    continue;
                }
                match pixelated {
                    true => ctx.draw_image_pixelated(rect, piece.source()),
                    false => ctx.draw_image(rect, piece.source()),
                }
            }
            self.paint_stroke(state, ctx, whole, &place, at(0.0, 0.0));
            let placing = Placing {
                whole,
                pixelated,
                place: &place,
                at: &at,
            };
            self.paint_text(state, ctx, &placing);
            self.paint_figures(state, ctx, &at);
            if let Some(Drag::Gradient { from, to, keys, .. }) = state.drag {
                let to = constrain_angle(from, to, keys);
                guide(ctx, at(from.x, from.y), at(to.x, to.y));
            }
            let outline = self.selection.outline();
            let (dx, dy) = self.selection.offset();
            ants::draw(ctx, &outline, bounds, |(x, y)| {
                at(x as f32 + dx as f32, y as f32 + dy as f32)
            });
            self.paint_path(state, ctx, &at);
            if let Some(Drag::Zoom { from, to, .. }) = state.drag {
                let marked = Rect::new(
                    from.x.min(to.x),
                    from.y.min(to.y),
                    (to.x - from.x).abs(),
                    (to.y - from.y).abs(),
                );
                let accent = palette().accent;
                ctx.draw_quad(
                    Quad::new(marked)
                        .fill(accent.scale_alpha(BOX_TINT))
                        .stroke(BOX_STROKE, accent),
                );
            }
        });
    }

    fn event(&self, state: &mut CanvasState, event: &Event, ctx: &mut EventCtx<'_>) {
        if state.menu.event(event, ctx) {
            return;
        }
        let bounds = ctx.bounds();
        let local = |at: Point| Point::new(at.x - bounds.origin.x, at.y - bounds.origin.y);
        match *event {
            Event::PointerPressed {
                position,
                button,
                modifiers,
            } => {
                if state.drag.is_some() {
                    ctx.set_handled();
                    return;
                }
                if button == PointerButton::Secondary
                    && self.summon_text_menu(state, local(position), position, ctx)
                {
                    return;
                }
                if self.polygon_pressed(state, local(position), button, modifiers, ctx) {
                    return;
                }
                state.drag = self.pressed(state, local(position), position, button, modifiers, ctx);
                if let Some(drag) = state.drag {
                    self.grip.set_panning(matches!(drag, Drag::Pan { .. }));
                    ctx.capture_pointer();
                    ctx.request_focus();
                    ctx.set_handled();
                }
            }
            Event::PointerMoved(position) => {
                state.pointed = local(position);
                self.track(local(position));
                self.polygon_moved(state, local(position), ctx);
                self.dragged(state, local(position), position, ctx);
            }
            Event::PointerReleased { position, .. } => {
                let Some(drag) = state.drag.take() else {
                    if state.path.is_some() {
                        ctx.set_handled();
                    }
                    return;
                };
                self.grip.set_panning(false);
                ctx.release_pointer();
                ctx.set_handled();
                self.released(state, drag, local(position), ctx);
            }
            Event::KeyPressed { key, .. } => {
                if self.type_event(state, event, ctx) {
                    ctx.set_handled();
                    return;
                }
                if !self.path_keyed(state, event, key, ctx)
                    && !self.key_figures(state, event, key, ctx)
                {
                    self.keyed(state, event, key, ctx);
                }
            }
            Event::KeyReleased { key, .. } => {
                if !self.path_keyed(state, event, key, ctx)
                    && !self.key_figures(state, event, key, ctx)
                {
                    self.keyed(state, event, key, ctx);
                }
            }
            Event::Command(_) | Event::Preedit { .. } | Event::Commit(_) | Event::FocusLost => {
                if self.type_event(state, event, ctx) {
                    ctx.set_handled();
                }
            }
            Event::PointerLeft | Event::PointerExited => self.pointer.set(None),
            Event::Scroll {
                position,
                delta,
                modifiers,
                source,
            } => {
                ctx.set_handled();
                let scale = self.camera.scale();
                self.scrolled(local(position), delta, modifiers, source);
                if self.camera.scale() != scale {
                    ctx.run(Paint::rescaled);
                }
            }
            _ => {}
        }
    }

    fn cursor(&self, _state: &Self::State, bounds: Rect, point: Point) -> Option<CursorShape> {
        if self.grip.is_panning() {
            return Some(CursorShape::Grabbing);
        }
        if self.grip.space_held() {
            return Some(CursorShape::Grab);
        }
        let local = Point::new(point.x - bounds.origin.x, point.y - bounds.origin.y);
        if self.tool.picks()
            && let Some(cursor) = self.figure_cursor(local)
        {
            return Some(cursor);
        }
        Some(match self.tool {
            Tool::Hand => CursorShape::Grab,
            Tool::Zoom => CursorShape::ZoomIn,
            Tool::Objects => CursorShape::Default,
            _ if self.brush.refusal.is_some() => CursorShape::NotAllowed,
            Tool::Text
                if self.grips_handle(Point::new(
                    point.x - bounds.origin.x,
                    point.y - bounds.origin.y,
                )) =>
            {
                CursorShape::Move
            }
            Tool::Text => CursorShape::Text,
            Tool::RectangleSelect
            | Tool::Pen
            | Tool::Highlighter
            | Tool::Line
            | Tool::Arrow
            | Tool::Rectangle
            | Tool::Ellipse
            | Tool::ColorPicker
            | Tool::EllipseSelect
            | Tool::Lasso
            | Tool::PolygonLasso
            | Tool::MagicWand
            | Tool::PaintBucket
            | Tool::Paintbrush
            | Tool::Eraser
            | Tool::Gradient
            | Tool::FreeformShape => CursorShape::Crosshair,
        })
    }

    fn caret(&self, state: &CanvasState) -> Option<Rect> {
        state.lettered.caret()
    }

    fn behavior(&self, _state: &CanvasState) -> Behavior {
        Behavior::CLIPS | Behavior::TRACKS_HOVER | Behavior::FOCUSABLE | Behavior::NO_TAB_STOP
    }

    fn same(&self, other: &Canvas) -> bool {
        self.tool == other.tool
            && self.brush == other.brush
            && self.selecting == other.selecting
            && self.smoothing == other.smoothing
            && self.camera.same(&other.camera)
            && self.picture.same(&other.picture)
            && self.pointer.same(&other.pointer)
            && self.grip.same(&other.grip)
            && self.selection.same(&other.selection)
            && self.ink.same(&other.ink)
            && self.selectable == other.selectable
            && self.typing.same(&other.typing)
            && self.letters == other.letters
            && self.figures.same(&other.figures)
    }
}

impl Canvas {
    fn pressed(
        &self,
        state: &mut CanvasState,
        at: Point,
        position: Point,
        button: PointerButton,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) -> Option<Drag> {
        let panning = button == PointerButton::Middle
            || (button == PointerButton::Primary
                && (self.tool == Tool::Hand || self.grip.space_held()));
        if panning {
            return Some(Drag::Pan { last: position });
        }
        match self.tool {
            Tool::Zoom => matches!(button, PointerButton::Primary | PointerButton::Secondary)
                .then_some(Drag::Zoom {
                    from: position,
                    to: position,
                    outward: button == PointerButton::Secondary || modifiers.alt,
                }),
            Tool::RectangleSelect
            | Tool::EllipseSelect
            | Tool::Lasso
            | Tool::PolygonLasso
            | Tool::MagicWand => self.begin_selecting(state, at, button, modifiers, ctx),
            Tool::PaintBucket => {
                self.pour(at, button, ctx);
                None
            }
            Tool::ColorPicker => {
                let slot = Slot::from_button(button)?;
                let pixel = self.pixel(at)?;
                ctx.call(&Handler::new(Paint::sample), (pixel, slot));
                Some(Drag::Sample { slot, at: pixel })
            }
            Tool::Line | Tool::Arrow | Tool::Rectangle | Tool::Ellipse | Tool::FreeformShape => {
                match self.press_figures(state, at, button, modifiers, ctx) {
                    PressOutcome::Handled(drag) => drag,
                    PressOutcome::Passed => self.begin_stroke(at, button, modifiers, ctx),
                }
            }
            Tool::Objects => match self.press_figures(state, at, button, modifiers, ctx) {
                PressOutcome::Handled(drag) => drag,
                PressOutcome::Passed => None,
            },
            Tool::Pen | Tool::Highlighter | Tool::Paintbrush | Tool::Eraser | Tool::Gradient => {
                self.begin_stroke(at, button, modifiers, ctx)
            }
            Tool::Text => self.press_text(at, button, modifiers, ctx),
            Tool::Hand => None,
        }
    }

    fn dragged(&self, state: &mut CanvasState, at: Point, position: Point, ctx: &mut EventCtx<'_>) {
        match &mut state.drag {
            Some(Drag::Pan { last }) => {
                let by = Point::new(position.x - last.x, position.y - last.y);
                *last = position;
                self.camera.pan(by);
            }
            Some(Drag::Zoom { to, .. }) => {
                *to = position;
                ctx.request_paint();
            }
            Some(Drag::Select { .. } | Drag::Lasso | Drag::Move { .. }) => {
                self.select_dragged(state, at, ctx);
            }
            Some(Drag::Draw { .. }) => {
                let origin = ctx.bounds().origin;
                let reached: Vec<Point> = match ctx.stylus() {
                    Some(stylus) => stylus
                        .samples()
                        .iter()
                        .map(|sample| {
                            self.camera.picture_point(Point::new(
                                sample.position.x - origin.x,
                                sample.position.y - origin.y,
                            ))
                        })
                        .collect(),
                    None => vec![self.camera.picture_point(at)],
                };
                self.ink.change(|stroke| {
                    let mut grew = false;
                    for point in reached {
                        grew |= stroke.reach(point);
                    }
                    grew
                });
            }
            Some(drag @ (Drag::Caret | Drag::Carry { .. })) => self.drag_text(*drag, at),
            Some(Drag::Figure | Drag::Band) => self.drag_figures(state, at, ctx),
            Some(Drag::Gradient {
                from,
                to,
                slot,
                keys,
            }) => {
                let reached = self.camera.picture_point(at);
                if reached != *to {
                    *to = reached;
                    let end = constrain_angle(*from, reached, *keys);
                    ctx.call(&Handler::new(Paint::gradient), ((*from, end), *slot, false));
                    ctx.request_paint();
                }
            }
            Some(Drag::Sample { slot, at: sampled }) => {
                if let Some(pixel) = self.pixel(at)
                    && pixel != *sampled
                {
                    *sampled = pixel;
                    ctx.call(&Handler::new(Paint::sample), (pixel, *slot));
                }
            }
            None => {}
        }
    }

    fn released(&self, state: &mut CanvasState, drag: Drag, at: Point, ctx: &mut EventCtx<'_>) {
        match drag {
            Drag::Zoom { from, outward, .. } => {
                let bounds = ctx.bounds();
                let from = Point::new(from.x - bounds.origin.x, from.y - bounds.origin.y);
                self.zoomed(from, at, outward);
                ctx.request_paint();
                ctx.run(Paint::rescaled);
            }
            Drag::Select { .. } | Drag::Lasso | Drag::Move { .. } => {
                self.select_released(state, drag, at, ctx);
            }
            Drag::Draw { .. } => {
                let reached = self.camera.picture_point(at);
                self.ink.change(|stroke| stroke.reach(reached));
                if let Some(stroke) = self.ink.take()
                    && stroke.is_worth_keeping()
                {
                    ctx.call(&Handler::new(Paint::stroke), (stroke,));
                }
            }
            Drag::Gradient {
                from, slot, keys, ..
            } => {
                let end = constrain_angle(from, self.camera.picture_point(at), keys);
                ctx.request_paint();
                ctx.call(&Handler::new(Paint::gradient), ((from, end), slot, true));
            }
            Drag::Figure | Drag::Band => self.release_figures(state, ctx),
            Drag::Pan { .. } | Drag::Sample { .. } | Drag::Caret | Drag::Carry { .. } => {}
        }
    }

    fn pixel(&self, at: Point) -> Option<(u32, u32)> {
        let extent = self.camera.extent();
        let point = self.camera.picture_point(at);
        let inside = point.x >= 0.0
            && point.y >= 0.0
            && point.x < extent.width
            && point.y < extent.height
            && self.picture.is_shown();
        inside.then_some((point.x as u32, point.y as u32))
    }

    fn track(&self, at: Point) {
        self.pointer.set(self.pixel(at));
    }

    fn marked(&self, from: Point, to: Point, square: bool) -> Area {
        let to = self.camera.picture_point(to);
        marquee::dragged_area(from, to, square, self.camera.extent())
    }

    fn zoomed(&self, from: Point, to: Point, outward: bool) {
        let dragged = (to.x - from.x).abs() > BOX_SLOP || (to.y - from.y).abs() > BOX_SLOP;
        match (dragged, outward) {
            (true, false) => self.camera.frame(
                self.camera.picture_point(from),
                self.camera.picture_point(to),
            ),
            _ => self.camera.step(!outward, Some(to)),
        }
    }

    fn scrolled(&self, at: Point, delta: ScrollDelta, modifiers: Modifiers, source: ScrollSource) {
        let content = delta.content();
        match (source, modifiers.control, modifiers.shift) {
            (ScrollSource::Wheel, false, false) => {
                let notches = delta.zoom();
                match notches.abs() >= 1.0 {
                    true => {
                        for _ in 0..notches.abs().round() as u32 {
                            self.camera.step(notches > 0.0, Some(at));
                        }
                    }
                    false => self.camera.magnify(WHEEL_STEP.powf(notches), at),
                }
            }
            (ScrollSource::Wheel, false, true) => self.camera.pan(Point::new(content.y, 0.0)),
            (ScrollSource::Wheel, true, _) => self.camera.pan(content),
            (ScrollSource::Continuous(_), true, _) => {
                self.camera.magnify(PINCH.powf(delta.zoom()), at);
            }
            (ScrollSource::Continuous(_), false, _) => self.camera.pan(content),
        }
    }
}

fn guide(ctx: &mut PaintCtx<'_>, from: Point, to: Point) {
    let length = from.distance(to);
    let steps = (length / GUIDE_STEP).floor() as u32;
    for step in 1..steps {
        let t = step as f32 * GUIDE_STEP / length;
        let dot = Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
        ctx.draw_circle(dot, GUIDE_DOT, Color::BLACK);
        ctx.draw_circle(dot, GUIDE_CORE, Color::WHITE);
    }
    for end in [from, to] {
        ctx.draw_circle(end, HANDLE, Color::BLACK);
        ctx.draw_circle(end, HANDLE_CORE, Color::WHITE);
    }
}

impl View for Canvas {
    fn build(self) -> Element {
        Element::new(self)
    }
}

impl std::fmt::Debug for Canvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Canvas")
            .field("tool", &self.tool)
            .finish_non_exhaustive()
    }
}
