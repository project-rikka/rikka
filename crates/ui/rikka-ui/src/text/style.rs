use rikka_text::{Align as TextAlign, Family, FontStyle, Weight, Wrap};
use rikka_theme::Theme;
use rikka_types::Color;

use crate::ambient;
use crate::style::Length;
use crate::text::run::TextRun;
use crate::view::Env;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextRole {
    Display,
    Title,
    Heading,
    Subheading,
    #[default]
    Body,
    Label,
    Caption,
}

impl TextRole {
    pub const fn scale(self) -> f32 {
        match self {
            Self::Display => 1.9,
            Self::Title => 1.45,
            Self::Heading => 1.15,
            Self::Subheading => 1.05,
            Self::Body => 1.0,
            Self::Label => 0.92,
            Self::Caption => 0.85,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TextColor {
    #[default]
    Inherited,
    Muted,
    Faint,
    Accent,
    Danger,
    Warning,
    Success,
    Exact(Color),
}

impl TextColor {
    pub fn resolve(self, theme: &Theme, env: &Env) -> Color {
        let palette = theme.palette();
        match self {
            Self::Inherited => env.text_color,
            Self::Muted => palette.text_muted,
            Self::Faint => theme.disabled(palette.text_muted),
            Self::Accent => palette.accent,
            Self::Danger => palette.danger,
            Self::Warning => palette.warning,
            Self::Success => palette.success,
            Self::Exact(color) => color,
        }
    }
}

impl From<Color> for TextColor {
    fn from(color: Color) -> Self {
        Self::Exact(color)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextStyle {
    pub role: TextRole,
    pub size: Option<Length>,
    pub color: TextColor,
    pub weight: Option<Weight>,
    pub slant: Option<FontStyle>,
    pub family: Option<Family>,
    pub align: Option<TextAlign>,
    pub wrap: Option<Wrap>,
    pub line_height: Option<f32>,
}

impl TextStyle {
    pub fn font_size(&self, env: &Env) -> f32 {
        let inherited = env.font_size * self.role.scale();
        match self.size {
            Some(size) => size.resolve(env.font_size).unwrap_or(inherited).max(1.0),
            None => inherited,
        }
    }

    pub fn fill(&self, theme: &Theme, env: &Env) -> Color {
        self.color.resolve(theme, env)
    }

    pub fn run(&self, content: impl Into<std::sync::Arc<str>>) -> TextRun {
        self.run_in(content, &ambient::env())
    }

    pub fn run_in(&self, content: impl Into<std::sync::Arc<str>>, env: &Env) -> TextRun {
        let mut run = TextRun::new(content, self.font_size(env));
        if let Some(line_height) = self.line_height {
            run.line_height = line_height;
        }
        if let Some(family) = &self.family {
            run.family = family.clone();
        }
        if let Some(weight) = self.weight {
            run.weight = weight;
        }
        if let Some(slant) = self.slant {
            run.style = slant;
        }
        if let Some(wrap) = self.wrap {
            run.wrap = wrap;
        }
        run.align = self.align;
        run
    }
}

pub trait TextStyled: Sized {
    fn text_style_mut(&mut self) -> &mut TextStyle;

    #[must_use]
    fn role(mut self, role: TextRole) -> Self {
        self.text_style_mut().role = role;
        self
    }

    #[must_use]
    fn display(self) -> Self {
        self.role(TextRole::Display)
    }

    #[must_use]
    fn title(self) -> Self {
        self.role(TextRole::Title)
    }

    #[must_use]
    fn heading(self) -> Self {
        self.role(TextRole::Heading)
    }

    #[must_use]
    fn subheading(self) -> Self {
        self.role(TextRole::Subheading)
    }

    #[must_use]
    fn body(self) -> Self {
        self.role(TextRole::Body)
    }

    #[must_use]
    fn label(self) -> Self {
        self.role(TextRole::Label)
    }

    #[must_use]
    fn caption(self) -> Self {
        self.role(TextRole::Caption)
    }

    #[must_use]
    fn text_size(mut self, size: impl Into<Length>) -> Self {
        self.text_style_mut().size = Some(size.into());
        self
    }

    #[must_use]
    fn color(mut self, color: impl Into<TextColor>) -> Self {
        self.text_style_mut().color = color.into();
        self
    }

    #[must_use]
    fn muted(self) -> Self {
        self.color(TextColor::Muted)
    }

    #[must_use]
    fn faint(self) -> Self {
        self.color(TextColor::Faint)
    }

    #[must_use]
    fn accent(self) -> Self {
        self.color(TextColor::Accent)
    }

    #[must_use]
    fn danger(self) -> Self {
        self.color(TextColor::Danger)
    }

    #[must_use]
    fn weight(mut self, weight: Weight) -> Self {
        self.text_style_mut().weight = Some(weight);
        self
    }

    #[must_use]
    fn light(self) -> Self {
        self.weight(crate::font::LIGHT)
    }

    #[must_use]
    fn medium(self) -> Self {
        self.weight(crate::font::MEDIUM)
    }

    #[must_use]
    fn semibold(self) -> Self {
        self.weight(crate::font::SEMIBOLD)
    }

    #[must_use]
    fn bold(self) -> Self {
        self.weight(crate::font::BOLD)
    }

    #[must_use]
    fn italic(mut self) -> Self {
        self.text_style_mut().slant = Some(FontStyle::Italic);
        self
    }

    #[must_use]
    fn family(mut self, family: Family) -> Self {
        self.text_style_mut().family = Some(family);
        self
    }

    #[must_use]
    fn align(mut self, align: TextAlign) -> Self {
        self.text_style_mut().align = Some(align);
        self
    }

    #[must_use]
    fn no_wrap(mut self) -> Self {
        self.text_style_mut().wrap = Some(Wrap::None);
        self
    }

    #[must_use]
    fn line_height(mut self, line_height: f32) -> Self {
        self.text_style_mut().line_height = Some(line_height);
        self
    }
}
