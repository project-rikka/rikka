use std::hash::Hash;

use rikka_ui::{Element, Layout, PaintCtx, Style, View, Widget, hash_of};

#[derive(Widget)]
pub struct Lazy {
    #[debug]
    digest: u64,
    build: Box<dyn Fn() -> Element>,
    #[style]
    style: Style,
}

pub fn lazy<V: View>(digest: impl Hash, build: impl Fn() -> V + 'static) -> Lazy {
    Lazy {
        digest: hash_of(digest),
        build: Box::new(move || build().build()),
        style: Style::default(),
    }
}

impl Widget for Lazy {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().with(&self.style)
    }

    fn memo(&self) -> Option<u64> {
        Some(self.digest)
    }

    fn build(&self, (): &mut ()) -> Vec<Element> {
        vec![(self.build)()]
    }

    fn paint(&self, (): &mut (), _ctx: &mut PaintCtx<'_>) {}
}
