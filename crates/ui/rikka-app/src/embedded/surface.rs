use std::any::type_name;

use rikka_theme::Theme;
use rikka_ui::{Element, Handler, IntoHandler, State, TypedUi, ViewFn};

use crate::sender::Sender;
use crate::window::Factory;

type MakeFn<S> = Box<dyn FnOnce(Sender<S>) -> S>;
type ViewFor<S> = Box<dyn Fn(&S) -> Element>;

#[must_use]
pub struct Embedded<S: State> {
    make: MakeFn<S>,
    view: ViewFor<S>,
    start: Option<Handler>,
    close: Option<Handler>,
}

impl<S: State> Embedded<S> {
    pub fn new<M>(factory: impl Factory<S, M>, view: impl for<'a> ViewFn<'a, S>) -> Self {
        Self {
            make: Box::new(move |sender| factory.make(sender)),
            view: Box::new(move |state| view.build(state)),
            start: None,
            close: None,
        }
    }

    pub fn on_start<M>(mut self, start: impl IntoHandler<(), (), M>) -> Self {
        self.start = Some(start.into_handler());
        self
    }

    pub fn on_close_request<M>(mut self, close: impl IntoHandler<(), (), M>) -> Self {
        self.close = Some(close.into_handler());
        self
    }

    pub(crate) fn into_ui(self, sender: Sender<S>, theme: Theme) -> (TypedUi<S>, Option<Handler>) {
        let Embedded {
            make,
            view,
            start,
            close,
        } = self;
        let mut ui = TypedUi::new(make(sender), move |state: &S| view(state), theme);
        if let Some(start) = start {
            ui.start(start);
        }
        (ui, close)
    }
}

impl<S: State> std::fmt::Debug for Embedded<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Embedded")
            .field("state", &type_name::<S>())
            .finish_non_exhaustive()
    }
}
