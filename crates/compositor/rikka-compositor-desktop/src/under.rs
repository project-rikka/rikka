use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_types::OutputId;

use crate::desktop::Desktop;
use crate::scene_layers::SceneLayers;
use crate::tags::Tags;
use crate::target::{Hit, Target, Under};
use crate::trees::SurfaceTrees;
use crate::walk::VisitedPopup;

impl<P> Desktop<P> {
    pub fn element_under(
        &self,
        trees: &impl SurfaceTrees,
        point: Point<f64, Logical>,
        around: impl FnOnce(OutputId) -> SceneLayers,
    ) -> Option<Under> {
        self.hit_under(trees, point, around).and_then(Hit::surface)
    }

    pub fn hit_under(
        &self,
        trees: &impl SurfaceTrees,
        point: Point<f64, Logical>,
        around: impl FnOnce(OutputId) -> SceneLayers,
    ) -> Option<Hit> {
        let output = self.output_at(point)?;
        self.walk_top_down(
            trees,
            output,
            around(output),
            &mut |visited: VisitedPopup| {
                if self.passive(visited.target) {
                    return None;
                }
                trees
                    .surface_under(visited.root, point - visited.location.to_f64())
                    .map(|(surface, local)| {
                        Hit::Surface(Under {
                            target: visited.target,
                            surface,
                            local,
                        })
                    })
                    .or_else(|| self.frame_hit(visited.target, point))
            },
        )
    }

    fn frame_hit(&self, target: Target, point: Point<f64, Logical>) -> Option<Hit> {
        let Target::Window(id) = target else {
            return None;
        };
        let window = self.windows.get(id)?;
        let frame = window.frame;
        if frame.is_none() {
            return None;
        }
        let reached = frame.reached(window.geometry).to_f64();
        let content = window.content().to_f64();
        (reached.contains(point) && !content.contains(point)).then(|| Hit::Frame {
            window: id,
            local: point - window.geometry.loc.to_f64(),
        })
    }

    fn passive(&self, target: Target) -> bool {
        let Target::Window(window) = target else {
            return false;
        };
        self.windows
            .get(window)
            .is_some_and(|window| window.tags().contains(Tags::PASSIVE))
    }
}
