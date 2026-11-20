use std::sync::Arc;

use rikka_gpu::Compute;
use rikka_physics_fluid::{BodyId, Replay};

use crate::cloth_scene::ClothScene;
use crate::error::Result;
use crate::fluid_bodies::add_bodies;
use crate::layout::Layout;
use crate::scene_frame::SceneFrame;
use crate::snapshot::Capture;

pub(super) struct SceneReplay {
    pub(super) layout: Arc<Layout>,
    compute: Compute,
    fluid: Replay,
    bodies: Vec<(usize, BodyId)>,
    cloth_scene: ClothScene,
    scene_frame: SceneFrame,
}

impl SceneReplay {
    pub(super) fn build(layout: Arc<Layout>, compute: &Compute) -> Result<SceneReplay> {
        let mut fluid = layout.fluid.clone().replay(compute)?;
        let bodies = add_bodies(&layout, |model| fluid.add_body(model))?;
        let cloth_scene = ClothScene::build(&layout, compute)?;
        let scene_frame = scene_frame(&layout, &fluid, &bodies, &cloth_scene);
        Ok(SceneReplay {
            layout,
            compute: compute.clone(),
            fluid,
            bodies,
            cloth_scene,
            scene_frame,
        })
    }

    pub(super) fn show(&mut self, capture: &Capture) -> Result<()> {
        let mut batch = self.compute.batch();
        let fluid = self.fluid.show_into(&mut batch, &capture.fluid)?;
        let cloth = match (self.cloth_scene.cloth.as_mut(), &capture.cloth) {
            (Some(cloth), Some(capture)) => Some(cloth.show_into(&mut batch, capture)?),
            _ => None,
        };
        let ready = batch.submit()?;
        self.fluid.commit(fluid, &ready);
        if let (Some(cloth), Some(drapes)) = (self.cloth_scene.cloth.as_mut(), cloth) {
            cloth.commit(drapes, &ready);
        }
        self.scene_frame = scene_frame(&self.layout, &self.fluid, &self.bodies, &self.cloth_scene);
        Ok(())
    }

    pub(super) fn frame(&self) -> SceneFrame {
        self.scene_frame.clone()
    }
}

fn scene_frame(
    layout: &Layout,
    fluid: &Replay,
    bodies: &[(usize, BodyId)],
    cloth_scene: &ClothScene,
) -> SceneFrame {
    SceneFrame::new(
        fluid.surface(),
        layout,
        bodies,
        |body| fluid.wetness(body),
        cloth_scene.drapes(layout),
    )
}
