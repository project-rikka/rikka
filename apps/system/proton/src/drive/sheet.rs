use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use crate::drive::{Drive, Sheet};

pub(crate) const MARK: &str = "proton-drive-mark";
pub(crate) const STAGE: &str = "proton-drive-stage";

const WIDTH: f32 = 380.0;

pub(crate) fn stage(body: impl View) -> Element {
    body.build().keyed(STAGE)
}

pub(crate) fn mark() -> Element {
    spacer().width(0.0).fill_height().build().keyed(MARK)
}

pub(crate) fn view(shown: &Sheet) -> Element {
    match shown {
        Sheet::Naming(name) => dialog(
            "proton-drive-naming",
            tr!("proton.drive.new-folder"),
            text_field(name)
                .placeholder(tr!("proton.drive.folder-name"))
                .autofocus(true)
                .on_submit(Drive::make_folder)
                .fill_width(),
            tr!("proton.drive.make"),
            Drive::make_folder,
        ),
        Sheet::Renaming(_, name) => dialog(
            "proton-drive-renaming",
            tr!("proton.drive.rename"),
            text_field(name)
                .placeholder(tr!("proton.drive.name"))
                .autofocus(true)
                .on_submit(Drive::rename)
                .fill_width(),
            tr!("proton.drive.save"),
            Drive::rename,
        ),
    }
}

fn dialog<Accept>(
    key: &'static str,
    heading: impl Into<Label>,
    field: impl View,
    go: impl Into<Label>,
    accept: impl rikka_ui::IntoHandler<(), (), Accept>,
) -> Element {
    sheet(
        column! {
            text(heading).label().semibold();
            field;
            row! {
                spacer().grow(1.0);
                button(text(tr!("proton.drive.cancel")))
                    .ghost()
                    .on_press(Drive::close_sheet);
                button(text(go))
                    .primary()
                    .on_press(accept);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
        }
        .gap(Space::ROOMY)
        .padding(20.0)
        .fill_width(),
    )
    .width(WIDTH)
    .keyed(key)
    .centered_on(STAGE)
    .build()
}
