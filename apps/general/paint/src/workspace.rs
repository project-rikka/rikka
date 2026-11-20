use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::canvas::Canvas;
use crate::command::{Command, Objecting};
use crate::paint::Paint;
use crate::tab::{Phase, Tab};
use crate::{options, panel, status, tools};

const SPINNER: f32 = 32.0;
const MENU: f32 = 240.0;
const SPOT: &str = "paint-figure-spot";

pub(crate) fn workspace(paint: &Paint, tab: &Tab) -> impl View {
    let open = matches!(tab.phase(), Phase::Open(_));
    column! {
        row! {
            tools::rail(paint);
            rule_upright();
            column! {
                if open {
                    options::bar(paint, tab);
                    rule();
                }
                container(stage(paint, tab)).grow(1.0).fill_width();
            }
            .grow(1.0)
            .fill_height();
            if open {
                rule_upright();
                panel::side(paint, tab);
            }
        }
        .grow(1.0)
        .fill_width();
        rule();
        container(status::bar(tab))
            .background(palette().surface)
            .fill_width();
    }
    .fill()
}

fn stage(paint: &Paint, tab: &Tab) -> Element {
    match tab.phase() {
        Phase::Opening => container(
            column! {
                spinner().extent(Length::px(SPINNER));
                text(tr!("paint.stage.opening", name = &tab.name())).muted();
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center),
        )
        .center()
        .fill()
        .any(),
        Phase::Failed(reason) => {
            let id = tab.id();
            status_page(MaterialSymbol::BROKEN_IMAGE, reason.clone())
                .tint(palette().danger)
                .action(
                    button(text(tr!("paint.command.close_tab")))
                        .on_press(move |paint: &mut Paint| paint.close_tab(id)),
                )
                .fill()
                .any()
        }
        Phase::Open(_) => {
            let canvas = Element::new(Canvas::new(paint, tab)).keyed(tab.id());
            match tab.figure_menu() {
                Some(at) => column_of(vec![
                    canvas,
                    container(spacer())
                        .size(1.0, 1.0)
                        .position(at.x, at.y)
                        .build()
                        .keyed((SPOT, tab.id())),
                    figure_menu(paint, tab.id()),
                ])
                .fill()
                .any(),
                None => canvas,
            }
        }
    }
}

fn figure_menu(paint: &Paint, tab: u64) -> Element {
    let entry = |objecting: Objecting, label: Label| Command::Object(objecting).entry(label, paint);
    let mut items = vec![
        entry(Objecting::Cut, tr!("paint.command.cut")),
        entry(Objecting::Copy, tr!("paint.command.copy")),
        entry(Objecting::Paste, tr!("paint.command.paste")),
        entry(Objecting::Duplicate, tr!("paint.command.duplicate")),
        entry(Objecting::Delete, tr!("paint.command.delete")),
        menu_separator(),
    ];
    items.extend(crate::caption::arrange_items(paint));
    items.extend([
        menu_separator(),
        entry(Objecting::Rasterize, tr!("paint.command.rasterize_objects")),
    ]);
    menu()
        .items(items)
        .width(MENU)
        .on_activate(|paint: &mut Paint, _: usize| paint.close_figure_menu())
        .on_dismiss(Paint::close_figure_menu)
        .build()
        .keyed(("paint-figure-menu", tab))
        .anchored(Anchor::new((SPOT, tab), AnchorEdge::BelowStart))
}
