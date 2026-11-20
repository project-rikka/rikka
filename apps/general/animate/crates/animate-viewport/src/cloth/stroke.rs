use std::sync::Arc;

use animate_cast::FigureId;
use animate_cloth::{Brush, Dab};
use animate_stage::{Lens, Stage};
use rikka_app::prelude::*;
use rikka_physics_tailor::ClothSettings;

const SPACING: f32 = 0.3;

#[derive(Debug)]
pub(crate) struct Stroke {
    figure: FigureId,
    before: Arc<ClothSettings>,
    working: ClothSettings,
    last: Point,
    erase: bool,
}

impl Stroke {
    pub(crate) fn begin(
        stage: &Stage,
        figure: FigureId,
        brush: &Brush,
        frame: Rect,
        at: Point,
        erase: bool,
    ) -> Option<Self> {
        let before = Arc::clone(&stage.figure_cloth_for(figure)?.cloth_settings);
        let mut stroke = Self {
            figure,
            working: (*before).clone(),
            before,
            last: at,
            erase,
        };
        stroke.dab(stage, brush, frame, at).then_some(stroke)
    }

    pub(crate) fn figure(&self) -> FigureId {
        self.figure
    }

    pub(crate) fn working(&self) -> &ClothSettings {
        &self.working
    }

    pub(crate) fn reach(&mut self, stage: &Stage, brush: &Brush, frame: Rect, at: Point) {
        let travel = (at.x - self.last.x).hypot(at.y - self.last.y);
        let spacing = (brush.radius * SPACING).max(1.0);
        let steps = (travel / spacing).ceil().max(1.0) as usize;
        let from = self.last;
        for step in 1..=steps {
            let share = step as f32 / steps as f32;
            let between = Point::new(
                from.x + (at.x - from.x) * share,
                from.y + (at.y - from.y) * share,
            );
            self.dab(stage, brush, frame, between);
        }
        self.last = at;
    }

    pub(crate) fn finish(self) -> Option<(FigureId, Arc<ClothSettings>)> {
        (self.working != *self.before).then(|| (self.figure, Arc::new(self.working)))
    }

    fn dab(&mut self, stage: &Stage, brush: &Brush, frame: Rect, at: Point) -> bool {
        let Some(paint_surface) = stage
            .paint_surface()
            .filter(|paint_surface| paint_surface.figure == self.figure)
        else {
            return false;
        };
        let lens = Lens::new(paint_surface.view_projection, frame);
        let (origin, direction) = lens.ray(at);
        let Some(hit) = paint_surface.surface.cast(origin, direction) else {
            return false;
        };
        let (_, _, pixel) = lens.sideways(hit.point);
        let mirror = stage
            .figure_cloth_for(self.figure)
            .map(|figure_cloth| Arc::clone(&figure_cloth.mirror));
        brush.dab(
            &mut self.working,
            &paint_surface.surface,
            mirror.as_deref(),
            Dab {
                centre: hit.point,
                radius: brush.radius * pixel,
                erase: self.erase,
            },
        );
        true
    }
}
