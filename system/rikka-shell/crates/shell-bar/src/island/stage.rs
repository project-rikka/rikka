use rikka_types::{Point, Rect};
use rikka_ui::{Align, Element, Inset, Layout, Length, View, Widget};
use shell_kit::taskbar::Shape;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Hang {
    Over,
    Below { left: f32 },
}

pub(crate) struct Stage {
    card: Element,
    hang: Hang,
}

pub(crate) fn stage(card: Element, hang: Hang) -> Stage {
    Stage { card, hang }
}

impl Widget for Stage {
    type State = ();

    fn layout(&self) -> Layout {
        let layout = Layout::column().width(Length::FILL).placed_at(Point::ZERO);
        match self.hang {
            Hang::Over => layout.align_items(Align::Center),
            Hang::Below { left } => layout.align_items(Align::Start).padding(Inset::exact(
                Shape.reserved(),
                0.0,
                0.0,
                left,
            )),
        }
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }

    fn debug_name(&self) -> &'static str {
        "IslandStage"
    }
}

impl std::fmt::Debug for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stage")
            .field("hang", &self.hang)
            .finish_non_exhaustive()
    }
}

impl View for Stage {
    fn build(mut self) -> Element {
        let card = std::mem::replace(&mut self.card, Element::empty());
        Element::with_children(self, vec![card])
    }
}
