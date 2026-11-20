use cut_project::{Project, Viewpoint};
use rikka_app::prelude::*;
use rikka_figure_editing::paint_dashed;

use super::Stage;
use super::item_projection::ItemProjection;
use super::items::Item;
use super::paint::dashes;
use super::viewing::Viewing;

const GHOST_ALPHA: f32 = 0.7;

impl Stage {
    pub(super) fn ghosts(
        &self,
        item: &Item,
        view: Viewpoint,
        project: &Project,
    ) -> Vec<Vec<Point>> {
        let format = project.format();
        let frame = self.transport.frame();
        let Some((_, clip)) = project.locate(item.clip) else {
            return Vec::new();
        };
        let Some(tween) = clip.tween() else {
            return Vec::new();
        };
        [tween.start, tween.end]
            .into_iter()
            .filter(|at| *at != frame)
            .filter_map(|at| {
                let visual = project.visual(clip, at)?;
                let shot = project.shot(clip, at, view)?;
                let projection =
                    ItemProjection::new(shot.projection, shot.path, &visual.placement(), format)?;
                projection.all_to_screen(&visual.grips(format)?.outline())
            })
            .collect()
    }

    pub(super) fn paint_ghosts(
        &self,
        ctx: &mut PaintCtx<'_>,
        viewing: &Viewing,
        item: &Item,
        view: Viewpoint,
        project: &Project,
    ) {
        let color = palette().accent.scale_alpha(GHOST_ALPHA);
        for outline in self.ghosts(item, view, project) {
            let outline_on_screen: Vec<Point> = outline
                .into_iter()
                .map(|at| viewing.to_screen(at))
                .collect();
            paint_dashed(ctx, &outline_on_screen, true, &dashes(color));
        }
    }
}
