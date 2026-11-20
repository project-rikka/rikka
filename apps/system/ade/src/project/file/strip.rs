use std::path::{Path, PathBuf};
use std::rc::Rc;

use rikka_app::prelude::{Event as Input, *};
use rikka_widgets::{MenuItem, menu_separator, row};

use crate::labels;
use crate::project::act::{act, given};
use crate::project::file::state::{Closing, Shown, TabMenu, Viewer};
use crate::project::glyph;
use crate::project::state::Project;
use crate::project::tab::Tab;

const HEIGHT: f32 = 34.0;
const GLYPH: f32 = 14.0;
const FONT: f32 = 13.0;
const MARK: &str = "ade-tabs-mark";
const MENU: &str = "ade-tabs-menu";

#[derive(Clone, Debug)]
struct Look {
    path: PathBuf,
    name: String,
    pinned: bool,
    unsaved: bool,
    active: bool,
}

pub(in crate::project) fn view(viewer: &Viewer) -> Element {
    let looks: Rc<[Look]> = viewer
        .tabs()
        .iter()
        .enumerate()
        .map(|(at, shown)| look(shown, viewer.active() == Some(at)))
        .collect();
    let drawn = Rc::clone(&looks);
    let count = looks.len();
    rikka_widgets::column! {
        spacer().width(0.0).height(0.0).build().keyed(MARK);
        lazy_row(count, move |slot| tab_view(slot.index, &drawn[slot.index]))
            .estimate(160.0)
            .reveals(viewer.active())
            .activates_on_press(true)
            .on_activate(given(|project, at: usize| {
                project.viewer(|viewer, git| viewer.activate(at, git));
            }))
            .on_pointer(move |event, ctx, surface| pointed(event, ctx, surface, count))
            .height(HEIGHT)
            .fill_width();
        if let Some(menu) = viewer.menu() {
            tab_menu(viewer, menu);
        }
    }
    .fill_width()
    .any()
}

fn look(shown: &Shown, active: bool) -> Look {
    Look {
        path: shown.path().to_owned(),
        name: labels::file_name(shown.path()),
        pinned: shown.is_pinned(),
        unsaved: shown.is_unsaved(),
        active,
    }
}

fn pointed(
    event: &Input,
    ctx: &mut EventCtx<'_>,
    surface: &rikka_widgets::Surface<'_>,
    count: usize,
) -> bool {
    let Input::PointerPressed {
        button, position, ..
    } = *event
    else {
        return false;
    };
    let Some(at) = surface.at(position).filter(|at| *at < count) else {
        return false;
    };
    match button {
        PointerButton::Middle => {
            ctx.set_handled();
            ctx.call(
                &Handler::new(given(|project, at: usize| {
                    project.viewer(|viewer, git| viewer.close(at, git));
                })),
                (at,),
            );
            true
        }
        PointerButton::Secondary => {
            ctx.set_handled();
            let origin = ctx.bounds().origin;
            let point = Point::new(position.x - origin.x, position.y - origin.y);
            ctx.call(
                &Handler::new(given(|project, (at, point): (usize, Point)| {
                    project.viewer(|viewer, _| viewer.summon(at, point));
                })),
                ((at, point),),
            );
            true
        }
        PointerButton::Primary if ctx.presses() >= 2 => {
            ctx.call(
                &Handler::new(given(|project, at: usize| {
                    project.viewer(|viewer, _| viewer.pin(at));
                })),
                (at,),
            );
            false
        }
        _ => false,
    }
}

fn tab_view(at: usize, look: &Look) -> Element {
    let palette = palette();
    let ink = match look.active {
        true => palette.text,
        false => palette.text_muted,
    };
    let name = text(look.name.as_str())
        .text_size(FONT)
        .color(ink)
        .no_wrap();
    let name = match look.pinned {
        true => name,
        false => name.italic(),
    };
    let closer = match look.unsaved {
        true => MaterialSymbol::CIRCLE,
        false => MaterialSymbol::CLOSE,
    };
    let closer_size = match look.unsaved {
        true => 9.0,
        false => GLYPH,
    };
    let background = match look.active {
        true => palette.surface,
        false => Color::TRANSPARENT,
    };
    let rule = match look.active {
        true => palette.accent,
        false => Color::TRANSPARENT,
    };
    rikka_widgets::column! {
        row! {
            icon(glyph::file(&look.path))
                .extent(Length::px(GLYPH))
                .color(palette.text_muted);
            name;
            button(icon(closer).extent(Length::px(closer_size)))
                .icon_only()
                .ghost()
                .small()
                .on_press(act(move |project| project.viewer(|viewer, git| viewer.close(at, git))));
        }
        .gap(6.0)
        .align_items(Align::Center)
        .padding(Inset::exact(0.0, 2.0, 0.0, 12.0))
        .grow(1.0);
        container(spacer()).background(rule).height(2.0).fill_width();
    }
    .background(background)
    .height(HEIGHT)
    .any()
}

fn tab_menu(viewer: &Viewer, menu: TabMenu) -> Element {
    let Some(shown) = viewer.tabs().get(menu.tab) else {
        return Element::empty();
    };
    menu_view(menu, shown.path(), viewer.root(), viewer.tabs().len())
}

fn menu_view(menu: TabMenu, path: &Path, root: &Path, count: usize) -> Element {
    let at = menu.tab;
    let closing = |label: Label, which: Closing, enabled: bool| {
        menu_item(label)
            .enabled(enabled)
            .on_press(act(move |project| {
                project.viewer(|viewer, git| {
                    viewer.put_menu_away();
                    viewer.close_many(which, at, git);
                });
            }))
    };
    let absolute = path.display().to_string();
    let relative = path
        .strip_prefix(root)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| absolute.clone());
    let revealed = path.to_owned();
    let items: Vec<MenuItem> = vec![
        menu_item(tr!("ade.tabs.close"))
            .icon(MaterialSymbol::CLOSE)
            .shortcut("Ctrl+W")
            .on_press(act(move |project| {
                project.viewer(|viewer, git| {
                    viewer.put_menu_away();
                    viewer.close(at, git);
                });
            })),
        closing(tr!("ade.tabs.close-others"), Closing::Others, count > 1),
        closing(tr!("ade.tabs.close-right"), Closing::Right, at + 1 < count),
        closing(tr!("ade.tabs.close-saved"), Closing::Saved, true),
        closing(tr!("ade.tabs.close-all"), Closing::All, true),
        menu_separator(),
        menu_item(tr!("ade.files.copy-path"))
            .icon(MaterialSymbol::LINK)
            .on_press(act(move |project| {
                rikka_ui::clipboard::set_text(&absolute);
                project.viewer(|viewer, _| viewer.put_menu_away());
            })),
        menu_item(tr!("ade.files.copy-relative-path"))
            .icon(MaterialSymbol::LINK)
            .on_press(act(move |project| {
                rikka_ui::clipboard::set_text(&relative);
                project.viewer(|viewer, _| viewer.put_menu_away());
            })),
        menu_item(tr!("ade.tabs.reveal"))
            .icon(MaterialSymbol::FOLDER_OPEN)
            .on_press(act(move |project| {
                project.viewer(|viewer, _| viewer.put_menu_away());
                reveal(project, &revealed);
            })),
    ];
    rikka_widgets::menu()
        .items(items)
        .width(240.0)
        .on_dismiss(act(|project| project.viewer(|viewer, _| viewer.dismiss())))
        .build()
        .keyed((MENU, menu.key))
        .anchored(Anchor::new(MARK, AnchorEdge::BesideTop).offset(menu.at.x, menu.at.y))
}

fn reveal(project: &mut Project, path: &Path) {
    let path = path.to_owned();
    project.turn_to(Tab::Files);
    project.explore(|explorer, mut parts| {
        parts.reveal(&path);
        explorer.choose(&path);
    });
}
