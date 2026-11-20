use crate::blueprint::Blueprint;
use crate::error::Result;
use crate::size::Size;
use crate::world::World;
pub trait Driver {
    fn drive(&mut self, world: &mut dyn World, step: u32) -> Result<()>;
}

pub struct Setup {
    pub blueprint: Blueprint,
    pub driver: Option<Box<dyn Driver>>,
}

impl Setup {
    pub fn new(blueprint: Blueprint) -> Self {
        Self {
            blueprint,
            driver: None,
        }
    }

    #[must_use]
    pub fn driven(mut self, driver: impl Driver + 'static) -> Self {
        self.driver = Some(Box::new(driver));
        self
    }
}
impl std::fmt::Debug for Setup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Setup")
            .field("blueprint", &self.blueprint)
            .field("driven", &self.driver.is_some())
            .finish()
    }
}

#[derive(Clone, Copy)]
pub struct Scene {
    pub name: &'static str,
    steps: u32,
    build: fn(Size) -> Result<Setup>,
}

impl Scene {
    pub(crate) const fn new(
        name: &'static str,
        steps: u32,
        build: fn(Size) -> Result<Setup>,
    ) -> Self {
        Self { name, steps, build }
    }

    pub fn steps(&self, size: Size) -> u32 {
        size.steps(self.steps)
    }

    pub fn setup(&self, size: Size) -> Result<Setup> {
        (self.build)(size)
    }
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Scene")
            .field("name", &self.name)
            .field("steps", &self.steps)
            .finish()
    }
}
