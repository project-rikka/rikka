use cut_project::{Format, Region, Viewpoint, Visual};
use cut_render::{Overrides, Request};
use rikka_app::prelude::*;
use rikka_figure_editing::{paint_band, paint_dashed, paint_grips, paint_line, paint_loop};

use super::anchor::paint_anchor;
use super::gesture::Gesture;
use super::guides::Guide;
use super::item_projection::ItemProjection;
use super::items::{Item, group};
use super::route::Route;
use super::viewing::Viewing;
use super::{PointerState, Stage};
use crate::tool::Tool;
use crate::{art, wording};

const NOTE: f32 = 13.0;
const NOTE_LINE: f32 = 18.0;
const MARGIN: f32 = 12.0;
const MEMBER_ALPHA: f32 = 0.6;
const DASH: f32 = 6.0;
const GAP: f32 = 4.0;

impl Stage {
    pub(super) fn paint_stage(&self, state: &mut PointerState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        ctx.draw_quad(Quad::new(bounds).fill(palette().surface_variant));
        let viewing = Viewing::new(bounds, self.format());
        let density = ctx.scale().max(f32::EPSILON);
        let frame = snapped(viewing.frame, density);
        ctx.draw_quad(Quad::new(frame).fill(Color::BLACK));
        let view = state
            .gesture
            .as_ref()
            .and_then(Gesture::viewpoint)
            .unwrap_or_else(|| self.view());
        let overrides = self.overrides();
        let items = self.items_seen(view, &overrides);
        self.preview.watch(ctx.handle());
        let peek = self.live.peek();
        let request = self.request(
            state,
            view,
            overrides.clone(),
            viewing.scale * density,
            peek,
        );
        self.preview.submit(request);
        if let Some(picture) = self.preview.picture() {
            ctx.draw_image(frame, picture);
        }
        if peek.is_some() {
            return;
        }
        ctx.clipped(frame, |ctx| self.paint_offline(ctx, &items, &viewing));
        self.paint_guides(state, ctx, &viewing);
        state.caret = self.paint_typing(ctx, &viewing, &items);
        if state.caret.is_none() {
            match self.framing(view_scale(view)) {
                Some(framing) => framing.paint(ctx, &viewing),
                None => {
                    self.paint_ranges(state, ctx, &viewing, &items);
                    self.paint_selected(state, ctx, &viewing, &items, view, &overrides);
                }
            }
        }
        if let Some(Gesture::Banding { from, to, .. }) = &state.gesture {
            paint_band(ctx, *from, *to);
        }
    }

    fn request(
        &self,
        state: &PointerState,
        view: Viewpoint,
        overrides: Overrides,
        display_scale: f32,
        peek: Option<i64>,
    ) -> Request {
        let mut overrides = overrides;
        if let Some(Gesture::Drawing { tool, from, to, .. }) = &state.gesture {
            overrides.draw(art::drawn(*tool, self.format(), *from, *to));
        }
        let frame = peek.unwrap_or_else(|| self.transport.frame());
        Request::new(self.shown_project(), frame)
            .scale(self.quality.scale(display_scale))
            .viewpoint(view)
            .overrides(overrides)
            .playing(self.transport.playing().filter(|_| peek.is_none()))
            .media_round(self.preview.media_round())
    }

    fn paint_offline(&self, ctx: &mut PaintCtx<'_>, items: &[Item], viewing: &Viewing) {
        let offline = self.preview.offline();
        if offline.is_empty() {
            return;
        }
        let format = self.format();
        for item in items {
            if !matches!(item.visual, Visual::Footage { .. })
                || !item.media.is_some_and(|media| offline.contains(&media))
            {
                continue;
            }
            let Some(bounds) = item.bounds(format) else {
                continue;
            };
            let (low, high) = (
                viewing.to_screen(Point::new(bounds.min_x(), bounds.min_y())),
                viewing.to_screen(Point::new(bounds.max_x(), bounds.max_y())),
            );
            let note = TextRun::new(wording::offline(), NOTE)
                .no_wrap()
                .ellipsized();
            ctx.draw_text(
                Rect::new(
                    low.x + MARGIN,
                    (low.y + high.y) / 2.0 - NOTE_LINE / 2.0,
                    (high.x - low.x - MARGIN * 2.0).max(0.0),
                    NOTE_LINE,
                ),
                note.align(TextAlign::Center),
                palette().danger,
            );
        }
    }

    fn paint_ranges(
        &self,
        state: &PointerState,
        ctx: &mut PaintCtx<'_>,
        viewing: &Viewing,
        items: &[Item],
    ) {
        let format = self.format();
        for item in self.selected_items(items) {
            if let (Visual::Region(region), Some(projection)) = (&item.visual, &item.projection) {
                paint_region(ctx, viewing, region, format, Some(projection));
            }
        }
        if let Some(Gesture::Drawing {
            tool: Tool::Range(kind),
            from,
            to,
            ..
        }) = &state.gesture
        {
            paint_region(
                ctx,
                viewing,
                &art::ranged(*kind, format, *from, *to),
                format,
                None,
            );
        }
    }

    fn paint_guides(&self, state: &PointerState, ctx: &mut PaintCtx<'_>, viewing: &Viewing) {
        let Some(Gesture::Manipulating(manipulating)) = &state.gesture else {
            return;
        };
        let format = self.format();
        let (width, height) = (format.width as f32, format.height as f32);
        let color = palette().warning;
        for guide in manipulating.guides() {
            let (from, to) = match *guide {
                Guide::Down(x) => (Point::new(x, 0.0), Point::new(x, height)),
                Guide::Across(y) => (Point::new(0.0, y), Point::new(width, y)),
            };
            paint_line(ctx, viewing.to_screen(from), viewing.to_screen(to), color);
        }
    }

    fn paint_selected(
        &self,
        state: &PointerState,
        ctx: &mut PaintCtx<'_>,
        viewing: &Viewing,
        items: &[Item],
        view: Viewpoint,
        overrides: &Overrides,
    ) {
        let format = self.format();
        let selected = self.selected_items(items);
        let drawing = matches!(
            state.gesture,
            Some(Gesture::Drawing { .. } | Gesture::Banding { .. })
        );
        if drawing {
            return;
        }
        if let [item] = selected[..] {
            if let Some(project) = self.animated_preview(item.clip, overrides) {
                self.paint_ghosts(ctx, viewing, item, view, &project);
                if let Some(route) = Route::for_item(item, &project) {
                    route.paint(ctx, viewing, self.transport.frame());
                }
            }
            if let Some(gimbal) = self.gimbal(item, view, overrides, viewing) {
                gimbal.paint(ctx);
            }
        }
        let grouped = group(&selected, format);
        if selected.len() > 1 || grouped.is_none() {
            let member = palette().accent.scale_alpha(MEMBER_ALPHA);
            for corners in selected.iter().filter_map(|item| item.outline(format)) {
                let corners: Vec<Point> = corners
                    .into_iter()
                    .map(|at| viewing.to_screen(at))
                    .collect();
                paint_loop(ctx, &corners, member);
            }
        }
        if let Some((grips, projection)) = grouped {
            let extent = grips.bounds();
            if extent.width() + extent.height() > 0.0
                && projection.grip_spots(&grips, viewing).is_some()
            {
                let to_screen =
                    |spot: Point| viewing.to_screen(projection.to_screen(spot).unwrap_or(spot));
                paint_grips(
                    ctx,
                    &grips,
                    &to_screen,
                    projection.turn_scale(&grips, viewing),
                );
            }
        }
        if let [item] = selected[..] {
            paint_anchor(ctx, viewing, item);
        }
    }
}

fn view_scale(view: Viewpoint) -> Option<f32> {
    match view {
        Viewpoint::Overview { scale, .. } => Some(scale),
        Viewpoint::Output => None,
    }
}

fn snapped(rect: Rect, density: f32) -> Rect {
    let snap = |value: f32| (value * density).round() / density;
    let (left, top) = (snap(rect.min_x()), snap(rect.min_y()));
    Rect::new(
        left,
        top,
        snap(rect.max_x()) - left,
        snap(rect.max_y()) - top,
    )
}

fn paint_region(
    ctx: &mut PaintCtx<'_>,
    viewing: &Viewing,
    region: &Region,
    format: Format,
    projection: Option<&ItemProjection>,
) {
    let Some(path) = region.path(format) else {
        return;
    };
    let color = palette().accent;
    for subpath in path.subpaths() {
        let points: Option<Vec<Point>> = subpath
            .points()
            .iter()
            .map(|point| match projection {
                Some(projection) => projection.to_screen(*point).map(|at| viewing.to_screen(at)),
                None => Some(viewing.to_screen(*point)),
            })
            .collect();
        if let Some(points) = points {
            paint_dashed(ctx, &points, subpath.is_closed(), &dashes(color));
        }
    }
}

pub(super) fn dashes(color: Color) -> [(f32, Color); 2] {
    [(DASH, color), (GAP, Color::TRANSPARENT)]
}
