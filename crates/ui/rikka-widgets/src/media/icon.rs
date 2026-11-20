use rikka_symbols::IconGlyph;
use rikka_text::IconSpec;
use rikka_text::Weight;
use rikka_types::{Color, Size};
use rikka_ui::{IconSource, Layout, Length, MeasureSpec, PaintCtx, Style, Widget, motion};

const DEFAULT_SIZE: f32 = 24.0;

const FILL_STEPS: f32 = 16.0;

fn quantize(fill: f32) -> f32 {
    (fill.clamp(0.0, 1.0) * FILL_STEPS).round() / FILL_STEPS
}

#[must_use]
#[derive(Widget)]
pub struct Icon {
    #[debug]
    source: IconSource,
    #[debug]
    size: f32,
    color: Option<Color>,
    fill: f32,
    weight: Weight,
    #[style]
    style: Style,
}

pub fn icon(source: impl Into<IconSource>) -> Icon {
    Icon {
        source: source.into(),
        size: DEFAULT_SIZE,
        color: None,
        fill: 0.0,
        weight: Weight::NORMAL,
        style: Style::default(),
    }
}

impl Icon {
    pub fn extent(mut self, extent: impl Into<Length>) -> Self {
        self.size = extent.into().resolve(0.0).unwrap_or(self.size).max(1.0);
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    fn fill_amount(mut self, fill: f32) -> Self {
        self.fill = fill;
        self
    }

    pub fn filled(self) -> Self {
        self.fill_amount(1.0)
    }

    pub fn weight(mut self, weight: Weight) -> Self {
        self.weight = weight;
        self
    }

    fn spec(&self, glyph: IconGlyph, fill: f32) -> IconSpec {
        glyph.spec(self.size).fill(fill).weight(self.weight)
    }

    fn tint(&self, ctx: &mut PaintCtx<'_>) -> Color {
        let target = self.color.unwrap_or(ctx.env().text_color);
        ctx.animate_curve("color", target, motion::COLOR)
    }
}

impl Widget for Icon {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn measure(&self) -> Option<MeasureSpec> {
        Some(MeasureSpec::Fixed(Size::new(self.size, self.size)))
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        match &self.source {
            IconSource::Picture(picture) => ctx.draw_image(ctx.bounds(), picture.clone()),
            IconSource::Mask(picture) => {
                let color = self.tint(ctx);
                ctx.draw_mask(ctx.bounds(), picture.clone(), color);
            }
            IconSource::Glyph(glyph) => {
                let color = self.tint(ctx);
                let fill = ctx.animate_curve("fill", self.fill, motion::TOGGLE);
                ctx.draw_icon(ctx.bounds(), self.spec(*glyph, quantize(fill)), color);
            }
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.source == other.source
            && self.size == other.size
            && self.color == other.color
            && self.fill == other.fill
            && self.weight == other.weight
            && self.style == other.style
    }
}
