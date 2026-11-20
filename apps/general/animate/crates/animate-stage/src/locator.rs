use std::sync::Arc;

use animate_cast::time_of;
use rikka_model::Instance;
use rikka_model::glam::Mat4;

use super::Stage;
use super::actor::Actor;

pub struct Locator {
    actors: Vec<Actor>,
}

impl std::fmt::Debug for Locator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Locator")
            .field("actors", &self.actors.len())
            .finish()
    }
}

impl Locator {
    pub fn locate(&mut self, figure: &str, bone: &str, frame: u32) -> Option<Mat4> {
        let actor = self.actors.iter_mut().find(|actor| actor.name == figure)?;
        actor.rested = false;
        actor.animate(time_of(frame));
        actor.instance.refresh();
        actor.bone_world(bone)
    }
}

impl Stage {
    pub fn locator(&self) -> Locator {
        Locator {
            actors: self
                .scene
                .members
                .iter()
                .map(|member| Actor::cast(member.clone(), Instance::new(Arc::clone(&member.model))))
                .collect(),
        }
    }
}
