mod egui;
mod gpui;
mod iced;
mod rikka;

use ui_scene::{Grid, Table};

pub use self::egui::Egui;
pub use self::gpui::{Gpui, GpuiFast};
pub use self::iced::Iced;
pub use self::rikka::Rikka;

#[derive(Clone, Debug)]
pub enum Screen {
    Table(Table),
    Grid(Grid),
}

impl Screen {
    pub fn table(&mut self) -> &mut Table {
        match self {
            Screen::Table(table) => table,
            Screen::Grid(_) => panic!("the screen shows a grid, not a table"),
        }
    }

    pub fn grid(&mut self) -> &mut Grid {
        match self {
            Screen::Grid(grid) => grid,
            Screen::Table(_) => panic!("the screen shows a table, not a grid"),
        }
    }
}

pub trait Frontend: Sized {
    type Host;

    fn host() -> Self::Host;

    fn open(host: Self::Host, screen: Screen) -> Self;

    fn update(&mut self, change: impl FnOnce(&mut Screen));
}
