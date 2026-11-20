use rikka_app::prelude::*;
use rikka_emoji::{Catalog, Group};

pub(crate) struct Found {
    pub(crate) text: &'static str,
    pub(crate) code: String,
}

pub(crate) fn search(query: &str, most: usize) -> Vec<Found> {
    let catalogue = Catalog::current();
    catalogue
        .search(query)
        .into_iter()
        .take(most)
        .filter_map(|index| catalogue.get(index))
        .map(|emoji| Found {
            text: emoji.text,
            code: emoji.code.clone(),
        })
        .collect()
}

pub(crate) fn in_group(group: Group) -> Vec<&'static str> {
    let catalogue = Catalog::current();
    catalogue
        .group_indices(group)
        .into_iter()
        .filter_map(|index| catalogue.get(index))
        .map(|emoji| emoji.text)
        .collect()
}

pub(crate) fn symbol(group: Group) -> MaterialSymbol {
    match group {
        Group::Smileys => MaterialSymbol::MOOD,
        Group::People => MaterialSymbol::PEOPLE,
        Group::Nature => MaterialSymbol::PETS,
        Group::Food => MaterialSymbol::RESTAURANT,
        Group::Activities => MaterialSymbol::SPORTS_ESPORTS,
        Group::Travel => MaterialSymbol::DIRECTIONS_CAR,
        Group::Objects => MaterialSymbol::LIGHTBULB,
        Group::Symbols => MaterialSymbol::EMOJI_SYMBOLS,
        Group::Flags => MaterialSymbol::FLAG,
    }
}

pub(crate) fn label(group: Group) -> Label {
    match group {
        Group::Smileys => tr!("discord.conversation.emoji.smileys"),
        Group::People => tr!("discord.conversation.emoji.people"),
        Group::Nature => tr!("discord.conversation.emoji.nature"),
        Group::Food => tr!("discord.conversation.emoji.food"),
        Group::Activities => tr!("discord.conversation.emoji.activities"),
        Group::Travel => tr!("discord.conversation.emoji.travel"),
        Group::Objects => tr!("discord.conversation.emoji.objects"),
        Group::Symbols => tr!("discord.conversation.emoji.symbols"),
        Group::Flags => tr!("discord.conversation.emoji.flags"),
    }
}
