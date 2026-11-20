use rikka_app::prelude::*;

use crate::drive::Drive;
use crate::drive::sheet::MARK;

const WIDTH: f32 = 248.0;

pub(crate) fn view(drive: &Drive, at: Point) -> Element {
    menu()
        .items(items(drive))
        .width(WIDTH)
        .on_dismiss(|drive: &mut Drive| drive.close_menu())
        .build()
        .anchored(Anchor::new(MARK, AnchorEdge::BesideTop).offset(at.x, at.y))
}

fn items(drive: &Drive) -> Vec<MenuItem> {
    let Some(chosen) = drive.selected().and_then(|at| drive.tree().entry(at)) else {
        return vec![
            item(
                tr!("proton.drive.new-folder"),
                MaterialSymbol::CREATE_NEW_FOLDER,
                Drive::start_folder,
            ),
            item(
                tr!("proton.drive.upload"),
                MaterialSymbol::UPLOAD,
                Drive::upload,
            ),
            menu_separator(),
            item(
                tr!("proton.drive.refresh"),
                MaterialSymbol::REFRESH,
                Drive::refresh,
            ),
        ];
    };

    let mut items = vec![item(
        tr!("proton.drive.open"),
        MaterialSymbol::OPEN_IN_NEW,
        Drive::open_selected,
    )];
    if !chosen.folder {
        items.push(item(
            tr!("proton.drive.download"),
            MaterialSymbol::DOWNLOAD,
            Drive::download_selected,
        ));
    }
    items.extend([
        menu_separator(),
        item(
            tr!("proton.drive.rename"),
            MaterialSymbol::DRIVE_FILE_RENAME_OUTLINE,
            Drive::start_rename,
        ),
        menu_separator(),
        item(
            tr!("proton.drive.trash"),
            MaterialSymbol::DELETE,
            Drive::throw,
        ),
        item(
            tr!("proton.drive.delete"),
            MaterialSymbol::DELETE_FOREVER,
            Drive::erase,
        ),
    ]);
    items
}

fn item(label: Label, symbol: MaterialSymbol, act: fn(&mut Drive)) -> MenuItem {
    menu_item(label)
        .icon(symbol)
        .on_press(move |drive: &mut Drive| {
            drive.close_menu();
            act(drive);
        })
}
