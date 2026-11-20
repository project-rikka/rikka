use rikka_ui::{MaterialSymbol, tr};
use rikka_widgets::{MenuItem, menu_item, menu_separator};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuAction {
    Edit,
    Remove,
    Wallpaper,
    Settings,
    Nothing,
}

pub(crate) struct Row {
    pub(crate) item: MenuItem,
    pub(crate) doing: MenuAction,
}

#[must_use]
pub(crate) fn rows(on_widget: bool) -> Vec<Row> {
    let mut rows = vec![Row {
        item: menu_item(tr!("shell.desk.menu.edit")).icon(MaterialSymbol::DASHBOARD_CUSTOMIZE),
        doing: MenuAction::Edit,
    }];
    if on_widget {
        rows.push(Row {
            item: menu_item(tr!("shell.desk.menu.remove")).icon(MaterialSymbol::DELETE),
            doing: MenuAction::Remove,
        });
    }
    rows.push(Row {
        item: menu_separator(),
        doing: MenuAction::Nothing,
    });
    rows.push(Row {
        item: menu_item(tr!("shell.desk.menu.wallpaper")).icon(MaterialSymbol::WALLPAPER),
        doing: MenuAction::Wallpaper,
    });
    rows.push(Row {
        item: menu_item(tr!("shell.desk.menu.settings")).icon(MaterialSymbol::DISPLAY_SETTINGS),
        doing: MenuAction::Settings,
    });
    rows
}
