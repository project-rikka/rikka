use rikka_types::RoundedRect;
use rikka_ui::{Element, Layout, Length, PaintCtx, Quad, View, Widget};
use shell_wallpaper::{FIT, Picture};

const BLUR: f32 = 32.0;

#[derive(Debug)]
pub(crate) struct Backdrop {
    pub(crate) picture: Option<Picture>,
}

impl Widget for Backdrop {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        if let Some(picture) = &self.picture {
            let content = picture.place(bounds, FIT);
            let source = picture.source();
            ctx.clipped(RoundedRect::rect(bounds), |ctx| {
                ctx.draw_image(content, source);
            });
        }
        ctx.draw_quad(Quad::new(bounds).blur(BLUR));
    }

    fn debug_name(&self) -> &'static str {
        "LockBackdrop"
    }
}

impl View for Backdrop {
    fn build(self) -> Element {
        Element::new(self)
    }
}
