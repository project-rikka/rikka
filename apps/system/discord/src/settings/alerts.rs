use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::chat::Chat;
use crate::preferences::Alerts;

pub(super) fn view(chat: &Chat) -> Element {
    let alerts = chat.alerts();
    section(
        tr!("discord.settings.desktop-notifications"),
        column! {
            toggle(tr!("discord.settings.notify"), alerts.enabled, true, |alerts, on| alerts.enabled = on);
            toggle(tr!("discord.settings.notify-directs"), alerts.directs, alerts.enabled, |alerts, on| {
                alerts.directs = on;
            });
            toggle(tr!("discord.settings.notify-mentions"), alerts.mentions, alerts.enabled, |alerts, on| {
                alerts.mentions = on;
            });
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
    .any()
}

fn toggle(label: Label, on: bool, enabled: bool, set: fn(&mut Alerts, bool)) -> impl View {
    field(
        label,
        switch(on)
            .enabled(enabled)
            .on_toggle(move |chat: &mut Chat, on: bool| {
                let mut alerts = chat.alerts();
                set(&mut alerts, on);
                chat.set_alerts(alerts);
            }),
    )
}
