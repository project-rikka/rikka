use cut_project::{Channel, Clip, ClipId, Property};
use rikka_app::prelude::*;
use rikka_widgets::row;

use super::diamond::{Diamond, DiamondTarget, Mark};
use crate::command::Command;
use crate::controls::tool;
use crate::cut::Cut;

const GLYPH: f32 = 18.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Keying {
    clip: ClipId,
    local: Option<i64>,
}

impl Mark {
    fn from_channels<'a>(
        channels: impl IntoIterator<Item = Channel<'a>>,
        local: Option<i64>,
    ) -> Mark {
        channels.into_iter().fold(Mark::Still, |mark, channel| {
            match (mark, local.is_some_and(|at| channel.has_key(at))) {
                (Mark::Here, _) | (_, true) => Mark::Here,
                (Mark::Animated | Mark::Still, false) => Mark::Animated,
            }
        })
    }
}

impl Keying {
    pub(crate) fn for_clip(clip: &Clip, frame: i64) -> Keying {
        Keying {
            clip: clip.id(),
            local: clip.covers(frame).then(|| clip.local(frame)),
        }
    }

    fn diamond_of(&self, keyed: DiamondTarget, mark: Mark) -> Element {
        Diamond {
            clip: self.clip,
            keyed,
            mark,
            reachable: self.local.is_some(),
        }
        .build()
    }

    pub(crate) fn diamond(&self, property: Property, channel: Option<Channel<'_>>) -> Element {
        let mark = Mark::from_channels(channel, self.local);
        self.diamond_of(DiamondTarget::Property(property), mark)
    }

    pub(crate) fn on(&self, clip: &Clip, property: Property) -> Element {
        let channel = clip.channel(&property);
        self.diamond(property, channel)
    }

    pub(crate) fn group(&self, clip: &Clip) -> Element {
        let grouped = clip.grouped();
        let mark = Mark::from_channels(
            grouped.iter().filter_map(|property| clip.channel(property)),
            self.local,
        );
        self.diamond_of(DiamondTarget::Group, mark)
    }
}

pub(crate) fn navigation(cut: &Cut) -> Element {
    row! {
        text(tr!("cut.panel.keyframes")).label().semibold().grow(1.0);
        tool(MaterialSymbol::CHEVRON_LEFT, GLYPH, Command::PreviousKey, cut);
        tool(MaterialSymbol::CHEVRON_RIGHT, GLYPH, Command::NextKey, cut);
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width()
    .build()
}
