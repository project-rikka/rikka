use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_ui::ImageSource;

use crate::checker;

const CHECKER_SQUARE: f32 = 4.0;
const RADIUS: f32 = Corner::Detail.radius();

pub(crate) struct Thumbnail {
    source: ImageSource,
    side: f32,
}

pub(crate) fn thumbnail(source: ImageSource, side: f32) -> Thumbnail {
    Thumbnail { source, side }
}

pub(crate) fn source_of(thumbnail: paint_document::Thumbnail) -> ImageSource {
    ImageSource::Rgba {
        width: thumbnail.width,
        height: thumbnail.height,
        pixels: Arc::from(thumbnail.pixels),
    }
}

impl Widget for Thumbnail {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::px(self.side), Length::px(self.side))
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let Some(natural) = self.source.natural_size().filter(|size| !size.is_empty()) else {
            return;
        };
        let area = ImageFit::Contain.place(bounds, natural);
        ctx.clipped(RoundedRect::new(area, RADIUS), |ctx| {
            checker::fill(ctx, area, area.origin, CHECKER_SQUARE);
            ctx.draw_image(area, self.source.clone());
        });
        ctx.draw_quad(
            Quad::new(area)
                .radius(RADIUS)
                .stroke(1.0, palette().outline),
        );
    }

    fn same(&self, other: &Thumbnail) -> bool {
        self.side == other.side && self.source.key() == other.source.key()
    }
}

impl View for Thumbnail {
    fn build(self) -> Element {
        Element::new(self)
    }
}

impl std::fmt::Debug for Thumbnail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Thumbnail")
            .field("side", &self.side)
            .finish_non_exhaustive()
    }
}
