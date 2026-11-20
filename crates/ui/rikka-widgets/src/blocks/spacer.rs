use rikka_ui::{Layout, PaintCtx, Style, Widget};

#[derive(Widget)]
pub struct Spacer {
    #[style]
    style: Style,
}

pub fn spacer() -> Spacer {
    Spacer {
        style: Style {
            default_grow: Some(1.0),
            ..Default::default()
        },
    }
}

impl Spacer {}

impl Widget for Spacer {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn paint(&self, _state: &mut (), _ctx: &mut PaintCtx<'_>) {}

    fn same(&self, other: &Self) -> bool {
        self.style == other.style
    }
}
