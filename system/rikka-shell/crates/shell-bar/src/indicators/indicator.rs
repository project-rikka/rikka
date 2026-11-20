use futures_util::stream::BoxStream;
use rikka_ui::Element;

use crate::indicators::{Kind, Shelf};

pub(crate) trait Indicator {
    fn kind(&self) -> Kind;

    fn changes(&self) -> BoxStream<'static, ()>;

    fn mark(&self) -> Option<Element>;

    fn panel(&self, shelf: &Shelf) -> Element;

    fn on_open(&self) {}

    fn tick(&self) {}
}
