use rikka_app::prelude::*;
use rikka_omnibox::{Intent, Surface, classify, copy_text};

use crate::omnibox::running::Opening;
use crate::omnibox::state::Chip;
use crate::shell::Shell;

const SURFACE: Surface = Surface::AddressField;

impl Shell {
    pub(crate) fn address_edited(&mut self) {
        skip_view();
        self.enter_keyword_by_space();
        let typed = self.address().field().text();
        let omnibox = &mut self.address_mut().omnibox;
        omnibox.typed = typed;
        omnibox.picker.arrowed = false;
        self.pick_address();
    }

    pub(crate) fn pick_address(&mut self) {
        let inline = self.pick(SURFACE);
        let address = self.address();
        let typed = address.omnibox.typed.clone();
        let field = address.field();
        let text = field.text();
        let selection = field.selection();
        let shows_inline = text.len() > typed.len()
            && text.starts_with(&typed)
            && selection == (typed.len()..text.len());
        let untouched = text == typed && field.caret() == typed.len() && selection.is_empty();
        if !shows_inline && !untouched {
            return;
        }
        let inline = inline.unwrap_or_default();
        let next = format!("{typed}{inline}");
        if next != text {
            field.set_text(&next);
        }
        match inline.is_empty() {
            true => {
                field.place_caret(typed.len(), false);
            }
            false => field.select(typed.len(), next.len()),
        }
    }

    pub(crate) fn address_key(&mut self, key: Key, modifiers: Modifiers) -> bool {
        let command = modifiers.control && !modifiers.alt && !modifiers.logo;
        {
            let omnibox = &mut self.address_mut().omnibox;
            omnibox.picker.deleting = matches!(key, Key::Backspace | Key::Delete);
            omnibox.spaced = key == Key::Space && !command;
        }
        match key.to_lowercase() {
            Key::Enter => {
                self.submit_with(modifiers);
                true
            }
            Key::Escape => {
                self.escape_address();
                true
            }
            Key::ArrowDown if !command => self.step_rows(1),
            Key::ArrowUp if !command => self.step_rows(-1),
            Key::Tab if !command => {
                (!modifiers.shift && self.enter_keyword_by_tab())
                    || self.step_rows(if modifiers.shift { -1 } else { 1 })
            }
            Key::Delete if modifiers.shift => self.remove_selected_row(),
            Key::Backspace => self.leave_keyword(),
            _ => false,
        }
    }

    pub(crate) fn submit_address(&mut self) {
        self.submit_with(Modifiers::NONE);
    }

    fn submit_with(&mut self, modifiers: Modifiers) {
        let opening = Opening::from_modifiers(modifiers);
        if modifiers.control
            && let Some(address) = www_com(&self.address().omnibox.typed)
        {
            self.finish_picking(SURFACE);
            self.open_address(address, opening);
            return;
        }
        let listed = self
            .picker(SURFACE)
            .listing()
            .filter(|listing| !listing.rows.is_empty())
            .map(|listing| listing.selected.unwrap_or(0));
        if let Some(index) = listed {
            self.choose(SURFACE, index, opening);
            return;
        }
        let engines = self.browser().search_engines();
        let classified = classify(&self.input_of(SURFACE), &engines);
        let address = match classified.intent() {
            Intent::Empty => return,
            Intent::Navigate(destination) => destination.url().to_string(),
            Intent::Search(search) => search.url().to_string(),
        };
        self.finish_picking(SURFACE);
        self.open_address(address, opening);
    }

    fn escape_address(&mut self) {
        let address = self.address();
        let omnibox = &address.omnibox;
        let changed = address.field().text() != omnibox.original
            || omnibox.chip().is_some()
            || omnibox.picker.listing().is_some();
        if !changed {
            self.leave_address();
            return;
        }
        self.forget_picking(SURFACE);
        let address = self.address_mut();
        let original = address.omnibox.original.clone();
        address.field().reset(&original);
        address.field().select_all();
        address.omnibox.typed = original;
        address.omnibox.set_chip(None);
    }

    fn step_rows(&mut self, by: isize) -> bool {
        let picker = self.picker_mut(SURFACE);
        let Some(listing) = picker.listing_mut() else {
            return false;
        };
        let Some(at) = listing.step(by) else {
            return false;
        };
        let fill = match at {
            0 => None,
            _ => Some(listing.rows[at].fill.clone()),
        };
        picker.arrowed = true;
        let address = self.address_mut();
        let shown = fill.unwrap_or_else(|| address.omnibox.typed.clone());
        address.field().set_text(&shown);
        address.field().place_caret(shown.len(), false);
        skip_view();
        true
    }

    fn remove_selected_row(&mut self) -> bool {
        let selected = self.picker(SURFACE).listing().and_then(|listing| {
            let at = listing.selected?;
            listing.rows.get(at)?.removable.then_some(at)
        });
        let Some(at) = selected else {
            return false;
        };
        self.remove_suggestion(SURFACE, at);
        true
    }

    fn enter_keyword_by_space(&mut self) {
        let address = self.address();
        if !address.omnibox.spaced || address.omnibox.chip().is_some() {
            return;
        }
        let text = address.field().text();
        let Some((keyword, rest)) = text.split_once(' ') else {
            return;
        };
        if address.field().caret() != keyword.len() + 1 {
            return;
        }
        let rest = rest.to_owned();
        self.enter_keyword(keyword, &rest);
    }

    fn enter_keyword_by_tab(&mut self) -> bool {
        let text = self.address().field().text();
        let keyword = text.trim();
        if keyword.is_empty() || keyword.contains(char::is_whitespace) {
            return false;
        }
        let keyword = keyword.to_owned();
        let entered = self.enter_keyword(&keyword, "");
        if entered {
            self.address_edited();
        }
        entered
    }

    fn enter_keyword(&mut self, keyword: &str, rest: &str) -> bool {
        let engines = self.browser().search_engines();
        let Some(engine) = engines.by_keyword(keyword) else {
            return false;
        };
        let chip = Chip {
            engine: engine.id().to_owned(),
            name: engine.name().to_owned(),
            keyword: engine.keyword().to_owned(),
        };
        let address = self.address_mut();
        address.field().set_text(rest);
        address.field().place_caret(rest.len(), false);
        address.omnibox.set_chip(Some(chip));
        true
    }

    fn leave_keyword(&mut self) -> bool {
        let address = self.address();
        let Some(chip) = address.omnibox.chip().cloned() else {
            return false;
        };
        let field = address.field();
        if field.caret() != 0 || field.has_selection() {
            return false;
        }
        let restored = format!("{} {}", chip.keyword, field.text());
        let address = self.address_mut();
        address.omnibox.set_chip(None);
        address.field().set_text(&restored);
        address.field().place_caret(chip.keyword.len() + 1, false);
        address.omnibox.typed = restored;
        self.pick_address();
        true
    }

    pub(crate) fn clip_address(&mut self, cut: bool) -> bool {
        let address = self.address();
        let field = address.field();
        let selection = field.selection();
        if selection.is_empty() {
            return false;
        }
        let shown = field.text();
        let copied = match &address.omnibox.url {
            Some(url) => copy_text(url, &shown, selection.clone()),
            None => field.selected_text(),
        };
        let copied = match copied.is_empty() {
            true => field.selected_text(),
            false => copied,
        };
        set_text_of(Selection::Clipboard, &copied);
        if cut {
            self.address_mut().omnibox.picker.deleting = true;
            self.address().field().delete_selection();
            self.address_edited();
        }
        true
    }

    pub(crate) fn summon_address_menu(&mut self) {
        if !self.address().editing() {
            self.focus_address();
        }
        send_command(Command::ContextMenu);
    }

    pub(crate) fn address_command(&mut self, command: Command) -> bool {
        self.address_mut().omnibox.picker.deleting =
            matches!(command, Command::Delete | Command::Cut);
        match command {
            Command::Copy => self.clip_address(false),
            Command::Cut => self.clip_address(true),
            _ => false,
        }
    }

    pub(crate) fn address_menu(&mut self) -> Vec<MenuItem> {
        let pasted = pasted();
        let engines = self.browser().search_engines();
        let searches = pasted
            .as_deref()
            .is_some_and(|text| matches!(classify(text, &engines).intent(), Intent::Search(_)));
        let label = match searches {
            true => tr!("browser.omnibox.paste-search"),
            false => tr!("browser.omnibox.paste-go"),
        };
        vec![
            menu_item(label)
                .icon(MaterialSymbol::KEYBOARD_RETURN)
                .enabled(pasted.is_some())
                .on_press(Shell::paste_and_go),
        ]
    }

    fn paste_and_go(&mut self) {
        let Some(pasted) = pasted() else {
            return;
        };
        let engines = self.browser().search_engines();
        let address = match classify(&pasted, &engines).intent() {
            Intent::Empty => return,
            Intent::Navigate(destination) => destination.url().to_string(),
            Intent::Search(search) => search.url().to_string(),
        };
        self.finish_picking(SURFACE);
        self.open_address(address, Opening::Here);
    }
}

fn pasted() -> Option<String> {
    text_of(Selection::Clipboard)
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|text| !text.is_empty())
}

pub(crate) fn www_com(typed: &str) -> Option<String> {
    let typed = typed.trim();
    let end = typed.find(['/', '?', '#']).unwrap_or(typed.len());
    let (host, rest) = typed.split_at(end);
    let plain = !host.is_empty()
        && host
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || letter == '-');
    if !plain {
        return None;
    }
    let rest = match rest.is_empty() {
        true => "/",
        false => rest,
    };
    Some(format!(
        "https://www.{}.com{rest}",
        host.to_ascii_lowercase()
    ))
}

impl Shell {
    pub(crate) fn search_address(&mut self) {
        if self.command_bar.is_open() {
            self.close_command_bar();
        }
        let engines = self.browser().search_engines();
        let engine = engines.default_engine();
        let chip = Chip {
            engine: engine.id().to_owned(),
            name: engine.name().to_owned(),
            keyword: engine.keyword().to_owned(),
        };
        let address = self.address_mut();
        address.begin(String::new());
        address.omnibox.set_chip(Some(chip));
    }
}
