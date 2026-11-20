use rikka_text::{Family, INTER, is_installed_family};
use rikka_types::{Color, Point};

use crate::error::{Error, Result};
use crate::look::Pen;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl TextAlign {
    pub const ALL: [TextAlign; 3] = [TextAlign::Left, TextAlign::Center, TextAlign::Right];
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub color: Color,
    pub offset: Point,
    pub blur: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub color: Color,
    pub padding: f32,
    pub radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lettering {
    pub text: String,
    pub font: String,
    pub size: f32,
    pub weight: u16,
    pub italic: bool,
    pub align: TextAlign,
    pub wrap_width: Option<u32>,
    pub antialias: bool,
    pub color: Color,
    pub origin: (i32, i32),
    pub letter_spacing: f32,
    pub line_spacing: f32,
    pub outlines: Vec<Pen>,
    pub shadow: Option<Shadow>,
    pub band: Option<Band>,
}

impl Lettering {
    pub const REGULAR: u16 = 400;
    pub const BOLD: u16 = 700;
    pub const SMALLEST: f32 = 1.0;
    pub const LARGEST: f32 = 2000.0;
    pub const LIGHTEST: u16 = 1;
    pub const HEAVIEST: u16 = 1000;
    pub const LINE_SPACING: f32 = 1.2;
    pub const TIGHTEST_LINES: f32 = 0.5;
    pub const LOOSEST_LINES: f32 = 5.0;
    pub const MAX_OUTLINES: usize = 2;

    #[must_use]
    pub fn line_height(&self) -> f32 {
        (self.size * self.line_spacing).round().max(1.0)
    }

    #[must_use]
    pub fn outline_reach(&self) -> f32 {
        self.outlines.iter().map(|pen| pen.width.max(0.0)).sum()
    }

    #[must_use]
    pub fn scaled(&self, factor: f32) -> Lettering {
        Lettering {
            size: (self.size * factor).clamp(Lettering::SMALLEST, Lettering::LARGEST),
            wrap_width: self
                .wrap_width
                .map(|width| ((width as f32 * factor).round() as u32).max(1)),
            letter_spacing: self.letter_spacing * factor,
            outlines: self
                .outlines
                .iter()
                .map(|pen| Pen::new(pen.color, pen.width * factor))
                .collect(),
            shadow: self.shadow.map(|shadow| Shadow {
                offset: shadow.offset * factor,
                blur: shadow.blur * factor,
                ..shadow
            }),
            band: self.band.map(|band| Band {
                padding: band.padding * factor,
                radius: band.radius * factor,
                ..band
            }),
            ..self.clone()
        }
    }

    #[must_use]
    pub fn is_decorated(&self) -> bool {
        !self.outlines.is_empty() || self.shadow.is_some() || self.band.is_some()
    }

    pub fn check(&self) -> Result<()> {
        if !(Lettering::SMALLEST..=Lettering::LARGEST).contains(&self.size) {
            return Err(Error::Unlettered(format!(
                "{} px is not a text size between {} and {} px",
                self.size,
                Lettering::SMALLEST,
                Lettering::LARGEST
            )));
        }
        if !(Lettering::LIGHTEST..=Lettering::HEAVIEST).contains(&self.weight) {
            return Err(Error::Unlettered(format!(
                "{} is not a font weight between {} and {}",
                self.weight,
                Lettering::LIGHTEST,
                Lettering::HEAVIEST
            )));
        }
        if !(Lettering::TIGHTEST_LINES..=Lettering::LOOSEST_LINES).contains(&self.line_spacing) {
            return Err(Error::Unlettered(format!(
                "{} is not a line spacing between {} and {}",
                self.line_spacing,
                Lettering::TIGHTEST_LINES,
                Lettering::LOOSEST_LINES
            )));
        }
        if !self.letter_spacing.is_finite() || self.letter_spacing < -self.size / 2.0 {
            return Err(Error::Unlettered(format!(
                "{} px letter spacing would fold {} px letters onto each other",
                self.letter_spacing, self.size
            )));
        }
        if self.outlines.len() > Lettering::MAX_OUTLINES
            || self
                .outlines
                .iter()
                .any(|pen| !pen.width.is_finite() || pen.width <= 0.0)
        {
            return Err(Error::Unlettered(format!(
                "text takes at most {} outlines, each wider than nothing",
                Lettering::MAX_OUTLINES
            )));
        }
        if self
            .shadow
            .is_some_and(|shadow| !shadow.offset.x.is_finite() || !shadow.offset.y.is_finite())
        {
            return Err(Error::Unlettered(
                "a shadow cannot be cast an endless distance away".into(),
            ));
        }
        let negative = |value: f32| !value.is_finite() || value < 0.0;
        if self.shadow.is_some_and(|shadow| negative(shadow.blur))
            || self
                .band
                .is_some_and(|band| negative(band.padding) || negative(band.radius))
        {
            return Err(Error::Unlettered(
                "shadow blur, band padding and band radius cannot be negative".into(),
            ));
        }
        if self.wrap_width == Some(0) {
            return Err(Error::Unlettered(
                "text cannot wrap inside a box no wider than nothing".into(),
            ));
        }
        let generic = !matches!(Family::parse(&self.font), Family::Named(_));
        if !generic && !is_installed_family(&self.font) {
            return Err(Error::UnknownFont {
                family: self.font.clone(),
            });
        }
        Ok(())
    }
}

impl Default for Lettering {
    fn default() -> Lettering {
        Lettering {
            text: String::new(),
            font: INTER.into(),
            size: 48.0,
            weight: Lettering::REGULAR,
            italic: false,
            align: TextAlign::Left,
            wrap_width: None,
            antialias: true,
            color: Color::BLACK,
            origin: (0, 0),
            letter_spacing: 0.0,
            line_spacing: Lettering::LINE_SPACING,
            outlines: Vec::new(),
            shadow: None,
            band: None,
        }
    }
}
