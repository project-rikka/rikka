use rikka_types::{CornerRadii, RoundedRect, Size};
use rikka_ui::{Behavior, Layout, Length, PaintCtx, Style, Widget};

use crate::handle::Viewport3d;

#[must_use]
#[derive(Widget)]
pub struct ViewportWidget {
    #[debug]
    handle: Viewport3d,
    #[debug]
    radii: CornerRadii,
    #[style]
    style: Style,
}

pub fn viewport(handle: &Viewport3d) -> ViewportWidget {
    ViewportWidget {
        handle: handle.clone(),
        radii: CornerRadii::ZERO,
        style: Style::default(),
    }
}

impl ViewportWidget {
    pub fn radius(mut self, radii: impl Into<CornerRadii>) -> Self {
        self.radii = radii.into();
        self
    }
}

impl Widget for ViewportWidget {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::FILL)
            .with(&self.style)
    }

    fn behavior(&self, _state: &()) -> Behavior {
        Behavior::TRACKS_PLACE
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        if bounds.is_empty() {
            return;
        }
        let scale = ctx.scale();
        let width = (bounds.width() * scale).round().max(1.0) as u32;
        let height = (bounds.height() * scale).round().max(1.0) as u32;
        let Some(texture) = self.handle.drawn_at(bounds, width, height) else {
            return;
        };

        let source = (texture, Size::new(width as f32, height as f32));
        if self.radii == CornerRadii::ZERO {
            ctx.draw_image(bounds, source);
            return;
        }
        let clip = RoundedRect::new(bounds, self.radii.clamp_for_size(bounds.size));
        ctx.clipped(clip, |ctx| ctx.draw_image(bounds, source));
    }

    fn debug_name(&self) -> &'static str {
        "Viewport3d"
    }
}
