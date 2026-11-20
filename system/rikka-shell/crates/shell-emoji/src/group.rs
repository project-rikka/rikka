use rikka_emoji::Group;
use rikka_ui::{Label, MaterialSymbol, tr};

#[must_use]
pub(crate) fn label(group: Group) -> Label {
    match group {
        Group::Smileys => tr!("shell.emoji.group.smileys"),
        Group::People => tr!("shell.emoji.group.people"),
        Group::Nature => tr!("shell.emoji.group.nature"),
        Group::Food => tr!("shell.emoji.group.food"),
        Group::Travel => tr!("shell.emoji.group.travel"),
        Group::Activities => tr!("shell.emoji.group.activities"),
        Group::Objects => tr!("shell.emoji.group.objects"),
        Group::Symbols => tr!("shell.emoji.group.symbols"),
        Group::Flags => tr!("shell.emoji.group.flags"),
    }
}

#[must_use]
pub(crate) fn symbol(group: Group) -> MaterialSymbol {
    match group {
        Group::Smileys => MaterialSymbol::MOOD,
        Group::People => MaterialSymbol::EMOJI_PEOPLE,
        Group::Nature => MaterialSymbol::EMOJI_NATURE,
        Group::Food => MaterialSymbol::EMOJI_FOOD_BEVERAGE,
        Group::Travel => MaterialSymbol::EMOJI_TRANSPORTATION,
        Group::Activities => MaterialSymbol::EMOJI_EVENTS,
        Group::Objects => MaterialSymbol::EMOJI_OBJECTS,
        Group::Symbols => MaterialSymbol::EMOJI_SYMBOLS,
        Group::Flags => MaterialSymbol::EMOJI_FLAGS,
    }
}
