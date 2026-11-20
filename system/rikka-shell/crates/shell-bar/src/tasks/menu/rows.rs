use rikka_format::truncate;
use rikka_session::workspaces::COUNT;
use rikka_ui::{Label, MaterialSymbol, tr};
use rikka_widgets::{MenuItem, menu_header, menu_item, menu_separator};

use crate::tasks::menu::flyout::{Flyout, MenuAction};
use crate::tasks::menu::opened::MenuRequest;

const TITLE: usize = 44;

#[must_use]
pub(crate) fn rows(request: &MenuRequest) -> Vec<MenuItem> {
    let mut rows = vec![menu_header(truncate(&request.name, TITLE).into_owned())];
    if request.windows.len() > 1 {
        rows.extend(request.windows.iter().map(|window| {
            command(title(&window.title), MenuAction::Focus(window.id.clone()))
                .icon(match window.minimized {
                    true => MaterialSymbol::MINIMIZE,
                    false => MaterialSymbol::WEB_ASSET,
                })
                .selected(window.focused)
        }));
        rows.push(menu_separator());
    }
    if request.starts.is_some() {
        rows.push(
            command(tr!("shell.tasks.new-window"), MenuAction::Start).icon(MaterialSymbol::ADD),
        );
    }
    if let [window] = request.windows.as_slice() {
        rows.push(match window.maximized {
            true => command(
                tr!("shell.tasks.restore"),
                MenuAction::Unmaximize(window.id.clone()),
            )
            .icon(MaterialSymbol::FILTER_NONE),
            false => command(
                tr!("shell.tasks.maximize"),
                MenuAction::Maximize(window.id.clone()),
            )
            .icon(MaterialSymbol::CROP_SQUARE),
        });
        if !window.tiled && !window.minimized {
            rows.push(
                command(
                    tr!("shell.tasks.minimize"),
                    MenuAction::Minimize(window.id.clone()),
                )
                .icon(MaterialSymbol::MINIMIZE),
            );
        }
    }
    rows.push(
        menu_item(match request.windows.len() {
            1 => tr!("shell.tasks.move"),
            _ => tr!("shell.tasks.move-all"),
        })
        .icon(MaterialSymbol::DRIVE_FILE_MOVE)
        .items(
            (1..=COUNT)
                .filter(|number| *number != request.space)
                .map(|number| {
                    command(
                        tr!("shell.tasks.workspace", number = number),
                        MenuAction::Move { workspace: number },
                    )
                }),
        ),
    );
    rows.push(menu_separator());
    rows.push(
        command(
            match request.windows.len() {
                1 => tr!("shell.tasks.close"),
                _ => tr!("shell.tasks.close-all"),
            },
            MenuAction::Close,
        )
        .icon(MaterialSymbol::CLOSE),
    );
    rows
}

fn command(label: impl Into<Label>, action: MenuAction) -> MenuItem {
    menu_item(label).on_press(move |flyout: &mut Flyout| flyout.act(action.clone()))
}

fn title(window_title: &str) -> Label {
    match window_title.trim().is_empty() {
        true => tr!("shell.tasks.untitled"),
        false => Label::from(truncate(window_title, TITLE).into_owned()),
    }
}
