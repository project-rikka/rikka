mod convex_pile;
mod forms;
mod joint_grid;
mod junkyard;
mod large_world;
mod pyramids;
mod spinner;
mod trees;
mod washer;

use crate::scene::Scene;

pub const LARGE_PYRAMID: Scene = Scene::new("large_pyramid", 200, pyramids::large);
pub const MANY_PYRAMIDS: Scene = Scene::new("many_pyramids", 100, pyramids::many);
pub const JOINT_GRID: Scene = Scene::new("joint_grid", 100, joint_grid::create);
pub const CONVEX_PILE: Scene = Scene::new("convex_pile", 500, convex_pile::create);
pub const JUNKYARD: Scene = Scene::new("junkyard", 500, junkyard::create);
pub const SPINNER: Scene = Scene::new("spinner", 800, spinner::create);
pub const LARGE_WORLD: Scene = Scene::new("large_world", 500, large_world::create);
pub const TREES100: Scene = Scene::new("trees100", 500, trees::create100);
pub const TREES50: Scene = Scene::new("trees50", 500, trees::create50);
pub const TREES25: Scene = Scene::new("trees25", 500, trees::create25);
pub const WASHER: Scene = Scene::new("washer", 1000, washer::create);

pub const SCENES: &[Scene] = &[
    CONVEX_PILE,
    JOINT_GRID,
    JUNKYARD,
    LARGE_PYRAMID,
    LARGE_WORLD,
    MANY_PYRAMIDS,
    SPINNER,
    TREES100,
    TREES50,
    TREES25,
    WASHER,
];
