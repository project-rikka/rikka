use discord_core::{MuteSpan, NotifyLevel};
use rikka_app::prelude::*;

use crate::chat::Chat;
use crate::conversation::time::sent_at;

pub(super) fn span_label(span: MuteSpan) -> Label {
    match span {
        MuteSpan::Minutes15 => tr!("discord.menu.mute-15m"),
        MuteSpan::Hour => tr!("discord.menu.mute-1h"),
        MuteSpan::Hours3 => tr!("discord.menu.mute-3h"),
        MuteSpan::Hours8 => tr!("discord.menu.mute-8h"),
        MuteSpan::Day => tr!("discord.menu.mute-24h"),
        MuteSpan::Forever => tr!("discord.menu.mute-forever"),
    }
}

pub(crate) fn level_label(level: NotifyLevel) -> Label {
    match level {
        NotifyLevel::All => tr!("discord.menu.level-all"),
        NotifyLevel::Mentions => tr!("discord.menu.level-mentions"),
        NotifyLevel::Nothing => tr!("discord.menu.level-nothing"),
    }
}

pub(super) fn mute_items<F>(
    muted: bool,
    ends: Option<i64>,
    mute: Label,
    unmute: Label,
    act: F,
) -> Vec<MenuItem>
where
    F: Fn(&mut Chat, Option<MuteSpan>) + Clone + 'static,
{
    if muted {
        let mut items = vec![
            menu_item(unmute)
                .icon(MaterialSymbol::NOTIFICATIONS_ACTIVE)
                .on_press(move |chat: &mut Chat| act(chat, None)),
        ];
        if let Some(ends) = ends {
            items.push(menu_header(tr!(
                "discord.menu.muted-until",
                time = sent_at(ends).as_str()
            )));
        }
        return items;
    }
    let spans = MuteSpan::ALL.map(|span| {
        let act = act.clone();
        menu_item(span_label(span)).on_press(move |chat: &mut Chat| act(chat, Some(span)))
    });
    vec![
        menu_item(mute)
            .icon(MaterialSymbol::NOTIFICATIONS_OFF)
            .items(spans),
    ]
}
