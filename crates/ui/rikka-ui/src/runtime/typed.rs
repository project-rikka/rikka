use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use rikka_theme::Theme;

use crate::runtime::Ui;
use crate::runtime::content::RootState;
use crate::view::{State, ViewFn};

const HELD: &str = "a typed UI holds the state it was made with";

pub struct TypedUi<S: State> {
    ui: Ui,
    state: PhantomData<fn() -> S>,
}

impl<S: State> TypedUi<S> {
    pub fn new(state: S, view: impl for<'a> ViewFn<'a, S>, theme: Theme) -> Self {
        Self {
            ui: Ui::new(state, view, theme),
            state: PhantomData,
        }
    }

    #[must_use]
    pub fn themed(mut self, theme_of: impl Fn(&S) -> Option<Theme> + 'static) -> Self {
        self.root_mut().theme_of = Some(Box::new(theme_of));
        self.ui.needs_view = true;
        self.ui.everything = true;
        self
    }

    pub fn state(&self) -> &S {
        &self.root().state
    }

    pub fn update(&mut self, update: impl FnOnce(&mut S)) {
        let _identified = self.ui.identified();
        let plain = self.state().has_plain();
        let ((), effects) = crate::task::collecting(|| {
            match plain {
                true => crate::task::changed(),
                false => crate::task::acted(),
            }
            update(&mut self.root_mut().state);
        });
        self.ui.absorb(effects);
    }

    pub fn into_ui(self) -> Ui {
        self.ui
    }

    fn root(&self) -> &RootState<S> {
        self.ui.content.as_any().downcast_ref().expect(HELD)
    }

    fn root_mut(&mut self) -> &mut RootState<S> {
        self.ui.content.as_any_mut().downcast_mut().expect(HELD)
    }
}

impl<S: State> From<TypedUi<S>> for Ui {
    fn from(typed: TypedUi<S>) -> Ui {
        typed.ui
    }
}

impl<S: State> Deref for TypedUi<S> {
    type Target = Ui;

    fn deref(&self) -> &Ui {
        &self.ui
    }
}

impl<S: State> DerefMut for TypedUi<S> {
    fn deref_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
}

impl<S: State> std::fmt::Debug for TypedUi<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.ui.fmt(f)
    }
}
