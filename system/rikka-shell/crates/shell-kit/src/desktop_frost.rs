use std::time::Duration;

use rikka_ui::{Element, Layout, Length, PaintCtx, State, View, Widget};

use crate::{Opacity, PointerMode, Presence, SurfaceDesc};

#[derive(Clone, Debug, Default)]
pub struct DesktopFrost {
    opacity: Opacity,
}

impl DesktopFrost {
    pub fn surface(
        &self,
        presence: Presence,
        exit: impl Into<Duration>,
    ) -> SurfaceDesc<impl State> {
        SurfaceDesc::per_output(|_| Frosted, frosted)
            .desktop()
            .scrim()
            .pointer(PointerMode::Ignore)
            .opacity(self.opacity.clone())
            .animated(presence, exit)
    }

    #[must_use]
    pub fn opacity(&self) -> Opacity {
        self.opacity.clone()
    }

    pub fn set(&self, output: &str, level: f32) {
        self.opacity.set(output, level);
    }
}

#[derive(Debug, State)]
struct Frosted;

fn frosted(_: &Frosted) -> impl View {
    Element::new(Frost)
}

#[derive(Debug)]
struct Frost;

impl Widget for Frost {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        ctx.glass(ctx.bounds());
    }

    fn debug_name(&self) -> &'static str {
        "DesktopFrost"
    }
}
