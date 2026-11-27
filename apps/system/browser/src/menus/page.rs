use rikka_app::prelude::*;
use rikka_web::{ContextImage, ContextLink, ContextMedia, ContextMenu, EditCommand, MediaKind};

use crate::command::Command;
use crate::keys;
use crate::menus::Entries;
use crate::shell::Shell;
use browser_model::TabId;

const QUOTED: usize = 24;

struct Page {
    tab: TabId,
    frame: i64,
}

pub(crate) fn entries(shell: &Shell, tab: TabId, menu: &ContextMenu) -> Entries {
    let mut entries = Entries::new();
    let page = Page {
        tab,
        frame: menu.frame,
    };
    if let Some(editable) = &menu.editable {
        editing(&mut entries, &page, editable.selected, &menu.selection);
    }
    if let Some(link) = &menu.link {
        linking(&mut entries, shell, &page, link);
    }
    if let Some(image) = &menu.image {
        picturing(&mut entries, &page, image);
    }
    if let Some(media) = &menu.media {
        playing(&mut entries, &page, media);
    }
    let selection = menu.selection.trim();
    if !selection.is_empty() {
        selecting(
            &mut entries,
            shell,
            &page,
            selection,
            menu.editable.is_none(),
        );
    }
    let bare = menu.link.is_none()
        && menu.image.is_none()
        && menu.media.is_none()
        && menu.editable.is_none()
        && selection.is_empty();
    if bare {
        navigating(&mut entries, shell, &page);
    }
    tools(&mut entries, &page);
    entries
}

fn editing(entries: &mut Entries, page: &Page, selected: bool, selection: &str) {
    let (tab, frame) = (page.tab, page.frame);
    let copyable = selected && !selection.is_empty();
    entries.separator();
    entries.leaf(
        menu_item(tr!("browser.context.undo"))
            .icon(MaterialSymbol::UNDO)
            .on_press(move |shell: &mut Shell| shell.edit_page(tab, frame, EditCommand::Undo)),
    );
    entries.leaf(
        menu_item(tr!("browser.context.redo"))
            .icon(MaterialSymbol::REDO)
            .on_press(move |shell: &mut Shell| shell.edit_page(tab, frame, EditCommand::Redo)),
    );
    entries.separator();
    let cut = selection.to_owned();
    entries.leaf(
        menu_item(tr!("browser.context.cut"))
            .icon(MaterialSymbol::CONTENT_CUT)
            .enabled(copyable)
            .on_press(move |shell: &mut Shell| shell.cut_page(tab, frame, &cut)),
    );
    let copied = selection.to_owned();
    entries.leaf(
        menu_item(tr!("browser.context.copy"))
            .icon(MaterialSymbol::CONTENT_COPY)
            .enabled(copyable)
            .on_press(move |_: &mut Shell| rikka_ui::clipboard::set_text(&copied)),
    );
    entries.leaf(
        menu_item(tr!("browser.context.paste"))
            .icon(MaterialSymbol::CONTENT_PASTE)
            .enabled(rikka_ui::clipboard::has_text())
            .on_press(move |shell: &mut Shell| shell.paste_page(tab, frame)),
    );
    entries.leaf(
        menu_item(tr!("browser.context.delete"))
            .icon(MaterialSymbol::DELETE)
            .enabled(selected)
            .on_press(move |shell: &mut Shell| shell.edit_page(tab, frame, EditCommand::Delete)),
    );
    entries.separator();
    entries.leaf(
        menu_item(tr!("browser.context.select-all"))
            .icon(MaterialSymbol::SELECT_ALL)
            .on_press(move |shell: &mut Shell| shell.edit_page(tab, frame, EditCommand::SelectAll)),
    );
}

fn linking(entries: &mut Entries, shell: &Shell, page: &Page, link: &ContextLink) {
    let tab = page.tab;
    let url = link.url.clone();
    entries.separator();
    entries.leaf(opening(
        tr!("browser.context.link-in-tab"),
        MaterialSymbol::TAB,
        &url,
        move |shell: &mut Shell, url: String| shell.open_link(tab, url, true),
    ));
    if !shell.little() {
        entries.leaf(opening(
            tr!("browser.context.link-in-background"),
            MaterialSymbol::TAB_UNSELECTED,
            &url,
            move |shell: &mut Shell, url: String| shell.open_link(tab, url, false),
        ));
        entries.leaf(opening(
            tr!("browser.context.link-in-peek"),
            MaterialSymbol::PICTURE_IN_PICTURE,
            &url,
            move |shell: &mut Shell, url: String| shell.peek_link(tab, url),
        ));
        entries.leaf(opening(
            tr!("browser.context.link-in-split"),
            MaterialSymbol::SPLITSCREEN_RIGHT,
            &url,
            move |shell: &mut Shell, url: String| shell.open_link_in_split(tab, url),
        ));
    }
    entries.leaf(opening(
        tr!("browser.context.link-in-window"),
        MaterialSymbol::NEW_WINDOW,
        &url,
        |shell: &mut Shell, url: String| {
            let private = shell.private();
            shell.open_link_in_window(url, private)
        },
    ));
    if !shell.private() {
        entries.leaf(opening(
            tr!("browser.context.link-in-private-window"),
            MaterialSymbol::VISIBILITY_OFF,
            &url,
            |shell: &mut Shell, url: String| shell.open_link_in_window(url, true),
        ));
    }
    entries.separator();
    entries.leaf(saving(tr!("browser.context.save-link"), page, &url));
    entries.leaf(copying(
        tr!("browser.context.copy-link"),
        MaterialSymbol::LINK,
        &url,
    ));
    let text = link.text.trim();
    if !text.is_empty() {
        entries.leaf(copying(
            tr!("browser.context.copy-link-text"),
            MaterialSymbol::TEXT_FIELDS,
            text,
        ));
    }
}

fn picturing(entries: &mut Entries, page: &Page, image: &ContextImage) {
    let tab = page.tab;
    entries.separator();
    entries.leaf(opening(
        tr!("browser.context.image-in-tab"),
        MaterialSymbol::IMAGE,
        &image.url,
        move |shell: &mut Shell, url: String| shell.open_link(tab, url, true),
    ));
    entries.leaf(saving(tr!("browser.context.save-image"), page, &image.url));
    entries.leaf(copying(
        tr!("browser.context.copy-image-address"),
        MaterialSymbol::LINK,
        &image.url,
    ));
}

fn playing(entries: &mut Entries, page: &Page, media: &ContextMedia) {
    let tab = page.tab;
    let (open, save, copy, symbol) = match media.kind {
        MediaKind::Video => (
            tr!("browser.context.video-in-tab"),
            tr!("browser.context.save-video"),
            tr!("browser.context.copy-video-address"),
            MaterialSymbol::MOVIE,
        ),
        MediaKind::Audio => (
            tr!("browser.context.audio-in-tab"),
            tr!("browser.context.save-audio"),
            tr!("browser.context.copy-audio-address"),
            MaterialSymbol::MUSIC_NOTE,
        ),
    };
    entries.separator();
    entries.leaf(opening(
        open,
        symbol,
        &media.url,
        move |shell: &mut Shell, url: String| shell.open_link(tab, url, true),
    ));
    entries.leaf(saving(save, page, &media.url));
    entries.leaf(copying(copy, MaterialSymbol::LINK, &media.url));
}

fn selecting(entries: &mut Entries, shell: &Shell, page: &Page, selection: &str, copy: bool) {
    let tab = page.tab;
    entries.separator();
    if copy {
        entries.leaf(copying(
            tr!("browser.context.copy"),
            MaterialSymbol::CONTENT_COPY,
            selection,
        ));
    }
    let engines = shell.browser().search_engines();
    let engine = engines.default_engine().name().to_owned();
    let terms = selection.to_owned();
    entries.leaf(
        menu_item(tr!(
            "browser.context.search",
            engine = &engine,
            text = &quoted(selection)
        ))
        .icon(MaterialSymbol::SEARCH)
        .on_press(move |shell: &mut Shell| shell.search_selection(tab, &terms)),
    );
}

fn navigating(entries: &mut Entries, shell: &Shell, page: &Page) {
    let tab = page.tab;
    let (back, forward, web) = shell
        .model()
        .tab(tab)
        .map_or((false, false, false), |tab_entry| {
            (
                *tab_entry.meta.back,
                *tab_entry.meta.forward,
                browser_pages::web_site(&tab_entry.meta.url).is_some(),
            )
        });
    entries.separator();
    entries.leaf(
        menu_item(tr!("browser.context.back"))
            .icon(MaterialSymbol::ARROW_BACK)
            .shortcut(keys::shortcut(Command::Back))
            .enabled(back)
            .on_press(move |shell: &mut Shell| shell.page_back(tab)),
    );
    entries.leaf(
        menu_item(tr!("browser.context.forward"))
            .icon(MaterialSymbol::ARROW_FORWARD)
            .shortcut(keys::shortcut(Command::Forward))
            .enabled(forward)
            .on_press(move |shell: &mut Shell| shell.page_forward(tab)),
    );
    entries.leaf(
        menu_item(tr!("browser.context.reload"))
            .icon(MaterialSymbol::REFRESH)
            .shortcut(keys::shortcut(Command::Reload))
            .on_press(move |shell: &mut Shell| shell.page_reload(tab)),
    );
    let reader = shell.active_tab() == Some(tab) && shell.reader.offers(tab);
    if !web && !reader {
        return;
    }
    entries.separator();
    if web {
        let browser = shell.browser();
        let item = match browser.translations().tabs().shows_translation(tab) {
            true => menu_item(tr!("browser.context.show-original"))
                .on_press(move |shell: &mut Shell| shell.show_original(tab)),
            false => {
                let language = browser.preferences().translate_to();
                menu_item(tr!(
                    "browser.context.translate",
                    language = language.native_name()
                ))
                .on_press(move |shell: &mut Shell| shell.start_translation(tab))
            }
        };
        entries.leaf(item.icon(MaterialSymbol::TRANSLATE));
    }
    if reader {
        entries.leaf(
            menu_item(tr!("browser.context.reader"))
                .icon(MaterialSymbol::CHROME_READER_MODE)
                .shortcut(keys::shortcut(Command::ReaderMode))
                .on_press(|shell: &mut Shell| shell.toggle_reader()),
        );
    }
}

fn tools(entries: &mut Entries, page: &Page) {
    let tab = page.tab;
    entries.separator();
    entries.leaf(
        menu_item(tr!("browser.context.print"))
            .icon(MaterialSymbol::PRINT)
            .shortcut(keys::shortcut(Command::Print))
            .on_press(move |shell: &mut Shell| shell.print_tab(tab)),
    );
    entries.leaf(
        menu_item(tr!("browser.context.view-source"))
            .icon(MaterialSymbol::CODE)
            .shortcut(keys::shortcut(Command::ViewSource))
            .on_press(move |shell: &mut Shell| shell.view_source(tab)),
    );
    entries.leaf(
        menu_item(tr!("browser.context.inspect"))
            .icon(MaterialSymbol::BUG_REPORT)
            .on_press(move |shell: &mut Shell| shell.inspect(tab)),
    );
}

fn opening(
    label: Label,
    symbol: MaterialSymbol,
    url: &str,
    open: impl Fn(&mut Shell, String) + 'static,
) -> MenuItem {
    let url = url.to_owned();
    menu_item(label)
        .icon(symbol)
        .on_press(move |shell: &mut Shell| open(shell, url.clone()))
}

fn saving(label: Label, page: &Page, url: &str) -> MenuItem {
    let tab = page.tab;
    let url = url.to_owned();
    menu_item(label)
        .icon(MaterialSymbol::DOWNLOAD)
        .on_press(move |shell: &mut Shell| shell.save_link(tab, &url))
}

fn copying(label: Label, symbol: MaterialSymbol, text: &str) -> MenuItem {
    let text = text.to_owned();
    menu_item(label)
        .icon(symbol)
        .on_press(move |_: &mut Shell| rikka_ui::clipboard::set_text(&text))
}

fn quoted(selection: &str) -> String {
    let flat = selection.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.char_indices().nth(QUOTED) {
        Some((end, _)) => format!("{}…", &flat[..end]),
        None => flat,
    }
}
