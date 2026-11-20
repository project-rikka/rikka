use discord_core::{ChannelId, ChannelKind, SpaceId};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::sidebar::kind_symbol;

const WIDTH: f32 = 560.0;
const ROW: f32 = 40.0;
const SHOWN: usize = 50;
const ROWS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Goal {
    Space(SpaceId),
    Channel(ChannelId),
}

#[derive(Clone, Debug)]
pub(crate) struct SwitcherHit {
    pub(crate) goal: Goal,
    symbol: MaterialSymbol,
    name: String,
    within: Option<String>,
    rank: (u8, usize),
}

#[derive(State)]
pub(crate) struct Switcher {
    open: Tracked<bool>,
    #[untracked]
    field: TextDocument,
    query: Tracked<String>,
    chosen: Tracked<usize>,
}

impl Switcher {
    pub(crate) fn new() -> Switcher {
        Switcher {
            open: Tracked::new(false),
            field: TextDocument::new(""),
            query: Tracked::new(String::new()),
            chosen: Tracked::new(0),
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        *self.open
    }

    pub(crate) fn query(&self) -> &str {
        &self.query
    }

    pub(crate) fn toggle(&mut self) {
        let open = !*self.open;
        self.field.clear();
        self.query.set(String::new());
        self.chosen.set(0);
        self.open.set(open);
    }

    pub(crate) fn close(&mut self) {
        self.open.set(false);
    }

    fn typed(&mut self) {
        let query = self.field.text().trim().to_lowercase();
        if query != *self.query {
            self.chosen.set(0);
        }
        self.query.set(query);
    }

    fn step(&mut self, by: isize, count: usize) {
        if count == 0 {
            return;
        }
        let next = (*self.chosen as isize + by).rem_euclid(count as isize) as usize;
        self.chosen.set(next);
    }
}

impl std::fmt::Debug for Switcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Switcher")
            .field("open", &*self.open)
            .field("query", &*self.query)
            .finish_non_exhaustive()
    }
}

pub(crate) fn search(linked: &LinkedAccount, query: &str) -> Vec<SwitcherHit> {
    let mut hits: Vec<SwitcherHit> = Vec::new();
    for item in linked.rail() {
        let spaces = match item {
            discord_core::RailItem::Space(space) => vec![space],
            discord_core::RailItem::Folder { spaces, .. } => spaces,
        };
        for space in spaces {
            if let Some(rank) = rank(&space.name, query, 2) {
                hits.push(SwitcherHit {
                    goal: Goal::Space(space.id),
                    symbol: MaterialSymbol::GROUPS,
                    name: space.name.clone(),
                    within: None,
                    rank,
                });
            }
            for section in linked.sections(&space.id) {
                for row in section.rows {
                    let channel = row.channel;
                    if !channel.kind.holds_messages() {
                        continue;
                    }
                    if let Some(rank) = rank(&channel.name, query, 1) {
                        hits.push(SwitcherHit {
                            goal: Goal::Channel(channel.id),
                            symbol: kind_symbol(channel.kind),
                            name: channel.name.clone(),
                            within: Some(space.name.clone()),
                            rank,
                        });
                    }
                }
            }
        }
    }
    for direct in linked.directs() {
        let title = linked.channel_title(direct);
        if let Some(rank) = rank(&title, query, 0) {
            hits.push(SwitcherHit {
                goal: Goal::Channel(direct.id),
                symbol: match direct.kind {
                    ChannelKind::Group => MaterialSymbol::GROUP,
                    _ => MaterialSymbol::ALTERNATE_EMAIL,
                },
                name: title,
                within: None,
                rank,
            });
        }
    }
    hits.sort_by_key(|hits| hits.rank);
    hits.truncate(SHOWN);
    hits
}

fn rank(name: &str, query: &str, kind: u8) -> Option<(u8, usize)> {
    if query.is_empty() {
        return Some((kind, 0));
    }
    let name = name.to_lowercase();
    let at = name.find(query)?;
    let starts = at == 0 || !name[..at].ends_with(char::is_alphanumeric);
    Some((u8::from(!starts) * 3 + kind, name.len()))
}

pub(crate) fn view(chat: &Chat) -> Option<Element> {
    let switcher = chat.switcher();
    if !switcher.is_open() {
        return None;
    }
    let linked = chat.current()?;
    let results = search(linked, switcher.query());
    let count = results.len();
    let chosen = (*switcher.chosen).min(count.saturating_sub(1));
    let goals: Vec<Goal> = results.iter().map(|hit| hit.goal.clone()).collect();
    let picked = goals.get(chosen).cloned();
    let list = lazy_column(count, move |slot| {
        result(&results[slot.index], slot.index == chosen)
    })
    .reveals(chosen)
    .height(ROW * ROWS.min(count.max(1)) as f32)
    .fill_width();
    Some(
        container(
            popover(
                column! {
                    text(tr!("discord.switcher.title")).label().semibold();
                    text_field(&switcher.field)
                        .placeholder(tr!("discord.switcher.placeholder"))
                        .autofocus(true)
                        .claims_focus(true)
                        .on_change(|switcher: &mut Switcher| switcher.typed())
                        .on_key(move |chat: &mut Chat, key: Key, _: Modifiers| match key {
                            Key::ArrowDown => {
                                chat.switcher_mut().step(1, count);
                                true
                            }
                            Key::ArrowUp => {
                                chat.switcher_mut().step(-1, count);
                                true
                            }
                            Key::Enter => {
                                if let Some(goal) = picked.clone() {
                                    chat.switch_to(&goal);
                                }
                                true
                            }
                            _ => false,
                        })
                        .fill_width();
                    if count == 0 {
                        text(tr!("discord.switcher.nothing")).caption().muted();
                    } else {
                        list;
                    }
                }
                .gap(Space::SNUG)
                .width(WIDTH),
            )
            .on_dismiss(|switcher: &mut Switcher| switcher.close()),
        )
        .align_x(Align::Center)
        .padding(Inset::edges(
            Space::WIDE,
            Space::NONE,
            Space::NONE,
            Space::NONE,
        ))
        .fill()
        .any(),
    )
}

fn result(hit: &SwitcherHit, chosen: bool) -> Element {
    let goal = hit.goal.clone();
    container(
        row! {
            icon(hit.symbol).extent(Length::px(18.0)).color(palette().text_muted);
            text(hit.name.clone()).label().medium().no_wrap();
            if let Some(within) = &hit.within {
                text(within.to_uppercase()).caption().muted().no_wrap();
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .background(match chosen {
        true => palette().accent.with_alpha(0.16),
        false => Color::TRANSPARENT,
    })
    .radius(Corner::Item)
    .on_press(move |chat: &mut Chat| chat.switch_to(&goal))
    .focusless()
    .height(ROW)
    .fill_width()
    .any()
}

impl Chat {
    pub(crate) fn switch_to(&mut self, goal: &Goal) {
        self.switcher_mut().close();
        match goal {
            Goal::Space(space) => self.open_space(space),
            Goal::Channel(channel) => self.open_channel(channel),
        }
    }
}
