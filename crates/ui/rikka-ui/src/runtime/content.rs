use std::any::Any;

use rikka_theme::Theme;

use crate::view::{Element, State, ViewFn};

type Render<S> = Box<dyn Fn(&S) -> Element>;

type ThemeFn<S> = Box<dyn Fn(&S) -> Option<Theme>>;

pub(super) trait Content {
    fn state(&mut self) -> &mut dyn State;

    fn view(&self) -> Element;

    fn theme(&self) -> Option<Theme>;

    fn name(&self) -> &'static str;

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub(super) struct RootState<S> {
    pub(super) state: S,
    view: Render<S>,
    pub(super) theme_of: Option<ThemeFn<S>>,
}

impl<S: State> RootState<S> {
    pub(super) fn new(state: S, view: impl for<'a> ViewFn<'a, S>) -> Self {
        Self {
            state,
            view: Box::new(move |state| view.build(state)),
            theme_of: None,
        }
    }
}

impl<S: State> Content for RootState<S> {
    fn state(&mut self) -> &mut dyn State {
        &mut self.state
    }

    fn view(&self) -> Element {
        (self.view)(&self.state)
    }

    fn theme(&self) -> Option<Theme> {
        self.theme_of.as_ref()?(&self.state)
    }

    fn name(&self) -> &'static str {
        std::any::type_name::<S>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
