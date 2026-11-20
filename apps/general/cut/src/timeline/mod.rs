mod headers;
pub(crate) mod keys;
pub(crate) mod lanes;
mod marker_editor;
mod markers;
mod playhead;
mod rows;
mod ruler;
mod scale;
mod scrollbar;
mod sizing;
mod toolbar;

use rikka_app::prelude::*;
use rikka_widgets::{column, row};

pub(crate) use scale::Scale;

use crate::caption::{effect_menu, transition_menu};
use crate::command::Command;
use crate::controls::corner;
use crate::cut::{Cut, Pointing};
use crate::tool::Tool;
use crate::wording;
use keys::Outline;
use lanes::{Disclosure, Lanes};
use playhead::playhead;
use rows::Rows;
use ruler::Ruler;
use scrollbar::Scrollbar;

const MENU_PIN: &str = "cut-menu-pin";
const MENU: f32 = 220.0;

pub(crate) fn timeline(cut: &Cut) -> impl View {
    let project = cut.project();
    let outline = cut
        .opened_clip()
        .and_then(|clip| Outline::for_clip(project, clip, cut.keyframing().is_graph()));
    let rows = Rows::layout(project, cut.resizing());
    let rows = match &outline {
        Some(outline) => rows.with_expanded(outline.layer, outline.height()),
        None => rows,
    };
    let top = row! {
        corner(headers::WIDTH, ruler::HEIGHT);
        Ruler {
            project: project.clone(),
            live: cut.live().clone(),
            scale: cut.scale().clone(),
        }
        .build()
        .keyed(ruler::KEY)
        .grow(1.0);
    }
    .fill_width();
    let lanes = Lanes {
        project: project.clone(),
        rows: rows.clone(),
        selected: cut.selected().clone(),
        edit_point: cut.selected_edit_point(),
        offline: cut.offline().clone(),
        snapping: cut.snapping(),
        trimmer: cut.trimmer(),
        scale: cut.scale().clone(),
        transport: cut.transport().clone(),
        disclosure: cut.selected_animated().map(|clip| Disclosure {
            clip: clip.id(),
            open: cut.keyframing().is_open(),
        }),
        media_previews: cut.media_previews().clone(),
        display: cut.display(),
        live: cut.live().clone(),
    };
    let opening = rows.opening().map(|opening| opening.top);
    let keyed = outline.clone().zip(opening);
    let field = column! {
        lanes.build().keyed(lanes::KEY).fill_width();
        if let Some((outline, top)) = keyed {
            keys::keys(cut, outline, top)
                .position(0.0, top)
                .fill_width();
        }
        keys::pin(cut);
        if let Some(pointing) = cut.pointing() {
            container(spacer())
                .size(0.0, 0.0)
                .position(pointing.at.x, pointing.at.y)
                .build()
                .keyed(MENU_PIN);
        }
    }
    .grow(1.0);
    let body = scrollable(
        row! {
            headers::headers(
                project,
                &rows,
                outline.map(|outline| keys::header(cut, outline, headers::WIDTH)),
            );
            field;
        }
        .fill_width(),
    )
    .grow(1.0)
    .fill_width();
    let overlay = playhead(cut.scale(), cut.transport(), headers::WIDTH);
    column! {
        toolbar::bar(cut);
        column! {
            top;
            body;
            overlay.build().position(0.0, 0.0).fill();
        }
        .grow(1.0)
        .fill_width();
        row! {
            corner(headers::WIDTH, scrollbar::HEIGHT);
            Scrollbar {
                scale: cut.scale().clone(),
            }
            .build()
            .grow(1.0);
        }
        .fill_width();
        if let Some(pointing) = cut.pointing() {
            menu_at(cut, *pointing);
        }
        keys::context_menu(cut);
        if let Some(editor) = marker_editor::editor(cut) {
            editor;
        }
    }
    .background(palette().background)
    .fill()
}

fn menu_at(cut: &Cut, pointing: Pointing) -> Element {
    let entry = |command: Command| command.entry(cut);
    let Pointing { frame, layer, .. } = pointing;
    let range = cut.range_shape();
    let mut adding: Vec<MenuItem> = Tool::made()
        .map(|tool| {
            menu_item(wording::tool_name(tool))
                .on_press(move |cut: &mut Cut| cut.add_at(tool, frame, layer))
        })
        .collect();
    adding.extend([
        menu_separator(),
        menu_item(wording::command(Command::AddRange))
            .on_press(move |cut: &mut Cut| cut.add_range_at(Some(range), frame, layer)),
        menu_item(wording::command(Command::AddWhole))
            .on_press(move |cut: &mut Cut| cut.add_range_at(None, frame, layer)),
        menu_item(wording::command(Command::AddCamera))
            .on_press(move |cut: &mut Cut| cut.add_camera_at(frame, layer)),
    ]);
    menu()
        .items([
            menu_item(tr!("cut.menu.add_item")).items(adding),
            effect_menu(cut),
            transition_menu(cut),
            menu_separator(),
            entry(Command::Split),
            menu_separator(),
            entry(Command::Cut),
            entry(Command::Copy),
            menu_item(crate::wording::command(Command::Paste))
                .enabled(cut.can_paste())
                .on_press(move |cut: &mut Cut| cut.paste_at(frame, layer)),
            entry(Command::PasteAttributes),
            entry(Command::PasteEffects),
            menu_separator(),
            entry(Command::Delete),
            entry(Command::RippleDelete),
            entry(Command::RemoveEffects),
            menu_separator(),
            entry(Command::ReplaceMedia),
            entry(Command::RelinkMedia),
        ])
        .width(MENU)
        .on_activate(|cut: &mut Cut, _: usize| cut.close_menu())
        .on_dismiss(Cut::close_menu)
        .build()
        .keyed(("cut-menu", pointing.layer, pointing.frame))
        .anchored(Anchor::new(MENU_PIN, AnchorEdge::BelowStart))
}
