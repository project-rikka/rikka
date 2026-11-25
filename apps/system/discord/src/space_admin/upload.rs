use discord_core::{SoundId, StickerId};

use crate::manage::Picked;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Upload {
    Sticker(Picked),
    Sound { picked: Picked, volume: f32 },
    EditSticker(StickerId),
    EditSound { id: SoundId, volume: f32 },
}
