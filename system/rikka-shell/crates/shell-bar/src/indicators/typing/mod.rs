mod mark;
mod panel;

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use rikka_ui::{Element, View};
use shell_ime::InputMethods;

use crate::indicators::{Indicator, Kind, Shelf};

#[derive(Debug)]
pub(crate) struct Typing {
    methods: InputMethods,
}

impl Typing {
    #[must_use]
    pub(crate) fn new(methods: InputMethods) -> Typing {
        Typing { methods }
    }
}

impl Indicator for Typing {
    fn kind(&self) -> Kind {
        Kind::InputMethod
    }

    fn changes(&self) -> BoxStream<'static, ()> {
        self.methods.changes().boxed()
    }

    fn mark(&self) -> Option<Element> {
        mark::badge(self.methods.status_area().current()?).map(View::build)
    }

    fn panel(&self, shelf: &Shelf) -> Element {
        panel::view(&self.methods, shelf).build()
    }

    fn on_open(&self) {
        self.methods.fold();
        if let Some(current) = self.methods.status_area().current() {
            self.methods.trigger(&current.key);
        }
    }
}
