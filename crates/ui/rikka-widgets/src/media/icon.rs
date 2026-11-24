use rikka_symbols::IconGlyph;
use rikka_text::IconSpec;
use rikka_text::Weight;
use rikka_types::{Color, Gradient, Point, Size};
use rikka_ui::{
    IconSource, Ink, Layout, Length, MeasureSpec, PaintCtx, Style, TextShadow, Widget, motion,
};

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
    gradient: Option<Box<Gradient>>,
    shadow: Option<TextShadow>,
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
        gradient: None,
        shadow: None,
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

    pub fn gradient(mut self, gradient: Gradient) -> Self {
        self.gradient = Some(Box::new(gradient));
        self
    }

    pub fn shadow(mut self, shadow: TextShadow) -> Self {
        self.shadow = Some(shadow);
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

    fn ink(&self, ctx: &mut PaintCtx<'_>) -> Ink {
        match self.gradient.as_deref().copied() {
            Some(gradient) => Ink::Gradient(gradient),
            None => Ink::Solid(self.tint(ctx)),
        }
    }

    fn draw(&self, ctx: &mut PaintCtx<'_>, offset: Point, ink: Ink, fill: f32) {
        let bounds = ctx.bounds().translate(offset);
        match &self.source {
            IconSource::Picture(picture) => ctx.draw_image(bounds, picture.clone()),
            IconSource::Mask(picture) => ctx.draw_mask(bounds, picture.clone(), ink),
            IconSource::Glyph(glyph) => ctx.draw_icon(bounds, self.spec(*glyph, fill), ink),
        }
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
        let fill = match self.source {
            IconSource::Glyph(_) => quantize(ctx.animate_curve("fill", self.fill, motion::TOGGLE)),
            _ => self.fill,
        };
        if let Some(shadow) = self.shadow
            && !matches!(self.source, IconSource::Picture(_))
        {
            self.draw(ctx, shadow.offset, Ink::Solid(shadow.color), fill);
        }
        let ink = self.ink(ctx);
        self.draw(ctx, Point::ZERO, ink, fill);
    }

    fn same(&self, other: &Self) -> bool {
        self.source == other.source
            && self.size == other.size
            && self.color == other.color
            && self.gradient == other.gradient
            && self.shadow == other.shadow
            && self.fill == other.fill
            && self.weight == other.weight
            && self.style == other.style
    }
}
