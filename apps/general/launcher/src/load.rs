use rikka_app::prelude::*;
use rikka_ui::Element;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Load<T> {
    #[default]
    Idle,
    Loading,
    Ready(T),
    Failed(String),
}

impl<T> Load<T> {
    pub(crate) fn ready(&self) -> Option<&T> {
        match self {
            Load::Ready(value) => Some(value),
            Load::Idle | Load::Loading | Load::Failed(_) => None,
        }
    }
}

pub(crate) fn loading<T, V: View>(load: &Load<T>, ready: impl FnOnce(&T) -> V) -> Element {
    match load {
        Load::Idle | Load::Loading => container(spinner().extent(Length::px(24.0)))
            .center()
            .padding(Inset::all(Space::WIDE))
            .fill_width()
            .any(),
        Load::Failed(error) => status_page(MaterialSymbol::ERROR, error.clone())
            .tint(palette().danger)
            .fill_width()
            .any(),
        Load::Ready(value) => ready(value).any(),
    }
}
