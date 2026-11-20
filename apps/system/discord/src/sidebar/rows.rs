use discord_core::{ChannelKind, Unseen};
use rikka_app::prelude::*;

use crate::widgets::badge;

pub(crate) const ROW: f32 = 34.0;
const DOT: f32 = 8.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Look {
    pub(crate) selected: bool,
    pub(crate) unseen: Unseen,
    pub(crate) muted: bool,
}

impl Look {
    pub(crate) fn ink(self) -> Color {
        let palette = palette();
        match () {
            _ if self.selected => palette.text,
            _ if self.muted => palette.text_muted.with_alpha(0.5),
            _ if self.unseen.unread || self.unseen.mentions > 0 => palette.text,
            _ => palette.text_muted,
        }
    }

    pub(crate) fn bold(self) -> bool {
        !self.muted && (self.unseen.unread || self.unseen.mentions > 0)
    }

    pub(crate) fn fill(self) -> Color {
        match self.selected {
            true => palette().text.with_alpha(0.1),
            false => Color::TRANSPARENT,
        }
    }
}

pub(crate) fn kind_symbol(kind: ChannelKind) -> MaterialSymbol {
    match kind {
        ChannelKind::Text | ChannelKind::Category => MaterialSymbol::TAG,
        ChannelKind::Announcement => MaterialSymbol::CAMPAIGN,
        ChannelKind::Voice => MaterialSymbol::VOLUME_UP,
        ChannelKind::Stage => MaterialSymbol::PODCASTS,
        ChannelKind::Forum => MaterialSymbol::FORUM,
        ChannelKind::Thread => MaterialSymbol::SUBDIRECTORY_ARROW_RIGHT,
        ChannelKind::Direct => MaterialSymbol::PERSON,
        ChannelKind::Group => MaterialSymbol::GROUP,
    }
}

pub(crate) fn unread_dot(look: Look) -> Element {
    let shown = !look.selected && look.bold();
    container(spacer())
        .background(match shown {
            true => palette().text,
            false => Color::TRANSPARENT,
        })
        .radius(DOT / 2.0)
        .size(Length::px(DOT / 2.0), Length::px(DOT))
        .any()
}

pub(crate) fn trailing_badge(look: Look) -> Option<Element> {
    (look.unseen.mentions > 0).then(|| badge(look.unseen.mentions).any())
}
