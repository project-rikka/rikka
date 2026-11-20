use rikka_ui::fetch::Fetch;
use rikka_ui::text::{Label, TextStyled, text};
use rikka_ui::{Align, Element, Styled, View};

use crate::blocks::container::container;
use crate::controls::spinner::spinner;

type Made = Box<dyn Fn() -> Element>;
type Ready<T> = Box<dyn Fn(&T) -> Element>;
type ErrorView = Box<dyn Fn(&str) -> Element>;

#[must_use]
pub struct AsyncView<T> {
    fetch: Fetch<T>,
    ready: Ready<T>,
    loading: Option<Made>,
    failed: Option<ErrorView>,
}

pub fn async_view<T: 'static, V: View>(
    fetch: Fetch<T>,
    ready: impl Fn(&T) -> V + 'static,
) -> AsyncView<T> {
    AsyncView {
        fetch,
        ready: Box::new(move |value| ready(value).build()),
        loading: None,
        failed: None,
    }
}

impl<T: 'static> AsyncView<T> {
    pub fn loading<V: View>(mut self, loading: impl Fn() -> V + 'static) -> Self {
        self.loading = Some(Box::new(move || loading().build()));
        self
    }

    pub fn failed<V: View>(mut self, failed: impl Fn(&str) -> V + 'static) -> Self {
        self.failed = Some(Box::new(move |why| failed(why).build()));
        self
    }
}

impl<T: 'static> View for AsyncView<T> {
    fn build(self) -> Element {
        match &self.fetch {
            Fetch::Ready(value) => (self.ready)(value),
            Fetch::Loading => match &self.loading {
                Some(loading) => loading(),
                None => container(spinner())
                    .align_x(Align::Center)
                    .align_y(Align::Center)
                    .fill_width()
                    .build(),
            },
            Fetch::Failed(why) => match &self.failed {
                Some(failed) => failed(why),
                None => text(Label::from(std::sync::Arc::clone(why)))
                    .caption()
                    .muted()
                    .build(),
            },
        }
    }
}

impl<T> std::fmt::Debug for AsyncView<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncView")
            .field("loading", &self.fetch.loading())
            .finish_non_exhaustive()
    }
}
