use cut_project::Side;
use rikka_app::prelude::*;
use rikka_timeline::CurvePlot;
use rikka_ui::Action;

const SIDE: f32 = 208.0;
const INSET: f32 = 12.0;
const MARGIN: f32 = 0.06;
const END: f32 = 6.0;
const SIDES: [Side; 2] = [Side::Outgoing, Side::Incoming];
const DIVISIONS: usize = 4;
const GRID: f32 = 0.5;
const STROKE: f32 = 1.5;

#[derive(Debug)]
pub(crate) struct SegmentGraph {
    pub(crate) curve: Vec<Point>,
    pub(crate) handles: [Point; 2],
    pub(crate) end: f32,
    pub(crate) on_drag: Action<(Side, Point)>,
    pub(crate) on_release: Action<(Side, Point)>,
}

#[derive(Debug, Default)]
pub(crate) struct Grab {
    drag: Option<HandleDrag>,
}

#[derive(Clone, Copy, Debug)]
struct HandleDrag {
    side: Side,
    plot: CurvePlot,
    at: Point,
    moved: bool,
}

impl SegmentGraph {
    fn plot(&self, bounds: Rect) -> CurvePlot {
        let values = self
            .curve
            .iter()
            .chain(&self.handles)
            .map(|point| point.y)
            .chain([self.end]);
        CurvePlot::fitting(bounds.inflate(-INSET), values, MARGIN)
    }

    fn handle_positions(&self, grab: &Grab, bounds: Rect) -> (CurvePlot, [Point; 2]) {
        match grab.drag {
            Some(drag) => {
                let mut handles = self.handles;
                handles[side_index(drag.side)] = drag.at;
                (drag.plot.with_area(bounds.inflate(-INSET)), handles)
            }
            None => (self.plot(bounds), self.handles),
        }
    }

    fn grabbed(&self, position: Point, bounds: Rect) -> Option<Side> {
        let index = self.plot(bounds).knob_at(&self.handles, position)?;
        Some(SIDES[index])
    }
}

fn side_index(side: Side) -> usize {
    match side {
        Side::Outgoing => 0,
        Side::Incoming => 1,
    }
}

impl Widget for SegmentGraph {
    type State = Grab;

    fn paint(&self, grab: &mut Grab, ctx: &mut PaintCtx<'_>) {
        let palette = ctx.palette();
        let bounds = ctx.bounds();
        ctx.draw_quad(
            Quad::new(bounds)
                .fill(palette.surface_variant)
                .radius(Corner::Item),
        );
        let (plot, handles) = self.handle_positions(grab, bounds);
        let area = bounds.inflate(-INSET);
        let faint = palette.outline.scale_alpha(GRID);
        for step in 1..DIVISIONS {
            let along = step as f32 / DIVISIONS as f32;
            let x = area.x() + area.width() * along;
            ctx.draw_quad(Quad::new(Rect::new(x, area.y(), 1.0, area.height())).fill(faint));
        }
        for level in [0.0, self.end] {
            let y = plot.place(Point::new(0.0, level)).y;
            ctx.draw_quad(Quad::new(Rect::new(area.x(), y, area.width(), 1.0)).fill(faint));
        }
        let ends = [Point::new(0.0, 0.0), Point::new(1.0, self.end)];
        for (end, handle) in ends.iter().zip(handles) {
            plot.line(ctx, &[*end, handle], STROKE, palette.text_muted);
        }
        plot.line(ctx, &self.curve, STROKE, palette.accent);
        for end in ends {
            ctx.draw_quad(
                Quad::new(Rect::from_center(plot.place(end), Size::new(END, END)))
                    .fill(palette.text),
            );
        }
        let drag = grab.drag.map(|handle_drag| handle_drag.side);
        for side in SIDES {
            plot.knob(
                ctx,
                handles[side_index(side)],
                drag == Some(side),
                palette.accent,
            );
        }
    }

    fn event(&self, grab: &mut Grab, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        match *event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                let Some(side) = self.grabbed(position, bounds) else {
                    return;
                };
                ctx.set_handled();
                ctx.keep_focus();
                ctx.capture_pointer();
                grab.drag = Some(HandleDrag {
                    side,
                    plot: self.plot(bounds),
                    at: self.handles[side_index(side)],
                    moved: false,
                });
                ctx.request_paint();
            }
            Event::PointerMoved(position) => {
                let Some(drag) = &mut grab.drag else {
                    return;
                };
                ctx.set_handled();
                drag.at = drag
                    .plot
                    .with_area(bounds.inflate(-INSET))
                    .unplace(position);
                drag.moved = true;
                let (side, at) = (drag.side, drag.at);
                ctx.act(&self.on_drag, (side, at));
                ctx.request_paint();
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => {
                let Some(drag) = grab.drag.take() else {
                    return;
                };
                ctx.set_handled();
                ctx.release_pointer();
                if drag.moved {
                    ctx.act(&self.on_release, (drag.side, drag.at));
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn behavior(&self, _grab: &Grab) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn same(&self, other: &SegmentGraph) -> bool {
        self.curve == other.curve && self.handles == other.handles && self.end == other.end
    }
}

impl View for SegmentGraph {
    fn build(self) -> Element {
        Element::new(self).size(SIDE, SIDE)
    }
}
