use std::hash::Hash;
use std::rc::Rc;

use rikka_ui::{Element, Layout, Length, PaintCtx, Style, Styled, View, Widget, hash_of};

use crate::blocks::spacer::spacer;

type Build = Rc<dyn Fn() -> Element>;

#[derive(Widget)]
pub struct Deferred {
    digest: u64,
    #[debug]
    estimate: f32,
    build: Build,
    #[style]
    style: Style,
}

pub fn deferred<V: View>(
    digest: impl Hash,
    estimate: f32,
    build: impl Fn() -> V + 'static,
) -> Deferred {
    Deferred {
        digest: hash_of(digest),
        estimate: estimate.max(1.0),
        build: Rc::new(move || build().build()),
        style: Style::default(),
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct DeferredState {
    reached: bool,
    height: Option<f32>,
}

impl Widget for Deferred {
    type State = DeferredState;

    fn layout(&self) -> Layout {
        Layout::column()
            .width(Length::FILL)
            .min_height(Length::px(0.0))
            .with(&self.style)
    }

    fn memo(&self) -> Option<u64> {
        Some(self.digest)
    }

    fn build(&self, state: &mut DeferredState) -> Vec<Element> {
        match state.reached {
            true => vec![(self.build)()],
            false => vec![
                spacer()
                    .height(Length::px(state.height.unwrap_or(self.estimate)))
                    .width(Length::FILL)
                    .build(),
            ],
        }
    }

    fn paint(&self, state: &mut DeferredState, ctx: &mut PaintCtx<'_>) {
        if !state.reached {
            state.reached = true;
            ctx.request_build();
            return;
        }
        state.height = Some(ctx.bounds().height());
    }
}
