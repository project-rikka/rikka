use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use proton_files::{Direction, SortKey, State, Transfer};
use rikka_app::prelude::*;
use rikka_ui::Handler;
use rikka_widgets::{Surface, column, row};

use crate::drive::backdrop::backdrop;
use crate::drive::cell::{Cell, Shown};
use crate::drive::layout::{Column, GUTTER, ROW, TILE, TILE_LINES, tile_height};
use crate::drive::{Drive, sheet};
use crate::pane;

pub(crate) fn toolbar(drive: &Drive) -> impl View {
    let depth = drive.tree().depth();
    let tiles = drive.shows_tiles();
    let orders = [
        tr!("proton.drive.by-name"),
        tr!("proton.drive.by-time"),
        tr!("proton.drive.by-kind"),
        tr!("proton.drive.by-size"),
    ];
    let order = match drive.tree().sorted() {
        SortKey::Name => 0,
        SortKey::Modified => 1,
        SortKey::Kind => 2,
        SortKey::Size => 3,
    };

    pane::toolbar(
        row! {
            button(icon(MaterialSymbol::ARROW_BACK).extent(Length::px(15.0)))
                .ghost()
                .small()
                .icon_only()
                .enabled(depth > 1)
                .on_press(move |drive: &mut Drive| drive.back_to(depth.saturating_sub(1)));
            crumbs(drive).grow(1.0);
            button(
                row! {
                    icon(MaterialSymbol::CREATE_NEW_FOLDER).extent(Length::px(15.0));
                    text(tr!("proton.drive.new-folder")).caption().no_wrap();
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center),
            )
            .ghost()
            .small()
            .on_press(Drive::start_folder);
            button(
                row! {
                    icon(MaterialSymbol::UPLOAD).extent(Length::px(15.0));
                    text(tr!("proton.drive.upload")).caption().no_wrap();
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center),
            )
            .primary()
            .small()
            .on_press(Drive::upload);
            combo(order)
                .options(orders)
                .control_size(ControlSize::Small)
                .width(132.0)
                .on_select(|drive: &mut Drive, at: usize| {
                    drive.sort(match at {
                        1 => SortKey::Modified,
                        2 => SortKey::Kind,
                        3 => SortKey::Size,
                        _ => SortKey::Name,
                    });
                });
            button(
                icon(match tiles {
                    true => MaterialSymbol::VIEW_LIST,
                    false => MaterialSymbol::GRID_VIEW,
                })
                .extent(Length::px(15.0)),
            )
            .ghost()
            .small()
            .icon_only()
            .on_press(move |drive: &mut Drive| drive.tile(!tiles));
            button(icon(MaterialSymbol::REFRESH).extent(Length::px(15.0)))
                .ghost()
                .small()
                .icon_only()
                .loading(drive.is_busy())
                .on_press(Drive::refresh);
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width(),
    )
}

fn crumbs(drive: &Drive) -> Element {
    let items: Vec<Element> = drive
        .tree()
        .crumbs()
        .iter()
        .enumerate()
        .map(|(at, crumb)| {
            let last = at + 1 == drive.tree().depth();
            row! {
                if at > 0 {
                    icon(MaterialSymbol::CHEVRON_RIGHT)
                        .extent(Length::px(14.0))
                        .color(palette().text_muted);
                }
                button(text(crumb.name.clone()).caption().no_wrap())
                    .ghost()
                    .small()
                    .enabled(!last)
                    .on_press(move |drive: &mut Drive| drive.back_to(at + 1));
            }
            .gap(Space::HAIR)
            .align_items(Align::Center)
            .build()
        })
        .collect();

    row_of(items)
        .gap(Space::HAIR)
        .align_items(Align::Center)
        .build()
}

pub(crate) fn view(drive: &Drive) -> Element {
    let aside = Handler::new(|drive: &mut Drive, at: Point| drive.open_menu_aside(at));

    row! {
        sheet::mark();
        backdrop(body(drive), aside).grow(1.0).fill_height();
    }
    .grow(1.0)
    .fill_width()
    .build()
}
fn body(drive: &Drive) -> Element {
    let entries = Arc::clone(drive.tree().entries());

    if entries.is_empty() {
        return match drive.is_busy() {
            true => pane::working(tr!("proton.drive.reading")).build(),
            false => pane::nothing(MaterialSymbol::CLOUD, tr!("proton.drive.empty")).build(),
        };
    }

    let tiles = drive.shows_tiles();
    let chosen = drive.selected();
    let count = entries.len();
    let shown = Rc::new(Shown {
        entries,
        thumbs: drive.thumbs().all(),
        chosen,
        tiles,
        widths: drive.widths().clone(),
    });
    let secondary = Secondary {
        above: match tiles {
            true => 0.0,
            false => COLUMN_HEADER,
        },
        chosen,
        opened: Handler::new(|drive: &mut Drive, at: Point| drive.open_menu(at)),
        picked: Handler::new(|drive: &mut Drive, at: Option<usize>| drive.pick(at)),
    };

    let height = match tiles {
        true => tile_height(TILE_LINES),
        false => ROW,
    };
    let list = lazy_column(count, move |slot| {
        Cell {
            shown: Rc::clone(&shown),
            index: slot.index,
        }
        .element(height)
    })
    .uniform(height)
    .selected(chosen)
    .focus(chosen)
    .place(drive.place())
    .shows(drive.tree().path())
    .on_select(|drive: &mut Drive, at: usize| drive.select(at))
    .on_activate(|drive: &mut Drive, at: usize| drive.enter(at))
    .on_visible(|drive: &mut Drive, range: Range<usize>| drive.showing(range))
    .on_pointer(move |event, ctx, surface| secondary.caught(event, ctx, surface))
    .grow(1.0)
    .fill_width();

    if tiles {
        return list
            .tiles(Length::px(TILE))
            .gap(Space::ROOMY)
            .padding(Space::NORMAL)
            .build();
    }

    let sorted = Column::ALL
        .iter()
        .position(|column| column.sorted() == drive.tree().sorted());

    column! {
        column_header(Column::ALL.map(Column::spec), drive.widths())
            .sorted(sorted, drive.tree().is_descending())
            .gutter(GUTTER)
            .on_sort(|drive: &mut Drive, at: usize| {
                if let Some(column) = Column::ALL.get(at) {
                    drive.sort(column.sorted());
                }
            })
            .build()
            .fill_width()
            .height(COLUMN_HEADER);
        list;
    }
    .fill()
    .build()
}

struct Secondary {
    above: f32,
    chosen: Option<usize>,
    opened: Handler<(Point,)>,
    picked: Handler<(Option<usize>,)>,
}

impl Secondary {
    fn caught(&self, event: &Event, ctx: &mut EventCtx<'_>, surface: &Surface<'_>) -> bool {
        let Event::PointerPressed {
            position,
            button: PointerButton::Secondary,
            ..
        } = *event
        else {
            return false;
        };

        ctx.request_focus();
        let hit = surface
            .visible()
            .find(|index| surface.cell(*index).contains(position));
        if hit != self.chosen {
            ctx.call(&self.picked, (hit,));
        }

        let bounds = ctx.bounds();
        let at = Point::new(
            position.x - bounds.origin.x,
            position.y - bounds.origin.y + self.above,
        );
        ctx.call(&self.opened, (at,));
        ctx.set_handled();
        true
    }
}

pub(crate) fn transfers(drive: &Drive) -> Option<Element> {
    if drive.transfers().is_empty() {
        return None;
    }

    Some(
        column! {
            rule();
            column_of(
                drive
                    .transfers()
                    .all()
                    .iter()
                    .map(|transfer| moving(transfer).build()),
            )
            .gap(Space::TIGHT)
            .padding(Space::SNUG)
            .fill_width();
        }
        .fill_width()
        .build(),
    )
}

fn moving(transfer: &Transfer) -> impl View {
    let id = transfer.id;
    let (symbol, tint) = match (&transfer.state, transfer.direction) {
        (State::Failed(_), _) => (MaterialSymbol::ERROR, palette().danger),
        (State::Done, _) => (MaterialSymbol::CHECK_CIRCLE, palette().success),
        (State::Running, Direction::Up) => (MaterialSymbol::UPLOAD, palette().accent),
        (State::Running, Direction::Down) => (MaterialSymbol::DOWNLOAD, palette().accent),
        (State::Running, Direction::View) => (MaterialSymbol::VISIBILITY, palette().accent),
    };
    let status = match &transfer.state {
        State::Failed(reason) => reason.clone(),
        State::Done => tr!("proton.drive.done").as_str().to_owned(),
        State::Running => match transfer.share() {
            Some(share) => format!("{:.0}%", share * 100.0),
            None => tr!("proton.drive.moving").as_str().to_owned(),
        },
    };

    row! {
        icon(symbol).extent(Length::px(15.0)).color(tint);
        text(transfer.name.clone()).caption().no_wrap().grow(1.0);
        if transfer.is_running() {
            match transfer.share() {
                Some(share) => progress(share).width(120.0).build(),
                None => progress_indeterminate().width(120.0).build(),
            }
        }
        text(status).caption().muted().no_wrap();
        button(icon(MaterialSymbol::CLOSE).extent(Length::px(13.0)))
            .ghost()
            .small()
            .icon_only()
            .on_press(move |drive: &mut Drive| drive.forget(id));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}
