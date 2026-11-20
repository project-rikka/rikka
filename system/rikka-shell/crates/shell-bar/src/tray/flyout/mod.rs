mod open;
mod state;
mod view;

pub(crate) use state::Flyout;
pub(crate) use view::view;

const WIDTH: f32 = 268.0;

const ICON: f32 = 20.0;

const MENU_WIDTH: f32 = 236.0;

const ROW: &str = "bar.tray.row";
const MENU: &str = "bar.tray.menu";
