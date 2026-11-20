use std::ops::Range;

use rikka_types::Color;

use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};

use crate::palette::Palette;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl Span {
    pub(crate) fn columns(self) -> usize {
        self.end - self.start
    }
}

impl From<Range<usize>> for Span {
    fn from(range: Range<usize>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ink {
    pub(crate) color: Color,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CellStyle<'a> {
    pub(crate) cell: &'a Cell,
    pub(crate) ink: Ink,
    background: Option<Color>,
    underline: Option<Color>,
    pub(crate) selected: bool,
}

impl<'a> CellStyle<'a> {
    pub(crate) fn new(
        cell: &'a Cell,
        selected: bool,
        palette: &Palette,
        overrides: &Colors,
    ) -> Self {
        let mut foreground = palette.resolve(cell.fg, overrides);
        let mut background = match cell.bg {
            AnsiColor::Named(NamedColor::Background) => None,
            own => Some(palette.resolve(own, overrides)),
        };
        if cell.flags.contains(Flags::INVERSE) {
            let ground = background.unwrap_or_else(|| palette.resolve(cell.bg, overrides));
            background = Some(foreground);
            foreground = ground;
        }
        if cell.flags.contains(Flags::HIDDEN) {
            foreground = background.unwrap_or(Color::TRANSPARENT);
        }
        Self {
            cell,
            ink: Ink {
                color: foreground,
                bold: cell.flags.contains(Flags::BOLD),
                italic: cell.flags.contains(Flags::ITALIC),
            },
            background,
            underline: cell
                .underline_color()
                .map(|color| palette.resolve(color, overrides)),
            selected,
        }
    }

    fn spacer(&self) -> bool {
        self.cell
            .flags
            .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
    }

    fn joinable(&self) -> bool {
        !self.cell.flags.contains(Flags::WIDE_CHAR)
            && self.cell.zerowidth().is_none()
            && matches!(self.cell.c, ' '..='~')
    }

    fn underlines(&self) -> bool {
        self.cell.flags.intersects(Flags::ALL_UNDERLINES)
    }

    fn strikes(&self) -> bool {
        self.cell.flags.contains(Flags::STRIKEOUT)
    }

    fn rule(&self) -> Color {
        self.underline.unwrap_or(self.ink.color)
    }
}

#[derive(Debug, Default)]
pub(crate) struct Row<'a> {
    cells: Vec<CellStyle<'a>>,
    decorated: bool,
    selected: bool,
}

impl<'a> Row<'a> {
    pub(crate) fn clear(&mut self) {
        self.cells.clear();
        self.decorated = false;
        self.selected = false;
    }

    pub(crate) fn push(&mut self, painted: CellStyle<'a>) {
        self.decorated |= painted.underlines() || painted.strikes();
        self.selected |= painted.selected;
        self.cells.push(painted);
    }

    pub(crate) fn backgrounds(&self, mut draw: impl FnMut(Span, Color)) {
        self.spans(
            |painted| painted.background,
            |span, background| {
                if let Some(color) = background {
                    draw(span, color);
                }
            },
        );
    }

    pub(crate) fn selection(&self, mut draw: impl FnMut(Span)) {
        if !self.selected {
            return;
        }
        self.spans(
            |painted| painted.selected,
            |span, selected| {
                if selected {
                    draw(span);
                }
            },
        );
    }

    pub(crate) fn runs(&self, text: &mut String, mut draw: impl FnMut(Span, &str, Ink)) {
        let mut start = 0;
        while start < self.cells.len() {
            let head = &self.cells[start];
            if head.spacer() {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            if head.joinable() {
                while end < self.cells.len()
                    && self.cells[end].joinable()
                    && self.cells[end].ink == head.ink
                {
                    end += 1;
                }
            }
            let span = Span { start, end };
            text.clear();
            let mut blank = true;
            for painted in &self.cells[start..end] {
                blank &= painted.cell.c == ' ';
                text.push(painted.cell.c);
                for mark in painted.cell.zerowidth().unwrap_or_default() {
                    blank = false;
                    text.push(*mark);
                }
            }
            if !blank {
                draw(span, text, head.ink);
            }
            start = end;
        }
    }

    pub(crate) fn decorations(&self, mut draw: impl FnMut(Span, Color, Decoration)) {
        if !self.decorated {
            return;
        }
        self.spans(
            |painted| {
                (
                    painted.underlines().then(|| line_of(painted.cell.flags)),
                    painted.strikes(),
                    painted.rule(),
                )
            },
            |span, (underline, strikeout, color)| {
                if let Some(underline) = underline {
                    draw(span, color, underline);
                }
                if strikeout {
                    draw(span, color, Decoration::Strikeout);
                }
            },
        );
    }

    fn spans<K: PartialEq>(
        &self,
        key: impl Fn(&CellStyle<'a>) -> K,
        mut draw: impl FnMut(Span, K),
    ) {
        let mut start = 0;
        while start < self.cells.len() {
            let value = key(&self.cells[start]);
            let mut end = start + 1;
            while end < self.cells.len() && key(&self.cells[end]) == value {
                end += 1;
            }
            draw(Span { start, end }, value);
            start = end;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Decoration {
    Underline,
    Double,
    Emphatic,
    Strikeout,
}

fn line_of(flags: Flags) -> Decoration {
    if flags.contains(Flags::DOUBLE_UNDERLINE) {
        Decoration::Double
    } else if flags.intersects(Flags::UNDERCURL | Flags::DOTTED_UNDERLINE | Flags::DASHED_UNDERLINE)
    {
        Decoration::Emphatic
    } else {
        Decoration::Underline
    }
}
