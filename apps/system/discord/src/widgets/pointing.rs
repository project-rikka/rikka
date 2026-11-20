use rikka_app::prelude::*;
use rikka_ui::{Handler, IntoHandler, Layout};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Pointing {
    #[default]
    Away,
    Passive,
    Hovered,
}

impl Pointing {
    pub(crate) fn is_over(self) -> bool {
        self != Pointing::Away
    }
}

pub(crate) fn pointing<M>(
    body: impl View,
    changed: impl IntoHandler<(Pointing,), (), M>,
) -> Element {
    Element::with_children(
        PointingArea {
            changed: Handler::new(changed),
        },
        vec![body.build()],
    )
}

struct PointingArea {
    changed: Handler<(Pointing,)>,
}

impl Widget for PointingArea {
    type State = Pointing;

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::HUG)
    }

    fn event(&self, pointing: &mut Pointing, event: &Event, ctx: &mut EventCtx<'_>) {
        let next = match event {
            Event::PointerEntered => Pointing::Passive,
            Event::PointerMoved(_) => Pointing::Hovered,
            Event::Scroll { .. } if *pointing == Pointing::Hovered => Pointing::Passive,
            Event::PointerExited => Pointing::Away,
            _ => return,
        };
        if *pointing != next {
            *pointing = next;
            ctx.call(&self.changed, (next,));
        }
    }
    fn same(&self, _: &Self) -> bool {
        true
    }

    fn debug_name(&self) -> &'static str {
        "PointingArea"
    }
}
