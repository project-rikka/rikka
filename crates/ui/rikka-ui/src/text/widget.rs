use rikka_types::{Gradient, Point};

use crate::motion;
use crate::paint::{Ink, MeasureSpec, PaintCtx};
use crate::style::{Layout, Style};
use crate::text::{Label, TextRun, TextShadow, TextStyle, TextStyled};
use crate::view::{Env, Widget};

#[derive(Widget)]
#[widget(crate = "crate")]
pub struct Text {
    #[debug]
    content: Label,
    text_style: TextStyle,
    gradient: Option<Box<Gradient>>,
    shadow: Option<TextShadow>,
    #[style]
    style: Style,
}

pub fn text(content: impl Into<Label>) -> Text {
    Text {
        content: content.into(),
        text_style: TextStyle::default(),
        gradient: None,
        shadow: None,
        style: Style::default(),
    }
}

impl Text {
    #[must_use]
    pub fn gradient(mut self, gradient: Gradient) -> Self {
        self.gradient = Some(Box::new(gradient));
        self
    }

    #[must_use]
    pub fn shadow(mut self, shadow: TextShadow) -> Self {
        self.shadow = Some(shadow);
        self
    }

    fn run(&self, env: &Env) -> TextRun {
        self.text_style
            .run_in(self.content.clone(), env)
            .ellipsized()
    }

    fn draw(&self, ctx: &mut PaintCtx<'_>, ink: Ink, offset: Point) {
        let bounds = ctx.bounds().translate(offset);
        if !ctx.draw_measured_text(bounds, ink) {
            let run = self.run(ctx.env());
            ctx.draw_text(bounds, run, ink);
        }
    }
}

impl Widget for Text {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn measure(&self) -> Option<MeasureSpec> {
        let env = &crate::ambient::env();
        Some(MeasureSpec::text(self.run(env)))
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        if let Some(shadow) = self.shadow {
            self.draw(ctx, Ink::Solid(shadow.color), shadow.offset);
        }
        let ink = match self.gradient.as_deref().copied() {
            Some(gradient) => Ink::Gradient(gradient),
            None => {
                let target = self.text_style.fill(ctx.theme(), ctx.env());
                Ink::Solid(ctx.animate_curve("color", target, motion::COLOR))
            }
        };
        self.draw(ctx, ink, Point::ZERO);
    }

    fn same(&self, other: &Self) -> bool {
        self.content == other.content
            && self.text_style == other.text_style
            && self.gradient == other.gradient
            && self.shadow == other.shadow
            && self.style == other.style
    }
}

impl TextStyled for Text {
    fn text_style_mut(&mut self) -> &mut TextStyle {
        &mut self.text_style
    }
}
