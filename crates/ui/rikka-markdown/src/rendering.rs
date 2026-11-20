use std::path::PathBuf;

use rikka_theme::{Palette, Syntax, Theme};
use rikka_ui::search::Found;
use rikka_ui::text::selection::Region;

use crate::LinkHandler;
use crate::style::MarkdownStyle;

#[derive(Clone)]
pub(crate) struct Rendering {
    pub(crate) style: MarkdownStyle,
    pub(crate) palette: Palette,
    pub(crate) syntax: Syntax,
    pub(crate) on_link: Option<LinkHandler>,
    pub(crate) base: Option<PathBuf>,
    pub(crate) region: Option<Region>,
    pub(crate) found: Option<Found>,
    pub(crate) unveiling: bool,
}

impl Rendering {
    pub(crate) fn from_theme(theme: &Theme) -> Rendering {
        Rendering {
            style: MarkdownStyle::from_theme(theme),
            palette: theme.palette(),
            syntax: theme.syntax(),
            on_link: None,
            base: None,
            region: None,
            found: None,
            unveiling: false,
        }
    }
}
