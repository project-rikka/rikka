mod board;
mod desk_scene;
mod desk_surface;
mod edges;
mod feature;
mod menu;
mod scene;
pub mod widgets;

pub use board::Board;
pub use desk_scene::DeskScene;
pub use feature::Desk;
pub use scene::{Scene, scene};

pub const CAPABILITY: &str = "rikka.desk";

const MENU: &str = "rikka.desk-menu";
