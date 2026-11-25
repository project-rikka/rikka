use std::rc::Rc;

use discord_core::{Folder, RailItem, Space as ChatSpace, SpaceId, Unseen};
use rikka_app::prelude::*;
use rikka_ui::{Handler, hash_of};
use rikka_widgets::{column, row};

use crate::account::{LinkedAccount, Scope};
use crate::brand;
use crate::chat::Chat;
use crate::menus::Subject;
use crate::widgets::{Pointing, badge, context_area, mark_of, picture_tile, pointing};

pub(crate) const WIDTH: f32 = 72.0;
const TILE: f32 = 48.0;
const PITCH: f32 = TILE + Space::SNUG.px();
const MINI: f32 = 18.0;
const ROUND: f32 = TILE / 2.0;
const SQUARE: f32 = 16.0;
const PILL: f32 = 4.0;
const BAND: f32 = 0.16;
const FOLDING: Motion = Motion::Standard;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Target {
    Home,
    Space(SpaceId),
    Folder(String),
    Add,
}

impl Target {
    fn go(&self, chat: &mut Chat) {
        match self {
            Target::Home => chat.open_home(),
            Target::Space(space) => chat.open_space(space),
            Target::Folder(folder) => chat.toggle_folder(folder),
            Target::Add => chat.open_space_sheet(),
        }
    }

    fn subject(&self) -> Option<Subject> {
        match self {
            Target::Space(space) => Some(Subject::Space(*space)),
            Target::Folder(folder) => Some(Subject::Folder(folder.clone())),
            Target::Home | Target::Add => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Look {
    selected: bool,
    unseen: Unseen,
    dimmed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Band {
    Head,
    Inside,
    Foot,
}

#[derive(Debug)]
enum Line {
    Space {
        space: ChatSpace,
        look: Look,
        band: Option<(Band, Color)>,
    },
    Folder {
        id: String,
        name: String,
        tint: Color,
        look: Look,
        minis: Vec<ChatSpace>,
        open: bool,
    },
}

impl Line {
    fn target(&self) -> Target {
        match self {
            Line::Space { space, .. } => Target::Space(space.id),
            Line::Folder { id, .. } => Target::Folder(id.clone()),
        }
    }
}

pub(crate) fn view(chat: &Chat) -> Element {
    let Some(linked) = chat.current() else {
        return spacer().width(WIDTH).any();
    };
    let scope = linked.location().scope.clone();
    let home = Look {
        selected: scope == Scope::Home,
        unseen: linked.directs_unseen(),
        dimmed: false,
    };
    let lines = Rc::new(lines(linked, &scope));
    let named = Rc::clone(&lines);
    let spaces = lazy_column(lines.len(), move |slot| line_view(&lines[slot.index]))
        .uniform(PITCH)
        .named(move |index| named.get(index).map_or(0, |line| hash_of(line.target())))
        .shows(linked.key())
        .grow(1.0)
        .fill_width();
    column! {
        tile(
            Target::Home,
            container(icon(brand::mark()).extent(Length::px(26.0)).color(match home.selected {
                true => Color::WHITE,
                false => palette().text,
            }))
            .center()
            .background(match home.selected {
                true => brand::blurple(),
                false => palette().surface,
            })
            .size(Length::px(TILE), Length::px(TILE))
            .any(),
            tr!("discord.rail.home").to_string(),
            home,
        );
        container(spacer())
            .background(palette().outline)
            .radius(1.0)
            .size(Length::px(32.0), Length::px(2.0));
        spaces;
        tile(
            Target::Add,
            container(icon(MaterialSymbol::ADD).extent(Length::px(24.0)).color(palette().success))
                .center()
                .background(palette().surface)
                .size(Length::px(TILE), Length::px(TILE))
                .any(),
            tr!("discord.rail.add-server").to_string(),
            Look::default(),
        );
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::NONE, Space::NORMAL))
    .width(WIDTH)
    .fill_height()
    .background(palette().background)
    .any()
}

fn lines(linked: &LinkedAccount, scope: &Scope) -> Vec<Line> {
    let mut lines = Vec::new();
    for item in linked.rail() {
        match item {
            RailItem::Space(space) => lines.push(space_line(linked, space, scope, None)),
            RailItem::Folder { folder, spaces } => {
                folder_lines(&mut lines, linked, folder, &spaces, scope);
            }
        }
    }
    lines
}

fn space_line(
    linked: &LinkedAccount,
    space: &ChatSpace,
    scope: &Scope,
    band: Option<(Band, Color)>,
) -> Line {
    Line::Space {
        space: space.clone(),
        look: Look {
            selected: scope.space() == Some(&space.id),
            unseen: linked.space_unseen(&space.id),
            dimmed: space.unavailable,
        },
        band,
    }
}

fn folder_lines(
    lines: &mut Vec<Line>,
    linked: &LinkedAccount,
    folder: &Folder,
    spaces: &[&ChatSpace],
    scope: &Scope,
) {
    let id = folder.id.clone().unwrap_or_default();
    let tint = folder.color.map_or_else(|| palette().accent, Color::hex);
    let open = linked.is_unfolded(&id);
    let unseen = spaces
        .iter()
        .map(|space| linked.space_unseen(&space.id))
        .fold(Unseen::default(), |gathered, space_unseen| Unseen {
            unread: gathered.unread || space_unseen.unread,
            mentions: gathered.mentions + space_unseen.mentions,
        });
    let name = folder.name.clone().unwrap_or_else(|| {
        spaces
            .iter()
            .map(|space| space.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    });
    let holds_here = spaces.iter().any(|space| scope.space() == Some(&space.id));
    let look = match open {
        true => Look::default(),
        false => Look {
            selected: holds_here,
            unseen,
            dimmed: false,
        },
    };
    lines.push(Line::Folder {
        id,
        name,
        tint,
        look,
        minis: spaces
            .iter()
            .take(4)
            .map(|space| (*space).clone())
            .collect(),
        open,
    });
    if !open {
        return;
    }
    let last = spaces.len() - 1;
    lines.extend(spaces.iter().enumerate().map(|(index, space)| {
        let band = match index == last {
            true => Band::Foot,
            false => Band::Inside,
        };
        space_line(linked, space, scope, Some((band, tint)))
    }));
}

fn line_view(line: &Line) -> Element {
    match line {
        Line::Space { space, look, band } => {
            let face = picture_tile(space.icon.as_ref(), &space.name, TILE, ROUND);
            let tile = tile(Target::Space(space.id), face, space.name.clone(), *look);
            banded(band.map(|(band, tint)| band_view(band, tint, 1.0)), tile)
        }
        Line::Folder {
            id,
            name,
            tint,
            look,
            minis,
            open,
        } => {
            let shown = |visible: bool| match visible {
                true => 1.0,
                false => 0.0,
            };
            let band = band_view(Band::Head, *tint, shown(*open));
            let grid = container(
                grid_of(
                    2,
                    minis.iter().map(|space| {
                        picture_tile(space.icon.as_ref(), &space.name, MINI, MINI / 2.0)
                    }),
                )
                .gap(Space::HAIR),
            )
            .center()
            .size(Length::px(TILE), Length::px(TILE))
            .build()
            .opacity(shown(!*open))
            .animate(FOLDING);
            let mark = container(
                icon(MaterialSymbol::FOLDER)
                    .extent(Length::px(24.0))
                    .color(*tint)
                    .filled(),
            )
            .center()
            .size(Length::px(TILE), Length::px(TILE))
            .build()
            .opacity(shown(*open))
            .animate(FOLDING)
            .position(0.0, 0.0);
            let face = container(column_of([mark, grid]))
                .background(tint.with_alpha(0.4 * shown(!*open)))
                .size(Length::px(TILE), Length::px(TILE))
                .build()
                .animate(FOLDING);
            banded(
                Some(band),
                tile(Target::Folder(id.clone()), face, name.clone(), *look),
            )
        }
    }
}

fn banded(band: Option<Element>, tile: Element) -> Element {
    column_of(
        band.into_iter()
            .chain([tile, spacer().height(Space::SNUG).any()]),
    )
    .fill_width()
    .any()
}

fn band_view(band: Band, tint: Color, shown: f32) -> Element {
    let (height, radii) = match band {
        Band::Head => (PITCH, CornerRadii::new(SQUARE, SQUARE, 0.0, 0.0)),
        Band::Inside => (PITCH, CornerRadii::new(0.0, 0.0, 0.0, 0.0)),
        Band::Foot => (TILE, CornerRadii::new(0.0, 0.0, SQUARE, SQUARE)),
    };
    container(
        container(spacer())
            .background(tint.with_alpha(BAND * shown))
            .radius(radii)
            .size(Length::px(TILE), Length::px(height))
            .build()
            .animate(FOLDING),
    )
    .center()
    .fill_width()
    .height(height)
    .position(0.0, 0.0)
    .any()
}

fn tile(target: Target, face: Element, name: String, look: Look) -> Element {
    let subject = target.subject();
    let anchor = mark_of(("discord-rail", &target));
    let tile = local_state(
        Pointing::Away,
        move |pointed: Pointing, set: Setter<Pointing>| {
            let hovered = pointed.is_over();
            let pill = match () {
                _ if look.selected => 40.0,
                _ if hovered => 20.0,
                _ if look.unseen.unread => 8.0,
                _ => 0.0,
            };
            let radius = match look.selected || hovered {
                true => SQUARE,
                false => ROUND,
            };
            let mut shaped = container(face)
                .radius(radius)
                .clip_content()
                .size(Length::px(TILE), Length::px(TILE))
                .any();
            if look.dimmed {
                shaped = shaped.opacity(0.4).any();
            }
            let mut layers = vec![
                row! {
                    container(spacer())
                        .background(palette().text)
                        .radius(CornerRadii::new(0.0, PILL, PILL, 0.0))
                        .size(Length::px(PILL), Length::px(pill));
                    spacer().grow(1.0);
                    shaped;
                    spacer().grow(1.0);
                    spacer().width(PILL);
                }
                .align_items(Align::Center)
                .height(TILE)
                .fill_width()
                .build()
                .keyed(anchor),
            ];
            if look.unseen.mentions > 0 {
                layers.push(
                    container(badge(look.unseen.mentions))
                        .border(3.0, palette().background)
                        .radius(11.0)
                        .position(WIDTH / 2.0 + 6.0, TILE - 18.0)
                        .any(),
                );
            }
            if pointed == Pointing::Hovered {
                layers.push(
                    container(text(name.clone()).label().semibold().no_wrap())
                        .padding(Inset::xy(Space::NORMAL, Space::SNUG))
                        .background(palette().surface)
                        .border(1.0, palette().outline)
                        .radius(Corner::Control)
                        .build()
                        .anchored(Anchor::beside(anchor).offset(-Space::SNUG.px(), 8.0)),
                );
            }
            let target = target.clone();
            pointing(
                container(column_of(layers).fill_width())
                    .on_press(move |chat: &mut Chat| target.go(chat))
                    .focusless()
                    .fill_width(),
                move |pointed: Pointing| {
                    set(pointed);
                    skip_view();
                },
            )
        },
    );
    match subject {
        Some(subject) => context_area(
            mark_of(("discord-rail-menu", &subject)),
            tile,
            Handler::new(move |chat: &mut Chat, anchor: Anchor| {
                chat.open_context_menu(subject.clone(), anchor);
            }),
        ),
        None => tile,
    }
}
