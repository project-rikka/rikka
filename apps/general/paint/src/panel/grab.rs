use std::rc::Rc;

use paint_document::{LayerId, Placement};
use rikka_app::prelude::*;
use rikka_ui::{Element, Handler, Layout};

const SLOP: f32 = 4.0;
const INTO: std::ops::Range<f32> = 0.3..0.7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Shape {
    pub(crate) depth: usize,
    pub(crate) group: bool,
    pub(crate) open: bool,
    pub(crate) background: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Drop {
    Above,
    Below,
    Into,
}

pub(crate) struct Grab {
    index: usize,
    ids: Rc<[LayerId]>,
    shapes: Rc<[Shape]>,
    pitch: f32,
    on_activate_layer: Handler,
    on_menu: Handler<(Point,)>,
    on_aim: Handler<(Option<Placement>,)>,
    on_drop: Handler<(Placement,)>,
}

#[derive(Debug, Default)]
pub(crate) struct GrabState {
    pressed: Option<Point>,
    dragging: bool,
    aim: Option<Placement>,
}

pub(crate) fn grab(
    face: impl View,
    index: usize,
    ids: Rc<[LayerId]>,
    shapes: Rc<[Shape]>,
    pitch: f32,
) -> Element {
    let id = ids[index];
    let grab = Grab {
        index,
        ids,
        shapes,
        pitch,
        on_activate_layer: Handler::new(move |paint: &mut crate::paint::Paint| {
            paint.activate_layer(id)
        }),
        on_menu: Handler::new(move |paint: &mut crate::paint::Paint, at: Point| {
            paint.open_layer_menu(id, at);
        }),
        on_aim: Handler::new(|paint: &mut crate::paint::Paint, aim: Option<Placement>| {
            paint.aim_layer(aim);
        }),
        on_drop: Handler::new(move |paint: &mut crate::paint::Paint, to: Placement| {
            paint.drop_layer(id, to);
        }),
    };
    Element::with_children(grab, vec![face.build()])
}

impl Widget for Grab {
    type State = GrabState;

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::HUG)
    }

    fn event(&self, state: &mut GrabState, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                state.pressed = Some(position);
                state.dragging = false;
                state.aim = None;
                ctx.capture_pointer();
                ctx.keep_focus();
                ctx.set_handled();
                ctx.call(&self.on_activate_layer, ());
            }
            Event::PointerPressed {
                position,
                button: PointerButton::Secondary,
                ..
            } => {
                ctx.set_handled();
                ctx.call(
                    &self.on_menu,
                    (Point::new(position.x - bounds.origin.x, 0.0),),
                );
            }
            Event::PointerMoved(position) => {
                let Some(from) = state.pressed else {
                    return;
                };
                let moved = (position.x - from.x).hypot(position.y - from.y);
                if !state.dragging && (moved < SLOP || self.shapes[self.index].background) {
                    return;
                }
                state.dragging = true;
                ctx.set_handled();
                let offset = position.y - (bounds.origin.y + bounds.size.height / 2.0);
                let aim = aimed(&self.shapes, self.index, offset / self.pitch).map(|(at, drop)| {
                    let target = self.ids[at];
                    match drop {
                        Drop::Above => Placement::Above(target),
                        Drop::Below => Placement::Below(target),
                        Drop::Into => Placement::Into(target),
                    }
                });
                if aim != state.aim {
                    state.aim = aim;
                    ctx.call(&self.on_aim, (aim,));
                }
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => {
                if state.pressed.take().is_none() {
                    return;
                }
                ctx.release_pointer();
                ctx.set_handled();
                let dragged = std::mem::take(&mut state.dragging);
                match (dragged, state.aim.take()) {
                    (true, Some(to)) => {
                        ctx.call(&self.on_drop, (to,));
                    }
                    (true, None) => {
                        ctx.call(&self.on_aim, (None,));
                    }
                    (false, _) => {}
                }
            }
            _ => {}
        }
    }

    fn debug_name(&self) -> &'static str {
        "LayerGrab"
    }
}

impl std::fmt::Debug for Grab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grab")
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

fn aimed(shapes: &[Shape], from: usize, rows: f32) -> Option<(usize, Drop)> {
    let last = shapes.len().checked_sub(1)?;
    let reach = from as f32 + rows;
    let (index, part) = match reach {
        reach if reach < -0.5 => (0, 0.0),
        reach if reach > last as f32 + 0.5 => (last, 1.0),
        reach => {
            let index = reach.round().clamp(0.0, last as f32) as usize;
            (index, reach - index as f32 + 0.5)
        }
    };
    let dragged = shapes.get(from)?;
    let inside = shapes[from + 1..]
        .iter()
        .take_while(|shape| shape.depth > dragged.depth)
        .count();
    if (from..=from + inside).contains(&index) {
        return None;
    }
    let target = shapes[index];
    let drop = match (target.group, INTO.contains(&part), part < 0.5) {
        (true, true, _) => Drop::Into,
        (_, _, true) => Drop::Above,
        _ if target.group && target.open => Drop::Into,
        _ => Drop::Below,
    };
    match (drop, target.background) {
        (Drop::Below, true) => None,
        _ => Some((index, drop)),
    }
}
