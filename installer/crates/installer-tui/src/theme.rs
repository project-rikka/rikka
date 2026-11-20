use ratatui::style::Color;

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub accent: Color,
    pub text: Color,
    pub muted: Color,
    pub border: Color,
    pub ok: Color,
    pub warn: Color,
    pub danger: Color,
    pub on_accent: Color,
}

impl Theme {
    #[must_use]
    pub fn for_terminal(colorterm: &str, term: &str) -> Self {
        if rich(colorterm, term) {
            Self::rikka()
        } else {
            Self::console()
        }
    }

    #[must_use]
    pub const fn rikka() -> Self {
        Self {
            accent: Color::Rgb(0x97, 0xCC, 0xF9),
            text: Color::Rgb(0xEC, 0xEA, 0xF3),
            muted: Color::Rgb(0xAA, 0xAD, 0xBF),
            border: Color::Rgb(0x4A, 0x50, 0x60),
            ok: Color::Rgb(0xAA, 0xD2, 0x91),
            warn: Color::Rgb(0xFF, 0xD0, 0x7C),
            danger: Color::Rgb(0xFF, 0xA2, 0xA9),
            on_accent: Color::Rgb(0x1F, 0x21, 0x2A),
        }
    }

    #[must_use]
    pub const fn console() -> Self {
        Self {
            accent: Color::Cyan,
            text: Color::Gray,
            muted: Color::DarkGray,
            border: Color::DarkGray,
            ok: Color::Green,
            warn: Color::Yellow,
            danger: Color::Red,
            on_accent: Color::Black,
        }
    }
}

fn rich(colorterm: &str, term: &str) -> bool {
    if matches!(colorterm, "truecolor" | "24bit") {
        return true;
    }
    term.contains("256color") || term.contains("truecolor") || term.contains("direct")
}
