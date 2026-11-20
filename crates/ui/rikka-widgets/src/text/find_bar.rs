use std::hash::Hash;

use rikka_symbols::MaterialSymbol;
use rikka_theme::Palette;
use rikka_types::{Key, Modifiers};
use rikka_ui::search::{Action, Query, Rule, Search, Step};
use rikka_ui::text::{Label, TextDocument, TextStyled, text};
use rikka_ui::{
    Align, Anchor, AnchorEdge, Command, Corner, Element, Handler, Inset, IntoHandler, Length,
    Space, Style, Styled, View,
};

use crate::blocks::container::container;
use crate::blocks::flex::{column_of, row_of};
use crate::blocks::spacer::spacer;
use crate::control::ControlSize;
use crate::controls::button::{Button, ButtonVariant, button};
use crate::media::icon::icon;
use crate::text::text_field::{TextField, text_field};

const GLYPH: f32 = 16.0;
const EDGE: f32 = 6.0;
const FIELD: f32 = 200.0;
const TALLY: f32 = 58.0;

#[must_use]
pub struct FindBar {
    search: Search,
    palette: Palette,
    on_action: rikka_ui::Action<(Action,)>,
    replaceable: bool,
    over: Option<Anchor>,
    style: Style,
}

pub fn find_bar(search: &Search) -> FindBar {
    let theme = rikka_ui::theme();
    FindBar {
        search: search.clone(),
        palette: theme.palette(),
        on_action: rikka_ui::Action::default(),
        replaceable: false,
        over: None,
        style: Style::default(),
    }
}

pub fn find_mark(mark: impl Hash) -> Element {
    spacer()
        .width(Length::FILL)
        .height(Length::px(0.0))
        .build()
        .keyed(mark)
}

impl FindBar {
    pub fn on_action<Marker>(mut self, on_action: impl IntoHandler<(Action,), (), Marker>) -> Self {
        self.on_action = rikka_ui::Action::new(on_action);
        self
    }

    pub fn replaceable(mut self, replaceable: bool) -> Self {
        self.replaceable = replaceable;
        self
    }

    pub fn anchored_to(mut self, mark: impl Hash) -> Self {
        self.over = Some(Anchor::new(mark, AnchorEdge::BelowEnd).offset(-EDGE, EDGE));
        self
    }

    fn action_handler(&self, action: Action) -> Option<Handler> {
        let on_action = self.on_action.handler()?.clone();
        Some(on_action.map(move |()| (action.clone(),)))
    }

    fn field(
        &self,
        document: &TextDocument,
        hint: Label,
        typed: Action,
        chording: fn(Key, Modifiers) -> Option<Action>,
    ) -> TextField {
        let mut field = text_field(document)
            .placeholder(hint)
            .control_size(ControlSize::Small)
            .min_width(Length::px(FIELD))
            .grow(1.0);
        if let Some(on_action) = self.on_action.handler() {
            let chorded =
                on_action
                    .clone()
                    .filter_map(move |(key, modifiers): (Key, Modifiers)| {
                        chording(key, modifiers).map(|action| (action,))
                    });
            field = field
                .on_change(on_action.clone().map(move |()| (typed.clone(),)))
                .on_key(chorded);
        }
        field
    }
    fn finding(&self) -> Element {
        let query = self.search.query();
        let mut line: Vec<Element> = Vec::with_capacity(9);

        if self.replaceable {
            let replacing = self.search.is_replacing();
            let symbol = match replacing {
                true => MaterialSymbol::KEYBOARD_ARROW_DOWN,
                false => MaterialSymbol::KEYBOARD_ARROW_RIGHT,
            };
            line.push(
                press(
                    symbol,
                    self.action_handler(Action::Replacing(!replacing)),
                    false,
                )
                .build(),
            );
        }
        let pattern = self.search.pattern_field();
        let field = self
            .field(
                &pattern,
                Label::translated_or("ui.find.pattern", "Find"),
                Action::Pattern,
                chord,
            )
            .error(query.is_broken())
            .claims_focus(true);
        line.push(field.build().keyed(self.search.opened_count()));

        line.push(
            text(self.said_tally(&query))
                .caption()
                .color(match query.is_broken() {
                    true => self.palette.danger,
                    false => self.palette.text_muted,
                })
                .no_wrap()
                .min_width(Length::px(TALLY))
                .build(),
        );

        for (rule, symbol) in [
            (Rule::Case, MaterialSymbol::MATCH_CASE),
            (Rule::Word, MaterialSymbol::MATCH_WORD),
            (Rule::Regex, MaterialSymbol::REGULAR_EXPRESSION),
        ] {
            let rule_enabled = self.search.options().holds(rule);
            line.push(
                press(
                    symbol,
                    self.action_handler(Action::Rule(rule)),
                    rule_enabled,
                )
                .build(),
            );
        }

        let nothing = self.search.tally().is_empty();
        for (step, symbol) in [
            (Step::Previous, MaterialSymbol::KEYBOARD_ARROW_UP),
            (Step::Next, MaterialSymbol::KEYBOARD_ARROW_DOWN),
        ] {
            line.push(
                press(symbol, self.action_handler(Action::Step(step)), false)
                    .enabled(!nothing)
                    .build(),
            );
        }
        line.push(
            press(
                MaterialSymbol::CLOSE,
                self.action_handler(Action::Close),
                false,
            )
            .build(),
        );

        row_of(line)
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .build()
    }

    fn replacing(&self) -> Element {
        let replacement = self.search.replacement_field();
        let field = self.field(
            &replacement,
            Label::translated_or("ui.find.replacement", "Replace"),
            Action::Replacement,
            replacing_chord,
        );

        let nothing = self.search.tally().is_empty();
        crate::row! {
            field;
            press(
                MaterialSymbol::FIND_REPLACE,
                self.action_handler(Action::Step(Step::Replace)),
                false,
            )
            .enabled(!nothing);
            press(
                MaterialSymbol::ALL_MATCH,
                self.action_handler(Action::Step(Step::ReplaceAll)),
                false,
            )
            .enabled(!nothing)
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .build()
    }

    fn said_tally(&self, query: &Query) -> Label {
        if query.is_broken() {
            return Label::translated_or("ui.find.broken", "Bad pattern");
        }
        if query.is_empty() {
            return Label::default();
        }
        let tally = self.search.tally();
        if tally.is_empty() {
            return Label::translated_or("ui.find.nothing", "No results");
        }
        let position = tally.current.map_or(0, |at| at + 1);
        Label::from(format!("{position}/{}", tally.found))
    }
}

fn press(symbol: MaterialSymbol, action: Option<Handler>, rule_enabled: bool) -> Button {
    let pressed = button(icon(symbol).extent(Length::px(GLYPH)))
        .icon_only()
        .control_size(ControlSize::Small)
        .variant(match rule_enabled {
            true => ButtonVariant::Primary,
            false => ButtonVariant::Ghost,
        });
    match action {
        Some(action) => pressed.on_press(action),
        None => pressed,
    }
}

fn chord(key: Key, modifiers: Modifiers) -> Option<Action> {
    match Command::for_key(key, modifiers) {
        Some(Command::FindNext) => return Some(Action::Step(Step::Next)),
        Some(Command::FindPrevious) => return Some(Action::Step(Step::Previous)),
        _ => {}
    }
    if modifiers.control || modifiers.alt || modifiers.logo {
        return None;
    }
    match key {
        Key::Enter if modifiers.shift => Some(Action::Step(Step::Previous)),
        Key::Enter => Some(Action::Step(Step::Next)),
        Key::Escape => Some(Action::Close),
        _ => None,
    }
}

fn replacing_chord(key: Key, modifiers: Modifiers) -> Option<Action> {
    if modifiers.alt || modifiers.logo {
        return None;
    }
    match key {
        Key::Enter if modifiers.control => Some(Action::Step(Step::ReplaceAll)),
        Key::Enter if modifiers.shift => None,
        Key::Enter => Some(Action::Step(Step::Replace)),
        Key::Escape if !modifiers.control => Some(Action::Close),
        _ => None,
    }
}

impl std::fmt::Debug for FindBar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FindBar")
            .field("replaceable", &self.replaceable)
            .finish_non_exhaustive()
    }
}

impl View for FindBar {
    fn build(self) -> Element {
        if !self.search.is_open() {
            return spacer()
                .width(Length::px(0.0))
                .height(Length::px(0.0))
                .build();
        }
        let mut lines = vec![self.finding()];
        if self.replaceable && self.search.is_replacing() {
            lines.push(self.replacing());
        }

        let panel = container(column_of(lines).gap(Space::TIGHT))
            .background(self.palette.surface)
            .border(1.0, self.palette.outline)
            .radius(Corner::Panel)
            .padding(Inset::all(EDGE))
            .with_layout(self.style)
            .build();
        match self.over {
            Some(anchor) => panel.anchored(anchor),
            None => panel,
        }
    }
}

impl Styled for FindBar {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
