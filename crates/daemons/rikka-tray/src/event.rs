use crate::item::Item;
use crate::menu::Menu;

#[derive(Clone, Debug)]
pub enum Event {
    Changed(Vec<Item>),
    Opened(Box<Menu>),
    Closed,
}
