use discord_core::{Abilities, Channel, ChannelKind, Emoji, Mention, Picture};

use crate::account::LinkedAccount;
use crate::conversation::emoji::catalogue;

const MOST: usize = 10;
const SHORTEST_EMOJI: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sigil {
    Member,
    Channel,
    Emoji,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Typed {
    pub(crate) sigil: Sigil,
    pub(crate) at: usize,
    pub(crate) end: usize,
    pub(crate) query: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Picked {
    Mention(Mention),
    Emoji(Emoji),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Look {
    Person(Option<Picture>),
    Role(Option<u32>),
    Everyone,
    Channel(ChannelKind),
    Glyph(&'static str),
    Custom(Picture),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Offer {
    pub(crate) label: String,
    pub(crate) detail: Option<String>,
    pub(crate) display: String,
    pub(crate) picked: Option<Picked>,
    pub(crate) look: Look,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Offers {
    pub(crate) typed: Typed,
    pub(crate) rows: Vec<Offer>,
    pub(crate) picked: usize,
}

impl Offers {
    pub(crate) fn step(&mut self, down: bool) {
        let count = self.rows.len().max(1);
        self.picked = match down {
            true => (self.picked + 1) % count,
            false => (self.picked + count - 1) % count,
        };
    }
}

pub(crate) fn typed_at(text: &str, caret: usize) -> Option<Typed> {
    let caret = caret.min(text.len());
    let before = text.get(..caret)?;
    let (at, sigil) = before
        .char_indices()
        .rev()
        .take_while(|(_, letter)| !letter.is_whitespace())
        .find_map(|(at, letter)| {
            let sigil = match letter {
                '@' => Sigil::Member,
                '#' => Sigil::Channel,
                ':' => Sigil::Emoji,
                _ => return None,
            };
            Some((at, sigil))
        })?;
    let opens = before[..at]
        .chars()
        .next_back()
        .is_none_or(char::is_whitespace);
    if !opens {
        return None;
    }
    let query = before[at + 1..].to_owned();
    if sigil == Sigil::Emoji && query.chars().count() < SHORTEST_EMOJI {
        return None;
    }
    let end = text[caret..]
        .find(char::is_whitespace)
        .map_or(text.len(), |offset| caret + offset);
    Some(Typed {
        sigil,
        at,
        end,
        query,
    })
}

pub(crate) fn offers(linked: &LinkedAccount, channel: &Channel, typed: &Typed) -> Vec<Offer> {
    let query = typed.query.to_lowercase();
    match typed.sigil {
        Sigil::Member => people(linked, channel, &query),
        Sigil::Channel => channels(linked, channel, &query),
        Sigil::Emoji => emoji(linked, channel, &query),
    }
}

fn matches(name: &str, query: &str) -> Option<u8> {
    let lowered;
    let name = match name.chars().any(char::is_uppercase) {
        true => {
            lowered = name.to_lowercase();
            lowered.as_str()
        }
        false => name,
    };
    match () {
        _ if query.is_empty() => Some(1),
        _ if name.starts_with(query) => Some(0),
        _ if name.contains(query) => Some(1),
        _ => None,
    }
}

enum Candidate<'a> {
    Person(&'a discord_core::User, String),
    Role(&'a discord_core::Role),
    Word(&'static str, Mention),
}

fn people(linked: &LinkedAccount, channel: &Channel, query: &str) -> Vec<Offer> {
    let space = channel.space.as_ref();
    let mut ranked: Vec<(u8, String, Candidate<'_>)> = Vec::new();
    let users: Vec<&discord_core::UserId> = match space {
        Some(space) => linked
            .members(space)
            .into_iter()
            .map(|member| &member.user)
            .collect(),
        None => channel
            .recipients
            .iter()
            .chain(linked.me().map(|me| &me.id))
            .collect(),
    };
    for id in users {
        let Some(user) = linked.user(id) else {
            continue;
        };
        let name = linked.display_name(space, user);
        let lowered = name.to_lowercase();
        let rank = matches(&lowered, query).or_else(|| matches(&user.handle, query));
        if let Some(rank) = rank {
            ranked.push((rank, lowered, Candidate::Person(user, name)));
        }
    }
    if let Some(space) = space {
        for role in linked.roles(space) {
            if role.id.get() == space.get() {
                continue;
            }
            let lowered = role.name.to_lowercase();
            if let Some(rank) = matches(&lowered, query) {
                ranked.push((rank, lowered, Candidate::Role(role)));
            }
        }
    }
    if channel.abilities.contains(Abilities::MENTION_EVERYONE) {
        for (word, mention) in [("everyone", Mention::Everyone), ("here", Mention::Here)] {
            if let Some(rank) = matches(word, query) {
                ranked.push((rank, format!("@{word}"), Candidate::Word(word, mention)));
            }
        }
    }
    let order = |left: &(u8, String, Candidate<'_>), right: &(u8, String, Candidate<'_>)| {
        left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1))
    };
    if ranked.len() > MOST {
        ranked.select_nth_unstable_by(MOST - 1, order);
        ranked.truncate(MOST);
    }
    ranked.sort_unstable_by(order);
    ranked
        .into_iter()
        .map(|(_, _, candidate)| match candidate {
            Candidate::Person(user, name) => Offer {
                display: format!("@{name}"),
                label: name,
                detail: Some(user.handle.clone()),
                picked: Some(Picked::Mention(Mention::User(user.id))),
                look: Look::Person(user.avatar.clone()),
            },
            Candidate::Role(role) => Offer {
                display: format!("@{}", role.name),
                label: role.name.clone(),
                detail: None,
                picked: Some(Picked::Mention(Mention::Role(role.id))),
                look: Look::Role(role.color),
            },
            Candidate::Word(word, mention) => Offer {
                display: format!("@{word}"),
                label: format!("@{word}"),
                detail: None,
                picked: Some(Picked::Mention(mention)),
                look: Look::Everyone,
            },
        })
        .collect()
}

fn channels(linked: &LinkedAccount, channel: &Channel, query: &str) -> Vec<Offer> {
    let Some(space) = channel.space.as_ref() else {
        return Vec::new();
    };
    let mut ranked: Vec<(u8, Offer)> = linked
        .sections(space)
        .iter()
        .flat_map(|section| section.rows.iter())
        .map(|row| row.channel)
        .filter(|channel_info| channel_info.kind != ChannelKind::Category)
        .filter_map(|channel_info| {
            let rank = matches(&channel_info.name, query)?;
            Some((
                rank,
                Offer {
                    display: format!("#{}", channel_info.name),
                    label: channel_info.name.clone(),
                    detail: None,
                    picked: Some(Picked::Mention(Mention::Channel(channel_info.id))),
                    look: Look::Channel(channel_info.kind),
                },
            ))
        })
        .collect();
    ranked.sort_by_key(|(rank, _)| *rank);
    ranked
        .into_iter()
        .take(MOST)
        .map(|(_, offer)| offer)
        .collect()
}

fn emoji(linked: &LinkedAccount, channel: &Channel, query: &str) -> Vec<Offer> {
    let foreign = channel.abilities.contains(Abilities::EXTERNAL_EMOJI);
    let mut offers: Vec<Offer> = linked
        .emoji_spaces()
        .into_iter()
        .filter(|(space, _)| foreign || channel.space.as_ref() == Some(&space.id))
        .flat_map(|(space, emojis)| {
            emojis
                .iter()
                .filter(move |emoji| matches(&emoji.name, query) == Some(0))
                .map(move |emoji| (space.name.clone(), emoji.clone()))
        })
        .take(MOST / 2)
        .filter_map(|(space, emoji)| {
            let picture = emoji.picture.clone()?;
            Some(Offer {
                display: format!(":{}:", emoji.name),
                label: format!(":{}:", emoji.name),
                detail: Some(space),
                picked: Some(Picked::Emoji(emoji)),
                look: Look::Custom(picture),
            })
        })
        .collect();
    let left = MOST.saturating_sub(offers.len());
    offers.extend(
        catalogue::search(query, left)
            .into_iter()
            .map(|glyph| Offer {
                display: glyph.text.to_owned(),
                label: format!(":{}:", glyph.code),
                detail: None,
                picked: None,
                look: Look::Glyph(glyph.text),
            }),
    );
    offers
}
