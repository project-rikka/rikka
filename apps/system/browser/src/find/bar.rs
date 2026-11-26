use browser_preferences::{FindOption, FindOptions};
use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::find::Find;
use crate::shell::Shell;
use browser_model::TabId;

const FIELD: f32 = 220.0;
const TALLY: f32 = 64.0;
const GLYPH: f32 = 16.0;

fn tool(symbol: MaterialSymbol, active: bool) -> Button {
    let pressed = button(icon(symbol).extent(Length::px(GLYPH)))
        .icon_only()
        .small()
        .focusless();
    match active {
        true => pressed.primary(),
        false => pressed.ghost(),
    }
}

fn find_summary(find: &Find) -> (String, bool) {
    if find.field().text().is_empty() {
        return (String::new(), false);
    }
    match find.tally() {
        None => (String::new(), false),
        Some((_, 0)) => (tr!("browser.find.nothing").to_string(), true),
        Some((current, matches)) => (format!("{current}/{matches}"), false),
    }
}

fn option_symbol(option: FindOption) -> MaterialSymbol {
    match option {
        FindOption::MatchCase => MaterialSymbol::MATCH_CASE,
        FindOption::WholeWord => MaterialSymbol::MATCH_WORD,
        FindOption::MatchDiacritics => MaterialSymbol::SPECIAL_CHARACTER,
        FindOption::LinksOnly => MaterialSymbol::LINK,
    }
}

fn option_name(option: FindOption) -> Label {
    match option {
        FindOption::MatchCase => tr!("browser.find.match-case"),
        FindOption::WholeWord => tr!("browser.find.whole-word"),
        FindOption::MatchDiacritics => tr!("browser.find.match-diacritics"),
        FindOption::LinksOnly => tr!("browser.find.links-only"),
    }
}

pub(crate) fn bar(find: &Find, tab: TabId, options: FindOptions) -> Element {
    let toggles = FindOption::ALL.map(|option| {
        tool(option_symbol(option), options.get(option))
            .on_press(move |shell: &mut Shell| shell.toggle_find_option(tab, option))
            .tooltip(option_name(option))
            .any()
    });
    let (tally, missed) = find_summary(find);
    let nothing = !matches!(find.tally(), Some((_, matches)) if matches > 0);
    let field = text_field(find.field())
        .placeholder(tr!("browser.find.placeholder"))
        .control_size(ControlSize::Small)
        .error(missed)
        .claims_focus(true)
        .on_change(move |shell: &mut Shell| shell.find_typed(tab))
        .on_submit(move |shell: &mut Shell| shell.find_step(tab, true))
        .on_key(move |shell: &mut Shell, key: Key, modifiers: Modifiers| {
            match (key, modifiers.shift) {
                (Key::Escape, _) => shell.close_find(tab),
                (Key::Enter | Key::F(3), true) => shell.find_step(tab, false),
                (Key::F(3), false) => shell.find_step(tab, true),
                _ => return false,
            }
            true
        })
        .width(FIELD)
        .build()
        .keyed(("rikka-browser-find-field", find.open_count()));
    container(
        row! {
            field;
            text(tally)
                .caption()
                .color(match missed {
                    true => palette().danger,
                    false => palette().text_muted,
                })
                .no_wrap()
                .min_width(Length::px(TALLY));
            row_of(toggles).gap(Space::HAIR);
            tool(MaterialSymbol::KEYBOARD_ARROW_UP, false)
                .enabled(!nothing)
                .on_press(move |shell: &mut Shell| shell.find_step(tab, false));
            tool(MaterialSymbol::KEYBOARD_ARROW_DOWN, false)
                .enabled(!nothing)
                .on_press(move |shell: &mut Shell| shell.find_step(tab, true));
            tool(MaterialSymbol::CLOSE, false).on_press(move |shell: &mut Shell| shell.close_find(tab));
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .padding(Inset::all(Space::TIGHT))
    .background(palette().surface)
    .border(1.0, palette().outline.with_alpha(0.6))
    .radius(Corner::Panel)
    .any()
}
