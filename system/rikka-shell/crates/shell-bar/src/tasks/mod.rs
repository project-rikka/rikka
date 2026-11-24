mod art;
mod menu;
mod state;
mod task_icon;
mod task_list;
mod thumbnail;
mod tile;

pub(crate) use art::Art;
pub(crate) use menu::{Flyout, MenuRequest, Opened, view as menu_view};
pub(crate) use state::Tasks;
pub(crate) use task_list::{Aim, TaskList, Touch, task_list};
pub(crate) use thumbnail::thumbnail;
pub(crate) use tile::Tile;

const LIFTED: f32 = 1.12;

const BORDER: f32 = 1.0;
