use rikka_symbols::MaterialSymbol;
use rikka_ui::text::Label;
use rikka_ui::{Command, tr};

use crate::navigation::menu::{MenuItem, menu_item};

pub fn command_item(command: Command) -> MenuItem {
    MenuItem {
        command: Some(command),
        ..menu_item(command_label(command))
            .icon(symbol(command))
            .shortcut(command.chord().to_string())
    }
}

pub fn command_label(command: Command) -> Label {
    match command {
        Command::Back => tr!("widgets.command.back"),
        Command::Forward => tr!("widgets.command.forward"),
        Command::Up => tr!("widgets.command.up"),
        Command::Home => tr!("widgets.command.home"),
        Command::Refresh => tr!("widgets.command.refresh"),
        Command::Undo => tr!("widgets.command.undo"),
        Command::Redo => tr!("widgets.command.redo"),
        Command::Cut => tr!("widgets.command.cut"),
        Command::Copy => tr!("widgets.command.copy"),
        Command::Paste => tr!("widgets.command.paste"),
        Command::SelectAll => tr!("widgets.command.select-all"),
        Command::SelectNone => tr!("widgets.command.select-none"),
        Command::Delete => tr!("widgets.command.delete"),
        Command::Find => tr!("widgets.command.find"),
        Command::FindNext => tr!("widgets.command.find-next"),
        Command::FindPrevious => tr!("widgets.command.find-previous"),
        Command::ZoomIn => tr!("widgets.command.zoom-in"),
        Command::ZoomOut => tr!("widgets.command.zoom-out"),
        Command::ZoomReset => tr!("widgets.command.zoom-reset"),
        Command::New => tr!("widgets.command.new"),
        Command::NewTab => tr!("widgets.command.new-tab"),
        Command::Open => tr!("widgets.command.open"),
        Command::Save => tr!("widgets.command.save"),
        Command::SaveAs => tr!("widgets.command.save-as"),
        Command::Close => tr!("widgets.command.close"),
        Command::Quit => tr!("widgets.command.quit"),
        Command::NextTab => tr!("widgets.command.next-tab"),
        Command::PreviousTab => tr!("widgets.command.previous-tab"),
        Command::Rename => tr!("widgets.command.rename"),
        Command::Properties => tr!("widgets.command.properties"),
        Command::Fullscreen => tr!("widgets.command.fullscreen"),
        Command::Preferences => tr!("widgets.command.preferences"),
        Command::ContextMenu => tr!("widgets.command.context-menu"),
    }
}

fn symbol(command: Command) -> MaterialSymbol {
    match command {
        Command::Back => MaterialSymbol::ARROW_BACK,
        Command::Forward => MaterialSymbol::ARROW_FORWARD,
        Command::Up => MaterialSymbol::ARROW_UPWARD,
        Command::Home => MaterialSymbol::HOME,
        Command::Refresh => MaterialSymbol::REFRESH,
        Command::Undo => MaterialSymbol::UNDO,
        Command::Redo => MaterialSymbol::REDO,
        Command::Cut => MaterialSymbol::CONTENT_CUT,
        Command::Copy => MaterialSymbol::CONTENT_COPY,
        Command::Paste => MaterialSymbol::CONTENT_PASTE,
        Command::SelectAll => MaterialSymbol::SELECT_ALL,
        Command::SelectNone => MaterialSymbol::DESELECT,
        Command::Delete => MaterialSymbol::DELETE,
        Command::Find => MaterialSymbol::SEARCH,
        Command::FindNext => MaterialSymbol::KEYBOARD_ARROW_DOWN,
        Command::FindPrevious => MaterialSymbol::KEYBOARD_ARROW_UP,
        Command::ZoomIn => MaterialSymbol::ZOOM_IN,
        Command::ZoomOut => MaterialSymbol::ZOOM_OUT,
        Command::ZoomReset => MaterialSymbol::FIT_SCREEN,
        Command::New => MaterialSymbol::NOTE_ADD,
        Command::NewTab => MaterialSymbol::ADD,
        Command::Open => MaterialSymbol::FOLDER_OPEN,
        Command::Save => MaterialSymbol::SAVE,
        Command::SaveAs => MaterialSymbol::SAVE_AS,
        Command::Close => MaterialSymbol::CLOSE,
        Command::Quit => MaterialSymbol::LOGOUT,
        Command::NextTab => MaterialSymbol::CHEVRON_RIGHT,
        Command::PreviousTab => MaterialSymbol::CHEVRON_LEFT,
        Command::Rename => MaterialSymbol::EDIT,
        Command::Properties => MaterialSymbol::INFO,
        Command::Fullscreen => MaterialSymbol::FULLSCREEN,
        Command::Preferences => MaterialSymbol::SETTINGS,
        Command::ContextMenu => MaterialSymbol::MENU,
    }
}
