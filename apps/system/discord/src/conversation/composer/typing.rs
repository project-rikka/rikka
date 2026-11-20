use std::time::{Duration, Instant};

use discord_core::ChannelId;
use rikka_app::prelude::*;
use rikka_ui::{Element, Layout};
use rikka_widgets::row;

use crate::account::LinkedAccount;

const DOT: f32 = 5.0;
const BEAT: Duration = Duration::from_millis(120);
const ROUND: f32 = 1.2;

pub(crate) fn line(linked: &LinkedAccount, channel: &ChannelId) -> Option<Element> {
    let space = linked
        .channel(channel)
        .and_then(|channel_info| channel_info.space);
    let names: Vec<String> = linked
        .typers(channel, Instant::now())
        .into_iter()
        .map(|user| {
            linked
                .name_of(space.as_ref(), user)
                .unwrap_or_else(|| user.to_string())
        })
        .collect();
    let caption = match names.as_slice() {
        [] => return None,
        [one] => tr!("discord.conversation.typing.one", name = one.as_str()),
        [first, second] => tr!(
            "discord.conversation.typing.two",
            first = first.as_str(),
            second = second.as_str()
        ),
        [first, second, third] => tr!(
            "discord.conversation.typing.three",
            first = first.as_str(),
            second = second.as_str(),
            third = third.as_str()
        ),
        _ => tr!("discord.conversation.typing.several"),
    };
    Some(
        row! {
            Element::new(Dots).size(Length::px(DOT * 5.0), Length::px(DOT * 2.0));
            text(caption).caption().semibold().no_wrap().min_width(0.0);
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .any(),
    )
}

struct Dots;

impl Widget for Dots {
    type State = Option<Instant>;

    fn layout(&self) -> Layout {
        Layout::DEFAULT
    }

    fn paint(&self, began: &mut Option<Instant>, ctx: &mut PaintCtx<'_>) {
        let now = ctx.now();
        let since = now.saturating_duration_since(*began.get_or_insert(now));
        let phase = (since.as_secs_f32() / ROUND).fract() * 3.0;
        let bounds = ctx.bounds();
        let ink = ctx.palette().text;
        for dot in 0..3 {
            let lit = 1.0 - ((phase - dot as f32).abs().min(1.0));
            let center = Point::new(
                bounds.x() + DOT * (0.5 + 2.0 * dot as f32),
                bounds.center().y,
            );
            let rect = Rect::from_circle(center, DOT / 2.0);
            ctx.draw_quad(
                Quad::new(rect)
                    .fill(ink.with_alpha(0.35 + 0.65 * lit))
                    .radius(DOT / 2.0),
            );
        }
        ctx.wake_at(now + BEAT);
    }
}
