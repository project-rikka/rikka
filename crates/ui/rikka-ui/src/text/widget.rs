use crate::motion;
use crate::paint::{MeasureSpec, PaintCtx};
use crate::style::{Layout, Style};
use crate::text::{Label, TextRun, TextStyle, TextStyled};
use crate::view::{Env, Widget};

#[derive(Widget)]
#[widget(crate = "crate")]
pub struct Text {
    #[debug]
    content: Label,
    text_style: TextStyle,
    #[style]
    style: Style,
}

pub fn text(content: impl Into<Label>) -> Text {
    Text {
        content: content.into(),
        text_style: TextStyle::default(),
        style: Style::default(),
    }
}

impl Text {
    fn run(&self, env: &Env) -> TextRun {
        self.text_style
            .run_in(self.content.clone(), env)
            .ellipsized()
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
        let target = self.text_style.fill(ctx.theme(), ctx.env());
        let color = ctx.animate_curve("color", target, motion::COLOR);
        let bounds = ctx.bounds();
        if !ctx.draw_measured_text(bounds, color) {
            let run = self.run(ctx.env());
            ctx.draw_text(bounds, run, color);
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.content == other.content
            && self.text_style == other.text_style
            && self.style == other.style
    }
}

impl TextStyled for Text {
    fn text_style_mut(&mut self) -> &mut TextStyle {
        &mut self.text_style
    }
}
