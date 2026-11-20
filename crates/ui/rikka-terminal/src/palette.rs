use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor, Rgb};
use rikka_theme::{Ansi, Theme};
use rikka_types::Color;

const DIM: f32 = 0.4;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Palette {
    indexed: Box<[Color; 256]>,
    foreground: Color,
    background: Color,
    cursor: Color,
    selection: Color,
}

impl Palette {
    pub(crate) fn from_theme(theme: &Theme) -> Self {
        Self::new(theme.ansi(), theme.palette().selection)
    }

    fn new(ansi: Ansi, selection: Color) -> Self {
        let mut indexed = Box::new([Color::BLACK; 256]);
        for (slot, color) in indexed.iter_mut().zip(ansi.colors) {
            *slot = color;
        }
        for (index, slot) in indexed.iter_mut().enumerate().skip(16) {
            *slot = xterm(index as u8);
        }
        Self {
            indexed,
            foreground: ansi.foreground,
            background: ansi.background,
            cursor: ansi.cursor,
            selection,
        }
    }

    pub(crate) fn background(&self) -> Color {
        self.background
    }

    pub(crate) fn ground_override(overrides: &Colors) -> Option<Color> {
        overridden(NamedColor::Background, overrides)
    }

    pub(crate) fn cursor(&self) -> Color {
        self.cursor
    }

    pub(crate) fn selection(&self) -> Color {
        self.selection
    }

    pub(crate) fn resolve(&self, color: AnsiColor, overrides: &Colors) -> Color {
        match color {
            AnsiColor::Spec(rgb) => rgb_to_color(rgb),
            AnsiColor::Indexed(index) => self.at(index as usize, overrides),
            AnsiColor::Named(named) => self.named(named, overrides),
        }
    }

    fn at(&self, index: usize, overrides: &Colors) -> Color {
        match overrides[index] {
            Some(rgb) => rgb_to_color(rgb),
            None => self.indexed[index.min(255)],
        }
    }

    fn named(&self, named: NamedColor, overrides: &Colors) -> Color {
        if let Some(color) = overridden(named, overrides) {
            return color;
        }
        match named {
            NamedColor::Foreground => self.foreground,
            NamedColor::Background => self.background,
            NamedColor::Cursor => self.cursor,
            NamedColor::BrightForeground => self.foreground,
            NamedColor::DimForeground => self.foreground.mix(self.background, DIM),
            NamedColor::DimBlack
            | NamedColor::DimRed
            | NamedColor::DimGreen
            | NamedColor::DimYellow
            | NamedColor::DimBlue
            | NamedColor::DimMagenta
            | NamedColor::DimCyan
            | NamedColor::DimWhite => {
                let ordinary = named as usize - NamedColor::DimBlack as usize;
                self.at(ordinary, overrides).mix(self.background, DIM)
            }
            other => self.at(other as usize, overrides),
        }
    }

    pub(crate) fn report(&self, index: usize, overrides: &Colors) -> Rgb {
        let color = match named_at(index) {
            Some(named) => self.named(named, overrides),
            None => self.at(index, overrides),
        };
        Rgb {
            r: channel(color.r),
            g: channel(color.g),
            b: channel(color.b),
        }
    }
}

fn named_at(index: usize) -> Option<NamedColor> {
    const NAMED: [NamedColor; 13] = [
        NamedColor::Foreground,
        NamedColor::Background,
        NamedColor::Cursor,
        NamedColor::DimBlack,
        NamedColor::DimRed,
        NamedColor::DimGreen,
        NamedColor::DimYellow,
        NamedColor::DimBlue,
        NamedColor::DimMagenta,
        NamedColor::DimCyan,
        NamedColor::DimWhite,
        NamedColor::BrightForeground,
        NamedColor::DimForeground,
    ];
    NAMED.get(index.checked_sub(256)?).copied()
}

fn xterm(index: u8) -> Color {
    const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    match index {
        16..=231 => {
            let index = index - 16;
            Color::rgb8(
                STEPS[(index / 36) as usize],
                STEPS[(index / 6 % 6) as usize],
                STEPS[(index % 6) as usize],
            )
        }
        232.. => {
            let level = 8 + (index - 232) * 10;
            Color::rgb8(level, level, level)
        }
        _ => Color::BLACK,
    }
}

fn overridden(named: NamedColor, overrides: &Colors) -> Option<Color> {
    overrides[named].map(rgb_to_color)
}

fn rgb_to_color(rgb: Rgb) -> Color {
    Color::rgb8(rgb.r, rgb.g, rgb.b)
}

fn channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}
