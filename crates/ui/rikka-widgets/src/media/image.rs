use rikka_types::{CornerRadii, RoundedRect, Size};
use rikka_ui::{ImageSource, Layout, MeasureSpec, PaintCtx, Style, Widget};

use crate::media::fit::ImageFit;

#[must_use]
#[derive(Widget)]
pub struct Image {
    #[debug]
    source: ImageSource,
    #[debug]
    fit: ImageFit,
    radii: CornerRadii,
    natural_size: Option<Size>,
    #[style]
    style: Style,
}

pub fn image(source: impl Into<ImageSource>) -> Image {
    Image {
        source: source.into(),
        fit: ImageFit::default(),
        radii: CornerRadii::ZERO,
        natural_size: None,
        style: Style::default(),
    }
}

impl Image {
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    pub fn radius(mut self, radii: impl Into<CornerRadii>) -> Self {
        self.radii = radii.into();
        self
    }

    pub fn natural_size(mut self, width: f32, height: f32) -> Self {
        self.natural_size = Some(Size::new(width, height));
        self
    }

    fn natural(&self) -> Size {
        self.natural_size
            .or_else(|| self.source.natural_size())
            .unwrap_or(Size::ZERO)
    }
}

impl Widget for Image {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn measure(&self) -> Option<MeasureSpec> {
        let natural = self.natural();
        (!natural.is_empty()).then_some(MeasureSpec::Fixed(natural))
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        draw_fitted(
            ctx,
            self.fit,
            self.radii,
            self.natural(),
            self.source.clone(),
        );
    }

    fn same(&self, other: &Self) -> bool {
        self.source.key() == other.source.key()
            && self.natural() == other.natural()
            && self.fit == other.fit
            && self.radii == other.radii
            && self.style == other.style
    }
}

fn draw_fitted(
    ctx: &mut PaintCtx<'_>,
    fit: ImageFit,
    radii: CornerRadii,
    natural: Size,
    source: ImageSource,
) {
    let bounds = ctx.bounds();
    if bounds.is_empty() {
        return;
    }

    let content = fit.place(bounds, natural);
    if content == bounds && radii == CornerRadii::ZERO {
        ctx.draw_image(bounds, source);
        return;
    }

    let clip = RoundedRect::new(bounds, radii.clamp_for_size(bounds.size));
    ctx.clipped(clip, |ctx| ctx.draw_image(content, source));
}
