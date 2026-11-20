use std::time::{Duration, Instant};

use discord_core::Abilities;
use rikka_app::prelude::*;

use crate::chat::Chat;

const TICK: Duration = Duration::from_secs(1);
const TICKING: &str = "discord-conversation-tick";
const CLOCK_EVERY: u32 = 30;

impl Chat {
    pub(crate) fn start_ticking(&mut self) {
        every_keyed(TICKING, TICK, |chat: &mut Chat| chat.tick());
    }

    fn tick(&mut self) {
        let Some(seat) = self.conversation().shown().cloned() else {
            cancel(TICKING);
            return;
        };
        let (key, channel) = &seat;
        let now = Instant::now();
        let mut cooldown = 0;
        if let Some(linked) = self.linked_mut(key) {
            linked.change(|store| store.expire_typing(now));
            if let Some(channel_info) = linked.channel(channel)
                && channel_info.slowmode > 0
                && !channel_info.abilities.contains(Abilities::MANAGE_MESSAGES)
            {
                let slowmode = channel_info.slowmode;
                cooldown = self
                    .conversation()
                    .composer
                    .sent_at
                    .get(&seat)
                    .map_or(0, |at| {
                        let spent = u32::try_from(at.elapsed().as_secs()).unwrap_or(u32::MAX);
                        slowmode.saturating_sub(spent)
                    });
            }
        }
        let conversation = self.conversation_mut();
        conversation.composer.cooldown.set(cooldown);
        conversation.ticks = conversation.ticks.wrapping_add(1);
        if conversation.ticks.is_multiple_of(CLOCK_EVERY) {
            conversation.clock.changed();
        }
    }
}
