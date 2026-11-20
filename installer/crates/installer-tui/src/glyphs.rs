use ratatui::widgets::BorderType;

#[derive(Clone, Copy, Debug)]
pub struct Glyphs {
    pub done: &'static str,
    pub current: &'static str,
    pub pending: &'static str,
    pub failed: &'static str,
    pub selected: &'static str,
    pub unselected: &'static str,
    pub checked: &'static str,
    pub unchecked: &'static str,
    pub dropdown: &'static str,
    pub warning: &'static str,
    pub mask: &'static str,
    pub ellipsis: &'static str,
    pub separator: &'static str,
    pub bar_full: &'static str,
    pub bar_empty: &'static str,
    pub spinner: &'static [&'static str],
    pub border: BorderType,
}

const UNICODE: Glyphs = Glyphs {
    done: "✔",
    current: "▶",
    pending: "·",
    failed: "✘",
    selected: "▸",
    unselected: " ",
    checked: "◉",
    unchecked: "○",
    dropdown: "▾",
    warning: "⚠",
    mask: "•",
    ellipsis: "…",
    separator: " · ",
    bar_full: "█",
    bar_empty: "░",
    spinner: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
    border: BorderType::Rounded,
};

const ASCII: Glyphs = Glyphs {
    done: "+",
    current: ">",
    pending: "-",
    failed: "!",
    selected: ">",
    unselected: " ",
    checked: "[x]",
    unchecked: "[ ]",
    dropdown: "v",
    warning: "!",
    mask: "*",
    ellipsis: "...",
    separator: " | ",
    bar_full: "#",
    bar_empty: ".",
    spinner: &["|", "/", "-", "\\"],
    border: BorderType::Plain,
};

impl Glyphs {
    #[must_use]
    pub fn for_terminal(term: &str, lang: &str) -> Self {
        let utf8 = lang.to_ascii_uppercase().contains("UTF-8")
            || lang.to_ascii_uppercase().contains("UTF8");
        if term == "linux" || !utf8 {
            ASCII
        } else {
            UNICODE
        }
    }

    #[must_use]
    pub fn ascii() -> Self {
        ASCII
    }

    #[must_use]
    pub fn toggle(self, on: bool) -> &'static str {
        if on { self.checked } else { self.unchecked }
    }
}
