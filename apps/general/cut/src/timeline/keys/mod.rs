mod gesture;
mod glyph;
pub(crate) mod graph;
mod header;
mod marks;
mod menu;
mod names;
mod outline;
mod shift;
mod strips;

use std::collections::BTreeSet;
use std::sync::Arc;

use cut_project::{Clip, KeyRef, Project, Property};
use rikka_app::prelude::*;

pub(crate) use header::header;
pub(crate) use menu::{context_menu, pin};
pub(crate) use outline::Outline;

use crate::cut::{Cut, Plot};
use crate::live::LivePreview;
use crate::timeline::scale::Scale;
use crate::transport::Transport;
use gesture::Grab;
use graph::{Drawing, Plotting};
use strips::Strips;

pub(crate) const KEY: &str = "cut-keys";

const BAND: f32 = 0.16;
const BAND_LINE: f32 = 1.0;
const SNAP_LINE: f32 = 1.5;

#[derive(Debug)]
pub(crate) struct Keys {
    pub(crate) project: Arc<Project>,
    pub(crate) live: LivePreview,
    pub(crate) outline: Outline,
    pub(crate) picked: Arc<BTreeSet<KeyRef>>,
    pub(crate) plotted: Arc<BTreeSet<Property>>,
    pub(crate) plot: Plot,
    pub(crate) snapping: bool,
    pub(crate) scale: Scale,
    pub(crate) transport: Transport,
    pub(crate) top: f32,
}

#[derive(Debug, Default)]
pub(crate) struct PointerState {
    grab: Option<Grab>,
    frozen: Option<Plotting>,
}

impl Keys {
    fn local(bounds: Rect, point: Point) -> Point {
        Point::new(point.x - bounds.x(), point.y - bounds.y())
    }

    fn clip(&self) -> Option<&Clip> {
        self.project.locate(self.outline.clip).map(|(_, clip)| clip)
    }

    fn plotting(&self, clip: &Clip, width: f32, frozen: Option<&Plotting>) -> Plotting {
        Plotting::new(
            clip,
            &self.outline,
            (self.plot, &self.plotted),
            self.outline.graph_area(width),
            frozen,
        )
    }

    fn shown_picked(
        &self,
        state: &PointerState,
        clip: &Clip,
        plotting: &Plotting,
    ) -> BTreeSet<KeyRef> {
        match &state.grab {
            Some(Grab::Shifting(shift, _)) => shift.moved().into_iter().collect(),
            Some(Grab::Banding(banding)) => {
                let mut selected: BTreeSet<KeyRef> = banding.base.iter().cloned().collect();
                selected.extend(self.caught(clip, plotting, banding.band.rect()));
                selected
            }
            _ => (*self.picked).clone(),
        }
    }

    fn paint_grab(&self, ctx: &mut PaintCtx<'_>, state: &PointerState, bounds: Rect) {
        let palette = palette();
        match &state.grab {
            Some(Grab::Banding(banding)) => {
                let band = banding.band.rect();
                ctx.draw_quad(
                    Quad::new(Rect::new(
                        bounds.x() + band.x(),
                        bounds.y() + band.y(),
                        band.width(),
                        band.height(),
                    ))
                    .fill(palette.selection.scale_alpha(BAND))
                    .stroke(BAND_LINE, palette.selection),
                );
            }
            Some(Grab::Shifting(shift, _)) => {
                if let Some(frame) = shift.snapped() {
                    let x = bounds.x() + self.scale.x_of(frame as f64);
                    ctx.draw_quad(
                        Quad::new(Rect::new(
                            x - SNAP_LINE / 2.0,
                            bounds.y(),
                            SNAP_LINE,
                            bounds.height(),
                        ))
                        .fill(palette.warning),
                    );
                }
            }
            _ => {}
        }
    }

    fn scroll(
        &self,
        local: Point,
        delta: ScrollDelta,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) {
        let end = self.project.end();
        let offset = delta.offset();
        if modifiers.control || modifiers.alt {
            self.scale.zoom_notches(delta, local.x);
        } else if modifiers.shift {
            self.scale.scroll_by(offset.y + offset.x, end);
        } else if offset.x != 0.0 {
            self.scale.scroll_by(offset.x, end);
        } else {
            return;
        }
        ctx.set_handled();
    }
}

impl Widget for Keys {
    type State = PointerState;

    fn paint(&self, state: &mut PointerState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let project = self.live.project_or(&self.project);
        let Some((_, clip)) = project.locate(self.outline.clip) else {
            return;
        };
        let plotting = self.plotting(clip, bounds.width(), state.frozen.as_ref());
        let picked = self.shown_picked(state, clip, &plotting);
        let strips = Strips {
            clip,
            outline: &self.outline,
            picked: &picked,
            scale: &self.scale,
            bounds,
        };
        strips.paint_backdrop(ctx);
        if self.outline.graph {
            Drawing {
                plotting: &plotting,
                clip,
                picked: &picked,
                scale: &self.scale,
                origin: bounds.origin,
                width: bounds.width(),
            }
            .paint(ctx);
        }
        strips.paint_marks(ctx);
        self.paint_grab(ctx, state, bounds);
    }

    fn event(&self, state: &mut PointerState, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        match event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                modifiers,
            } => {
                ctx.capture_pointer();
                ctx.keep_focus();
                ctx.set_handled();
                self.press(state, Self::local(bounds, *position), *modifiers, ctx);
                ctx.request_paint();
            }
            Event::PointerPressed {
                position,
                button: PointerButton::Secondary,
                ..
            } => {
                ctx.keep_focus();
                ctx.set_handled();
                self.context(Self::local(bounds, *position), ctx);
            }
            Event::PointerMoved(position) => {
                if self.drag(state, Self::local(bounds, *position), ctx) {
                    ctx.set_handled();
                    ctx.request_paint();
                }
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } if ctx.captured() => {
                ctx.release_pointer();
                ctx.set_handled();
                self.release(state, ctx);
                ctx.request_paint();
            }
            Event::Scroll {
                position,
                delta,
                modifiers,
                ..
            } => self.scroll(Self::local(bounds, *position), *delta, *modifiers, ctx),
            _ => {}
        }
    }

    fn behavior(&self, _held: &PointerState) -> Behavior {
        Behavior::CLIPS
    }

    fn same(&self, other: &Keys) -> bool {
        Arc::ptr_eq(&self.project, &other.project)
            && self.live.same(&other.live)
            && self.outline == other.outline
            && self.picked == other.picked
            && self.plotted == other.plotted
            && self.plot == other.plot
            && self.snapping == other.snapping
            && self.scale == other.scale
            && self.transport == other.transport
            && self.top == other.top
    }
}

impl View for Keys {
    fn build(self) -> Element {
        let height = self.outline.height();
        Element::new(self).height(height)
    }
}

pub(crate) fn keys(cut: &Cut, outline: Outline, top: f32) -> Element {
    let keyframing = cut.keyframing();
    Keys {
        project: Arc::clone(cut.project()),
        live: cut.live().clone(),
        outline,
        picked: cut.picked_keys(),
        plotted: Arc::clone(keyframing.plotted()),
        plot: keyframing.plot(),
        snapping: cut.snapping(),
        scale: cut.scale().clone(),
        transport: cut.transport().clone(),
        top,
    }
    .build()
    .keyed(KEY)
}
