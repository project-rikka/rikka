use rikka_app::prelude::*;
use rikka_languages::Language;
use rikka_web::{
    Choice as Option_, ChoiceMode, EditCommand, MediaCommand, MenuTarget, Playback, Spelling, World,
};
use serde_json::Value;

use crate::command::Command;
use crate::keys;
use crate::requests::Mark;
use crate::requests::{ChoiceRequest, ContextMenuRequest, TabRequests};
use crate::shell::{Shell, WindowContent, window};
use browser_model::{Placement, TabId};

const WIDTH: f32 = 260.0;
const QUOTED: usize = 24;
const SUGGESTIONS: usize = 5;
const EDITS: [EditCommand; 6] = [
    EditCommand::Undo,
    EditCommand::Redo,
    EditCommand::Cut,
    EditCommand::Copy,
    EditCommand::Paste,
    EditCommand::SelectAll,
];
const SELECTING: &str = "rikka-browser-selection";
const SELECTION: &str = "String(window.getSelection() || '')";
const WORLD: &str = "rikka-menu";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    Suggestion(String),
    NoSuggestions,
    AddToDictionary,
    Edit(EditCommand),
    LinkInTab(String),
    LinkInSplit(String),
    LinkInPeek(String),
    LinkInWindow(String),
    LinkPrivately(String),
    CopyLink(String),
    ImageInTab(String),
    CopyImageAddress(String),
    MediaInTab(String),
    CopyMediaAddress(String),
    Play,
    Pause,
    Mute(bool),
    Copy(String),
    Search(String),
    Back,
    Forward,
    Reload,
    SavePage,
    ViewSource,
    Translate(Language),
    ShowOriginal,
    Separator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Translation {
    Offer(Language),
    Shown,
}

pub(crate) struct MenuContext<'a> {
    pub(crate) target: &'a MenuTarget,
    pub(crate) link: Option<&'a str>,
    pub(crate) source: Option<&'a str>,
    pub(crate) selected: Option<&'a str>,
    pub(crate) editable: bool,
    pub(crate) spelling: Option<&'a Spelling>,
    pub(crate) private: bool,
    pub(crate) back: bool,
    pub(crate) forward: bool,
    pub(crate) playing: bool,
    pub(crate) muted: bool,
    pub(crate) translation: Option<Translation>,
}

fn separate(into: &mut Vec<Choice>) {
    if into.last().is_some_and(|last| *last != Choice::Separator) {
        into.push(Choice::Separator);
    }
}

pub(crate) fn choices(context: &MenuContext<'_>) -> Vec<Choice> {
    let mut into = Vec::new();
    if let Some(spelling) = context.spelling {
        let suggestions = spelling.suggestions.iter().take(SUGGESTIONS);
        into.extend(suggestions.cloned().map(Choice::Suggestion));
        if spelling.suggestions.is_empty() {
            into.push(Choice::NoSuggestions);
        }
        into.push(Choice::AddToDictionary);
        separate(&mut into);
    }
    if context.editable {
        into.extend(EDITS.map(Choice::Edit));
        separate(&mut into);
    }
    if let Some(link) = context.link {
        let link = link.to_owned();
        into.push(Choice::LinkInTab(link.clone()));
        into.push(Choice::LinkInSplit(link.clone()));
        into.push(Choice::LinkInPeek(link.clone()));
        into.push(Choice::LinkInWindow(link.clone()));
        if !context.private {
            into.push(Choice::LinkPrivately(link.clone()));
        }
        separate(&mut into);
        into.push(Choice::CopyLink(link));
        separate(&mut into);
    }
    let media = matches!(context.target, MenuTarget::Video | MenuTarget::Audio);
    match (context.target, context.source) {
        (MenuTarget::Image, Some(image)) => {
            into.push(Choice::ImageInTab(image.to_owned()));
            into.push(Choice::CopyImageAddress(image.to_owned()));
            separate(&mut into);
        }
        (MenuTarget::Video | MenuTarget::Audio, source) => {
            into.push(match context.playing {
                true => Choice::Pause,
                false => Choice::Play,
            });
            into.push(Choice::Mute(!context.muted));
            separate(&mut into);
            if let Some(source) = source {
                into.push(Choice::CopyMediaAddress(source.to_owned()));
                into.push(Choice::MediaInTab(source.to_owned()));
                separate(&mut into);
            }
        }
        _ => {}
    }
    let selected = context
        .selected
        .map(str::trim)
        .filter(|selected| !selected.is_empty());
    if let Some(selected) = selected {
        if !context.editable {
            into.push(Choice::Copy(selected.to_owned()));
        }
        into.push(Choice::Search(selected.to_owned()));
        separate(&mut into);
    }
    let bare = context.link.is_none()
        && !context.editable
        && !media
        && *context.target != MenuTarget::Image
        && selected.is_none();
    if bare {
        if context.back {
            into.push(Choice::Back);
        }
        if context.forward {
            into.push(Choice::Forward);
        }
        into.push(Choice::Reload);
        separate(&mut into);
        into.push(Choice::SavePage);
        into.push(Choice::ViewSource);
        match &context.translation {
            Some(Translation::Offer(target)) => into.push(Choice::Translate(*target)),
            Some(Translation::Shown) => into.push(Choice::ShowOriginal),
            None => {}
        }
    }
    if into.last() == Some(&Choice::Separator) {
        into.pop();
    }
    into
}

fn quoted(selected: &str) -> String {
    let flat: String = selected.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.char_indices().nth(QUOTED) {
        Some((end, _)) => format!("{}…", &flat[..end]),
        None => flat,
    }
}

fn edit_label(command: EditCommand) -> Label {
    match command {
        EditCommand::Undo => tr!("browser.context.undo"),
        EditCommand::Redo => tr!("browser.context.redo"),
        EditCommand::Cut => tr!("browser.context.cut"),
        EditCommand::Copy => tr!("browser.context.copy"),
        EditCommand::Paste => tr!("browser.context.paste"),
        EditCommand::PastePlainText => tr!("browser.context.paste-plain"),
        EditCommand::SelectAll => tr!("browser.context.select-all"),
        EditCommand::Delete => tr!("browser.context.delete"),
    }
}

fn label(choice: &Choice, engine: &str) -> Label {
    match choice {
        Choice::Suggestion(word) => Label::from(word.clone()),
        Choice::NoSuggestions => tr!("browser.context.no-suggestions"),
        Choice::AddToDictionary => tr!("browser.context.add-to-dictionary"),
        Choice::Edit(command) => edit_label(*command),
        Choice::LinkInTab(_) => tr!("browser.context.link-in-tab"),
        Choice::LinkInSplit(_) => tr!("browser.context.link-in-split"),
        Choice::LinkInPeek(_) => tr!("browser.context.link-in-peek"),
        Choice::LinkInWindow(_) => tr!("browser.context.link-in-window"),
        Choice::LinkPrivately(_) => tr!("browser.context.link-in-private-window"),
        Choice::CopyLink(_) => tr!("browser.context.copy-link"),
        Choice::ImageInTab(_) => tr!("browser.context.image-in-tab"),
        Choice::CopyImageAddress(_) => tr!("browser.context.copy-image-address"),
        Choice::MediaInTab(_) => tr!("browser.context.media-in-tab"),
        Choice::CopyMediaAddress(_) => tr!("browser.context.copy-media-address"),
        Choice::Play => tr!("browser.context.play"),
        Choice::Pause => tr!("browser.context.pause"),
        Choice::Mute(true) => tr!("browser.context.mute"),
        Choice::Mute(false) => tr!("browser.context.unmute"),
        Choice::Copy(_) => tr!("browser.context.copy"),
        Choice::Search(selected) => tr!(
            "browser.context.search",
            engine = engine,
            text = &quoted(selected)
        ),
        Choice::Back => tr!("browser.context.back"),
        Choice::Forward => tr!("browser.context.forward"),
        Choice::Reload => tr!("browser.context.reload"),
        Choice::SavePage => tr!("browser.context.save-page"),
        Choice::ViewSource => tr!("browser.context.view-source"),
        Choice::Translate(target) => {
            tr!("browser.context.translate", language = target.native_name())
        }
        Choice::ShowOriginal => tr!("browser.context.show-original"),
        Choice::Separator => Label::default(),
    }
}

fn symbol(choice: &Choice) -> Option<MaterialSymbol> {
    Some(match choice {
        Choice::Suggestion(_) | Choice::NoSuggestions => return None,
        Choice::AddToDictionary => MaterialSymbol::SPELLCHECK,
        Choice::Edit(EditCommand::Undo) => MaterialSymbol::UNDO,
        Choice::Edit(EditCommand::Redo) => MaterialSymbol::REDO,
        Choice::Edit(EditCommand::Cut) => MaterialSymbol::CONTENT_CUT,
        Choice::Edit(EditCommand::Copy) => MaterialSymbol::CONTENT_COPY,
        Choice::Edit(EditCommand::Paste | EditCommand::PastePlainText) => {
            MaterialSymbol::CONTENT_PASTE
        }
        Choice::Edit(EditCommand::SelectAll) => MaterialSymbol::SELECT_ALL,
        Choice::Edit(EditCommand::Delete) => MaterialSymbol::DELETE,
        Choice::LinkInTab(_) | Choice::ImageInTab(_) | Choice::MediaInTab(_) => MaterialSymbol::TAB,
        Choice::LinkInSplit(_) => MaterialSymbol::SPLITSCREEN_RIGHT,
        Choice::LinkInPeek(_) => MaterialSymbol::PICTURE_IN_PICTURE,
        Choice::LinkInWindow(_) => MaterialSymbol::NEW_WINDOW,
        Choice::LinkPrivately(_) => MaterialSymbol::VISIBILITY_OFF,
        Choice::CopyLink(_) | Choice::CopyImageAddress(_) | Choice::CopyMediaAddress(_) => {
            MaterialSymbol::LINK
        }
        Choice::Play => MaterialSymbol::PLAY_ARROW,
        Choice::Pause => MaterialSymbol::PAUSE,
        Choice::Mute(true) => MaterialSymbol::VOLUME_OFF,
        Choice::Mute(false) => MaterialSymbol::VOLUME_UP,
        Choice::Copy(_) => MaterialSymbol::CONTENT_COPY,
        Choice::Search(_) => MaterialSymbol::SEARCH,
        Choice::Back => MaterialSymbol::ARROW_BACK,
        Choice::Forward => MaterialSymbol::ARROW_FORWARD,
        Choice::Reload => MaterialSymbol::REFRESH,
        Choice::SavePage => MaterialSymbol::PICTURE_AS_PDF,
        Choice::ViewSource => MaterialSymbol::CODE,
        Choice::Translate(_) | Choice::ShowOriginal => MaterialSymbol::TRANSLATE,
        Choice::Separator => return None,
    })
}

fn shortcut(choice: &Choice) -> Option<String> {
    match choice {
        Choice::Back => Some(keys::shortcut(Command::Back)),
        Choice::Forward => Some(keys::shortcut(Command::Forward)),
        Choice::Reload => Some(keys::shortcut(Command::Reload)),
        Choice::SavePage => Some(keys::shortcut(Command::SavePage)),
        Choice::ViewSource => Some(keys::shortcut(Command::ViewSource)),
        _ => None,
    }
}

impl Shell {
    pub(crate) fn context_menu(&mut self, tab: TabId, menu: rikka_web::ContextMenu) {
        let Some(view) = self.view_of(tab) else {
            return;
        };
        spawn_keyed(
            (SELECTING, tab),
            async move {
                let selected = view.evaluate(SELECTION, &World::isolated(WORLD)).await;
                (menu, selected)
            },
            move |shell: &mut Shell,
                  (menu, selected): (rikka_web::ContextMenu, rikka_web::Result<Value>)| {
                let selected = selected
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .filter(|selected| !selected.trim().is_empty());
                shell.point(tab, menu, selected);
            },
        );
    }

    fn point(&mut self, tab: TabId, menu: rikka_web::ContextMenu, selected: Option<String>) {
        let page_services = self.browser().page_services();
        let mut requests = page_services.requests_mut();
        let opened = requests.next_opening();
        let replaced = requests.edit(tab).pointing.replace(ContextMenuRequest {
            menu,
            selected,
            opened,
        });
        drop(replaced);
    }

    fn take_context_menu(&mut self, tab: TabId) -> Option<ContextMenuRequest> {
        self.browser()
            .page_services()
            .requests_mut()
            .edit_existing(tab)?
            .pointing
            .take()
    }

    pub(crate) fn dismiss_pointing(&mut self, tab: TabId) {
        drop(self.take_context_menu(tab));
    }

    fn take_choice(&mut self, tab: TabId) -> Option<ChoiceRequest> {
        self.browser()
            .page_services()
            .requests_mut()
            .edit_existing(tab)
            .and_then(|requests| requests.choosing.take())
    }

    pub(crate) fn choose_option(&mut self, tab: TabId, id: Option<String>) {
        let Some(ChoiceRequest { prompt, .. }) = self.take_choice(tab) else {
            return;
        };
        match id {
            Some(id) => prompt.choose([id]),
            None => prompt.dismiss(),
        }
    }

    pub(crate) fn toggle_option(&mut self, tab: TabId, id: String) {
        let page_services = self.browser().page_services();
        let mut requests = page_services.requests_mut();
        let Some(choosing) = requests
            .edit_existing(tab)
            .and_then(|requests| requests.choosing.as_mut())
        else {
            return;
        };
        match choosing
            .chosen
            .iter()
            .position(|candidate| *candidate == id)
        {
            Some(at) => {
                choosing.chosen.remove(at);
            }
            None => choosing.chosen.push(id),
        }
    }

    pub(crate) fn finish_options(&mut self, tab: TabId) {
        if let Some(ChoiceRequest { prompt, chosen, .. }) = self.take_choice(tab) {
            prompt.choose(chosen);
        }
    }

    fn chose(&mut self, tab: TabId, choice: Choice) {
        if self.take_context_menu(tab).is_none() {
            return;
        }
        match choice {
            Choice::Suggestion(word) => {
                if let Some(view) = self.view_of(tab) {
                    view.replace_misspelling(word);
                }
            }
            Choice::AddToDictionary => {
                if let Some(view) = self.view_of(tab) {
                    view.add_to_dictionary();
                }
            }
            Choice::Edit(command) => self.edit_page(tab, command),
            Choice::NoSuggestions => {}
            Choice::LinkInTab(url) => self.open_link_aside(tab, url),
            Choice::ImageInTab(url) | Choice::MediaInTab(url) => self.open_aside(url),
            Choice::LinkInSplit(url) => self.open_in_split(url),
            Choice::LinkInPeek(url) => self.peek(url),
            Choice::LinkInWindow(url) => {
                let private = self.private();
                open_window(window(
                    self.browser(),
                    private,
                    WindowContent::Addresses(vec![url]),
                ));
            }
            Choice::LinkPrivately(url) => {
                open_window(window(
                    self.browser(),
                    true,
                    WindowContent::Addresses(vec![url]),
                ));
            }
            Choice::CopyLink(url)
            | Choice::CopyImageAddress(url)
            | Choice::CopyMediaAddress(url)
            | Choice::Copy(url) => rikka_ui::clipboard::set_text(&url),
            Choice::Play => self.media_command(tab, MediaCommand::Play),
            Choice::Pause => self.media_command(tab, MediaCommand::Pause),
            Choice::Mute(_) => self.toggle_mute(tab),
            Choice::Search(selected) => self.search_for(tab, &selected),
            Choice::Back => self.back(),
            Choice::Forward => self.forward(),
            Choice::Reload => self.reload(false),
            Choice::SavePage => self.save_page(),
            Choice::ViewSource => self.view_source(tab),
            Choice::Translate(_) => self.start_translation(tab),
            Choice::ShowOriginal => self.show_original(tab),
            Choice::Separator => {}
        }
    }

    fn edit_page(&mut self, tab: TabId, command: EditCommand) {
        let Some(view) = self.view_of(tab) else {
            return;
        };
        spawn(
            async move { view.edit(command).await },
            |_: &mut Shell, edited: rikka_web::Result<()>| {
                if let Err(error) = edited {
                    tracing::debug!(%error, "page edit failed");
                }
            },
        );
    }

    fn media_command(&mut self, tab: TabId, command: MediaCommand) {
        if let Some(view) = self.view_of(tab) {
            view.media(command);
        }
    }

    fn search_for(&mut self, tab: TabId, selected: &str) {
        let url = self.browser().search_url(selected);
        self.browser()
            .edit()
            .open_tab(self.id(), Some(url), Placement::After(tab), true);
        self.changed();
    }
}

pub(crate) fn context_view(
    shell: &Shell,
    tab_requests: &TabRequests,
    tab: TabId,
    mark: Mark,
) -> Option<Element> {
    let ContextMenuRequest {
        menu,
        selected,
        opened,
    } = tab_requests.pointing.as_ref()?;
    let model = shell.model();
    let tab_held = model.tab(tab);
    let (back, forward) = tab_held.map_or((false, false), |tab_requests| {
        (*tab_requests.meta.back, *tab_requests.meta.forward)
    });
    let muted = tab_held
        .and_then(|tab_requests| tab_requests.page().live())
        .is_some_and(|live| live.muted);
    let playing = tab_held
        .and_then(|tab_requests| tab_requests.page().view())
        .is_some_and(|view| view.page().media.playback == Playback::Playing);
    drop(model);
    let translation = translation(shell, tab);
    let context = MenuContext {
        target: &menu.target,
        link: menu.link_uri.as_deref(),
        source: menu.source_uri.as_deref(),
        selected: selected.as_deref(),
        editable: menu.editable,
        spelling: menu.spelling.as_ref(),
        private: shell.private(),
        back,
        forward,
        playing,
        muted,
        translation,
    };
    let engines = shell.browser().search_engines();
    let engine = engines.default_engine().name().to_owned();
    let items: Vec<MenuItem> = choices(&context)
        .into_iter()
        .map(|choice| {
            if choice == Choice::Separator {
                return menu_separator();
            }
            let mut item =
                menu_item(label(&choice, &engine)).enabled(choice != Choice::NoSuggestions);
            if let Some(symbol) = symbol(&choice) {
                item = item.icon(symbol);
            }
            if let Some(keys) = shortcut(&choice) {
                item = item.shortcut(keys);
            }
            item.on_press(move |shell: &mut Shell| shell.chose(tab, choice.clone()))
        })
        .collect();
    let at = menu.at;
    Some(
        rikka_widgets::menu()
            .items(items)
            .width(WIDTH)
            .on_dismiss(move |shell: &mut Shell| shell.dismiss_pointing(tab))
            .build()
            .keyed(("rikka-browser-context-menu", *opened))
            .anchored(Anchor::new(mark, AnchorEdge::BelowStart).offset(at.x, at.y)),
    )
}

fn translation(shell: &Shell, tab: TabId) -> Option<Translation> {
    let web = shell.model().tab(tab).is_some_and(|tab_entry| {
        tab_entry.internal.is_none() && browser_pages::web_site(&tab_entry.address()).is_some()
    });
    if !web {
        return None;
    }
    let browser = shell.browser();
    match browser.translations().tabs().shows_translation(tab) {
        true => Some(Translation::Shown),
        false => Some(Translation::Offer(browser.preferences().translate_to())),
    }
}

fn option_items(options: &[Option_], tab: TabId, chosen: &[String], many: bool) -> Vec<MenuItem> {
    let mut items = Vec::new();
    for option in options {
        if option.separator {
            items.push(menu_separator());
            continue;
        }
        let label = option.label.clone().unwrap_or_default();
        if let Some(group) = &option.items {
            items.push(menu_header(label));
            items.extend(option_items(group, tab, chosen, many));
            continue;
        }
        let id = option.id.clone();
        let item = menu_item(label)
            .selected(chosen.contains(&id))
            .enabled(!option.disabled);
        items.push(match many {
            true => item.on_press(move |shell: &mut Shell| shell.toggle_option(tab, id.clone())),
            false => {
                item.on_press(move |shell: &mut Shell| shell.choose_option(tab, Some(id.clone())))
            }
        });
    }
    items
}

pub(crate) fn choice_view(tab_requests: &TabRequests, tab: TabId, mark: Mark) -> Option<Element> {
    let ChoiceRequest {
        prompt,
        chosen,
        opened,
    } = tab_requests.choosing.as_ref()?;
    let many = prompt.mode == ChoiceMode::Multiple;
    let mut items = option_items(&prompt.choices, tab, chosen, many);
    if many {
        items.push(menu_separator());
        items.push(
            menu_item(tr!("browser.dialog.done"))
                .on_press(move |shell: &mut Shell| shell.finish_options(tab)),
        );
    }
    let dismiss = move |shell: &mut Shell| match many {
        true => shell.finish_options(tab),
        false => shell.choose_option(tab, None),
    };
    Some(
        rikka_widgets::menu()
            .items(items)
            .width(WIDTH)
            .on_dismiss(dismiss)
            .build()
            .keyed(("rikka-browser-option-menu", *opened))
            .anchored(Anchor::new(mark, AnchorEdge::Center)),
    )
}
