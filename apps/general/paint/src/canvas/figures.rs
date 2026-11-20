use paint_document::{Figure, FigureId, FigureKind, Layer, LayerId, LayerKind};
use rikka_app::prelude::*;
use rikka_figure::{GRIP_REACH, GrabTarget, Grip, Grips, Keys, TURN_GAP, enclosing};

use super::{Canvas, CanvasState, Drag};
use crate::figures::{ManipulatedFigures, Manipulation, grips_of};
use crate::paint::Paint;
use crate::tool::Tool;

const HIT_REACH: f32 = 4.0;
const SLOP: f32 = 3.0;

#[derive(Debug)]
pub(super) struct Manipulating {
    manipulation: Manipulation,
    copies: bool,
    keys: Keys,
    moved: bool,
    start: Point,
    pressed: Option<(LayerId, FigureId)>,
    pub(super) reached: Option<ManipulatedFigures>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Band {
    pub(super) from: Point,
    pub(super) to: Point,
    adds: bool,
}

pub(super) enum PressOutcome {
    Handled(Option<Drag>),
    Passed,
}

impl Canvas {
    pub(super) fn picked_layer(&self) -> Option<LayerId> {
        let first = *self.figures.picked.ids().first()?;
        Layer::holding(&self.figures.layers, first).map(|layer| layer.id)
    }

    pub(super) fn shown_picks(&self) -> Vec<Figure> {
        let figures = self
            .picked_layer()
            .and_then(|layer| Layer::find(&self.figures.layers, layer))
            .map_or(&[][..], |layer| layer.figures.as_slice());
        self.figures.picked.with_preview(figures)
    }

    fn reach(&self) -> f32 {
        HIT_REACH / self.camera.scale()
    }

    fn screen(&self, point: Point) -> Point {
        self.camera.origin() + point * self.camera.scale()
    }

    fn grip_under(&self, grips: &Grips, at: Point) -> Option<Grip> {
        let turn_gap = TURN_GAP / self.camera.scale();
        grips.grip_at(at, GRIP_REACH, turn_gap, |spot| self.screen(spot))
    }

    pub(super) fn figure_under(&self, point: Point) -> Option<(LayerId, Figure)> {
        let reach = self.reach();
        hit(&self.figures.layers, point, reach)
    }

    pub(super) fn press_figures(
        &self,
        state: &mut CanvasState,
        at: Point,
        button: PointerButton,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) -> PressOutcome {
        let objects = self.tool == Tool::Objects;
        if !self.figures.editable {
            return match objects {
                true => PressOutcome::Handled(None),
                false => PressOutcome::Passed,
            };
        }
        let point = self.camera.picture_point(at);
        if button == PointerButton::Secondary && objects {
            if let Some((layer, figure)) = self.figure_under(point)
                && !self.figures.picked.holds(figure.id)
            {
                ctx.call(&Handler::new(Paint::pick_figures), (layer, vec![figure.id]));
            }
            ctx.set_handled();
            ctx.call(&Handler::new(Paint::open_figure_menu), (at,));
            return PressOutcome::Handled(None);
        }
        if button != PointerButton::Primary {
            return match objects {
                true => PressOutcome::Handled(None),
                false => PressOutcome::Passed,
            };
        }
        let keys = Keys {
            shift: modifiers.shift,
            alt: modifiers.alt,
        };
        let picks = self.shown_picks();
        let grips = grips_of(&picks);
        if let Some(grip) = grips.as_ref().and_then(|grips| self.grip_under(grips, at)) {
            return self.grab(state, GrabTarget::Grip(grip), point, at, picks, keys, None);
        }
        let doubled = ctx.presses() >= 2;
        if let Some((layer, figure)) = self.figure_under(point) {
            if doubled && objects && figure.kind() == FigureKind::Text {
                ctx.call(&Handler::new(Paint::edit_text_figure), (figure,));
                return PressOutcome::Handled(Some(Drag::Caret));
            }
            let same_layer = self.picked_layer().is_none_or(|picked| picked == layer);
            let picked = self.figures.picked.holds(figure.id);
            if keys.shift && objects && same_layer {
                let mut ids: Vec<FigureId> = picks.iter().map(|pick| pick.id).collect();
                match picked {
                    true => ids.retain(|id| *id != figure.id),
                    false => ids.push(figure.id),
                }
                ctx.call(&Handler::new(Paint::pick_figures), (layer, ids));
                if picked {
                    return PressOutcome::Handled(None);
                }
                let originals = picks.into_iter().chain(std::iter::once(figure)).collect();
                return self.grab(state, GrabTarget::Body, point, at, originals, keys, None);
            }
            let originals = match picked {
                true => picks,
                false => {
                    ctx.call(&Handler::new(Paint::pick_figures), (layer, vec![figure.id]));
                    vec![figure.clone()]
                }
            };
            let pressed = Some((layer, figure.id));
            return self.grab(state, GrabTarget::Body, point, at, originals, keys, pressed);
        }
        if grips.is_some_and(|grips| grips.contains(point, self.reach())) {
            return self.grab(state, GrabTarget::Body, point, at, picks, keys, None);
        }
        if objects {
            if !keys.shift && !picks.is_empty() {
                ctx.run(Paint::unpick);
            }
            state.band = Some(Band {
                from: point,
                to: point,
                adds: keys.shift,
            });
            ctx.request_paint();
            return PressOutcome::Handled(Some(Drag::Band));
        }
        if !picks.is_empty() {
            ctx.run(Paint::unpick);
        }
        PressOutcome::Passed
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "a grab gathers where and how the press landed"
    )]
    fn grab(
        &self,
        state: &mut CanvasState,
        grabbed: GrabTarget,
        point: Point,
        at: Point,
        originals: Vec<Figure>,
        keys: Keys,
        pressed: Option<(LayerId, FigureId)>,
    ) -> PressOutcome {
        let copies = keys.alt && grabbed == GrabTarget::Body;
        let Some(manipulation) = Manipulation::new(grabbed, point, originals) else {
            return PressOutcome::Handled(None);
        };
        state.manipulating = Some(Manipulating {
            manipulation,
            copies,
            keys,
            moved: false,
            start: at,
            pressed,
            reached: None,
        });
        PressOutcome::Handled(Some(Drag::Figure))
    }

    pub(super) fn drag_figures(&self, state: &mut CanvasState, at: Point, ctx: &mut EventCtx<'_>) {
        let point = self.camera.picture_point(at);
        if let Some(band) = &mut state.band {
            band.to = point;
            ctx.request_paint();
            return;
        }
        let Some(manipulating) = &mut state.manipulating else {
            return;
        };
        if !manipulating.moved && manipulating.start.distance(at) < SLOP {
            return;
        }
        manipulating.moved = true;
        let reached = manipulating.manipulation.reach(point, manipulating.keys);
        let changed = manipulating
            .reached
            .as_ref()
            .is_none_or(|before| before.figures != reached.figures);
        manipulating.reached = Some(reached.clone());
        ctx.request_paint();
        if changed {
            ctx.call(
                &Handler::new(Paint::preview_picked),
                (reached.figures, manipulating.copies),
            );
        }
    }

    pub(super) fn release_figures(&self, state: &mut CanvasState, ctx: &mut EventCtx<'_>) {
        if let Some(band) = state.band.take() {
            self.finish_band(band, ctx);
            ctx.request_paint();
            return;
        }
        let Some(manipulating) = state.manipulating.take() else {
            return;
        };
        ctx.request_paint();
        match (manipulating.moved, manipulating.reached) {
            (true, Some(reached)) => {
                ctx.call(
                    &Handler::new(Paint::commit_picked),
                    (
                        reached.figures,
                        manipulating.manipulation.changing(),
                        manipulating.copies,
                    ),
                );
            }
            _ => {
                if let Some((layer, id)) = manipulating.pressed
                    && manipulating.manipulation.originals().len() > 1
                {
                    ctx.call(&Handler::new(Paint::pick_figures), (layer, vec![id]));
                }
            }
        }
    }

    pub(super) fn key_figures(
        &self,
        state: &mut CanvasState,
        event: &Event,
        key: Key,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let down = matches!(event, Event::KeyPressed { .. });
        if state.band.is_some() && key == Key::Escape && down {
            state.band = None;
            state.drag = None;
            ctx.release_pointer();
            ctx.request_paint();
            ctx.set_handled();
            return true;
        }
        let Some(manipulating) = &mut state.manipulating else {
            return false;
        };
        match key {
            Key::Escape if down => {
                let moved = manipulating.moved;
                state.manipulating = None;
                state.drag = None;
                ctx.release_pointer();
                ctx.set_handled();
                ctx.request_paint();
                if moved {
                    ctx.run(Paint::drop_picked_preview);
                }
                return true;
            }
            Key::ShiftLeft | Key::ShiftRight => manipulating.keys.shift = down,
            Key::AltLeft | Key::AltRight => manipulating.keys.alt = down,
            _ => return false,
        }
        if manipulating.moved {
            let point = self.camera.picture_point(state.pointed);
            let reached = manipulating.manipulation.reach(point, manipulating.keys);
            manipulating.reached = Some(reached.clone());
            ctx.request_paint();
            ctx.call(
                &Handler::new(Paint::preview_picked),
                (reached.figures, manipulating.copies),
            );
        }
        true
    }

    fn finish_band(&self, band: Band, ctx: &mut EventCtx<'_>) {
        let area = enclosing([band.from, band.to]);
        if area.width() * self.camera.scale() < SLOP && area.height() * self.camera.scale() < SLOP {
            return;
        }
        let touched_figures = |layer: &Layer| -> Vec<FigureId> {
            layer
                .figures
                .iter()
                .filter(|figure| figure.form.extent().inflate(0.5).intersects(&area))
                .map(|figure| figure.id)
                .collect()
        };
        let active_layer = self
            .figures
            .active_layer
            .and_then(|id| Layer::find(&self.figures.layers, id))
            .filter(|layer| shown_pixels(layer));
        let selection = active_layer
            .map(|layer| (layer.id, touched_figures(layer)))
            .filter(|(_, ids)| !ids.is_empty())
            .or_else(|| {
                visible_pixel_layers(&self.figures.layers)
                    .into_iter()
                    .map(|layer| (layer.id, touched_figures(layer)))
                    .find(|(_, ids)| !ids.is_empty())
            });
        let Some((layer, mut ids)) = selection else {
            return;
        };
        if band.adds && self.picked_layer() == Some(layer) {
            let mut remaining = self.figures.picked.ids();
            remaining.retain(|id| !ids.contains(id));
            remaining.append(&mut ids);
            ids = remaining;
        }
        ctx.call(&Handler::new(Paint::pick_figures), (layer, ids));
    }

    pub(super) fn figure_cursor(&self, at: Point) -> Option<CursorShape> {
        if !self.figures.editable {
            return None;
        }
        let grips = grips_of(&self.shown_picks());
        if let Some(grips) = &grips
            && let Some(grip) = self.grip_under(grips, at)
        {
            return Some(grips.cursor(grip));
        }
        let point = self.camera.picture_point(at);
        let over = self.figure_under(point).is_some()
            || grips.is_some_and(|grips| grips.contains(point, self.reach()));
        over.then_some(CursorShape::Move)
    }
}

fn shown_pixels(layer: &Layer) -> bool {
    layer.kind == LayerKind::Pixel && layer.visible
}

fn visible_pixel_layers(layers: &[Layer]) -> Vec<&Layer> {
    let mut pixel_layers = Vec::new();
    for layer in layers {
        if !layer.visible {
            continue;
        }
        match layer.kind {
            LayerKind::Group => pixel_layers.extend(visible_pixel_layers(&layer.children)),
            LayerKind::Pixel => pixel_layers.push(layer),
            _ => {}
        }
    }
    pixel_layers
}

fn hit(layers: &[Layer], point: Point, reach: f32) -> Option<(LayerId, Figure)> {
    visible_pixel_layers(layers).into_iter().find_map(|layer| {
        layer
            .figures
            .iter()
            .rev()
            .find(|figure| figure.form.hits(point, reach))
            .map(|figure| (layer.id, figure.clone()))
    })
}
