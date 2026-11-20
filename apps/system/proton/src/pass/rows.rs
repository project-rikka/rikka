use proton_codes::code_at;
use proton_vaults::Strength;
use rikka_app::prelude::*;
use rikka_markdown::markdown;
use rikka_widgets::{column, row};

use crate::pane;
use crate::pass::Pass;

const ICON: f32 = 16.0;
const ACTION: f32 = 14.0;
const HIDDEN_LENGTH: usize = 12;

pub(crate) struct Row {
    key: String,
    symbol: MaterialSymbol,
    name: Label,
    value: String,
    shown: String,
    hidden: bool,
    revealed: bool,
    copyable: bool,
    tint: Color,
    below: Option<Element>,
}

impl Row {
    pub(crate) fn hidden(mut self, vault: &Pass) -> Row {
        self.hidden = true;
        self.revealed = vault.is_revealed(&self.key);
        self.shown = match self.revealed {
            true => self.value.clone(),
            false => "•".repeat(HIDDEN_LENGTH),
        };
        self
    }

    pub(crate) fn masked(mut self, masked: String) -> Row {
        self.shown = masked;
        self
    }

    pub(crate) fn plain(mut self) -> Row {
        self.copyable = false;
        self
    }

    pub(crate) fn tint(mut self, tint: Color) -> Row {
        self.tint = tint;
        self
    }

    pub(crate) fn below(mut self, below: impl View) -> Row {
        self.below = Some(below.build());
        self
    }
}

pub(crate) fn field(key: &str, symbol: MaterialSymbol, name: Label, value: &str) -> Row {
    Row {
        key: key.to_owned(),
        symbol,
        name,
        value: value.to_owned(),
        shown: value.to_owned(),
        hidden: false,
        revealed: false,
        copyable: true,
        tint: palette().accent,
        below: None,
    }
}

impl From<Row> for Element {
    fn from(row: Row) -> Element {
        let Row {
            key,
            symbol,
            name,
            value,
            shown,
            hidden,
            revealed,
            copyable,
            tint,
            below,
        } = row;
        let copied = value;
        let toggled = key;

        container(
            row! {
                icon(symbol).extent(Length::px(ICON)).color(tint);
                column! {
                    text(name).caption().muted().no_wrap();
                    text(shown).label();
                    if let Some(below) = below {
                        below;
                    }
                }
                .gap(Space::HAIR)
                .grow(1.0);
                if hidden {
                    button(
                        icon(match revealed {
                            true => MaterialSymbol::VISIBILITY_OFF,
                            false => MaterialSymbol::VISIBILITY,
                        })
                        .extent(Length::px(ACTION)),
                    )
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(move |vault: &mut Pass| vault.reveal(toggled.clone()));
                }
                if copyable {
                    button(icon(MaterialSymbol::CONTENT_COPY).extent(Length::px(ACTION)))
                        .ghost()
                        .small()
                        .icon_only()
                        .on_press(move |vault: &mut Pass| vault.copied(copied.clone()));
                }
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width(),
        )
        .padding(Inset::xy(12.0, 10.0))
        .fill_width()
        .build()
    }
}

pub(crate) fn section(title: Option<Label>, rows: Vec<Element>) -> Option<Element> {
    if rows.is_empty() {
        return None;
    }

    let mut listed: Vec<Element> = Vec::with_capacity(rows.len() * 2 - 1);
    for row in rows {
        if !listed.is_empty() {
            listed.push(rule().build());
        }
        listed.push(row);
    }

    Some(
        column! {
            if let Some(title) = title {
                container(text(title).caption().muted()).padding(Inset::xy(4.0, 0.0));
            }
            container(column_of(listed).fill_width())
                .surface()
                .radius(Corner::Panel)
                .fill_width();
        }
        .gap(Space::TIGHT)
        .fill_width()
        .build(),
    )
}

pub(crate) struct Cell {
    title: String,
    subtitle: String,
    initial: String,
    tint: Color,
    marks: Vec<Element>,
    chosen: bool,
}

pub(crate) fn cell(title: &str, subtitle: String, tint: Color) -> Cell {
    Cell {
        title: title.to_owned(),
        subtitle,
        initial: crate::wording::initial(title),
        tint,
        marks: Vec::new(),
        chosen: false,
    }
}

impl Cell {
    pub(crate) fn mark(mut self, mark: impl View) -> Cell {
        self.marks.push(mark.build());
        self
    }

    pub(crate) fn chosen(mut self, chosen: bool) -> Cell {
        self.chosen = chosen;
        self
    }
}

impl From<Cell> for Element {
    fn from(cell: Cell) -> Element {
        let Cell {
            title,
            subtitle,
            initial,
            tint,
            marks,
            chosen,
        } = cell;

        container(
            row! {
                pane::avatar(&initial, tint);
                column! {
                    row! {
                        text(title).label().no_wrap().grow(1.0);
                        row_of(marks).gap(Space::TIGHT).align_items(Align::Center);
                    }
                    .gap(Space::TIGHT)
                    .align_items(Align::Center)
                    .fill_width();
                    text(subtitle).caption().muted().no_wrap();
                }
                .gap(Space::HAIR)
                .grow(1.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width(),
        )
        .background(match chosen {
            true => palette().selection,
            false => Color::TRANSPARENT,
        })
        .padding(Inset::xy(12.0, 8.0))
        .fill_width()
        .build()
    }
}

pub(crate) fn link(url: &str) -> Element {
    let target = url.to_owned();
    let copied = url.to_owned();

    container(
        row! {
            icon(MaterialSymbol::LINK).extent(Length::px(ICON)).color(palette().accent);
            text(url.to_owned()).label().no_wrap().grow(1.0);
            button(icon(MaterialSymbol::OPEN_IN_NEW).extent(Length::px(ACTION)))
                .ghost()
                .small()
                .icon_only()
                .on_press(move || drop(rikka_open::open(&target)));
            button(icon(MaterialSymbol::CONTENT_COPY).extent(Length::px(ACTION)))
                .ghost()
                .small()
                .icon_only()
                .on_press(move |vault: &mut Pass| vault.copied(copied.clone()));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(12.0, 10.0))
    .fill_width()
    .build()
}

pub(crate) fn code(name: Label, uri: &str, now: jiff::Timestamp) -> Element {
    let current = code_at(uri, now);
    let copied = current
        .as_ref()
        .map(|code| code.now.clone())
        .unwrap_or_default();
    let display = match &current {
        Some(code) => spaced(&code.now),
        None => tr!("proton.pass.no-code").as_str().to_owned(),
    };

    container(
        row! {
            icon(MaterialSymbol::TIMER).extent(Length::px(ICON)).color(palette().accent);
            column! {
                text(name).caption().muted().no_wrap();
                text(display).heading().no_wrap();
            }
            .gap(Space::HAIR)
            .grow(1.0);
            if let Some(code) = &current {
                text(format!("{}", code.remaining)).caption().muted().no_wrap();
                gauge(code.remaining_fraction()).extent(Length::px(18.0));
            }
            if !copied.is_empty() {
                button(icon(MaterialSymbol::CONTENT_COPY).extent(Length::px(ACTION)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(move |vault: &mut Pass| vault.copied(copied.clone()));
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(12.0, 10.0))
    .fill_width()
    .build()
}

pub(crate) fn strength(strength: Strength) -> impl View {
    let (label, tint, share) = match strength {
        Strength::Vulnerable => (
            tr!("proton.pass.strength.vulnerable"),
            palette().danger,
            0.33,
        ),
        Strength::Weak => (tr!("proton.pass.strength.weak"), palette().warning, 0.66),
        Strength::Strong => (tr!("proton.pass.strength.strong"), palette().success, 1.0),
    };

    row! {
        progress(share)
            .color(tint)
            .control_size(ControlSize::Small)
            .width(72.0);
        text(label).caption().color(tint).no_wrap();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

pub(crate) fn note(id: &str, content: &str) -> Element {
    container(
        column! {
            text(tr!("proton.pass.note")).caption().muted();
            markdown(content.to_owned())
                .selectable(format!("{id}.note"))
                .fill_width();
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
    .surface()
    .radius(Corner::Panel)
    .padding(Inset::xy(12.0, 10.0))
    .fill_width()
    .build()
}

pub(crate) fn spaced(code: &str) -> String {
    let half = code.len() / 2;
    match half {
        0 => code.to_owned(),
        half => format!("{} • {}", &code[..half], &code[half..]),
    }
}

pub(crate) fn card_number(number: &str, shown: bool) -> String {
    let digits: Vec<char> = number.chars().filter(char::is_ascii_digit).collect();
    let last = digits.len().saturating_sub(4);

    digits
        .iter()
        .enumerate()
        .map(|(at, digit)| match shown || at < 4 || at >= last {
            true => *digit,
            false => '•',
        })
        .collect::<Vec<char>>()
        .chunks(4)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<String>>()
        .join(" ")
}

pub(crate) fn expiration(input: &str) -> String {
    let digits: String = input.chars().filter(char::is_ascii_digit).collect();
    match digits.len() {
        6 => format!("{} / {}", &digits[4..6], &digits[2..4]),
        _ => input.to_owned(),
    }
}
