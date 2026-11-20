use crate::effect::builtin::{BuiltinEffect, single};
use crate::effect::desc::Effect;

impl Effect {
    pub(crate) fn color_lookup() -> Self {
        single(BuiltinEffect::ColorLookup, "grade", |step| step)
    }

    pub(crate) fn channel_mixer() -> Self {
        single(BuiltinEffect::ChannelMixer, "mix", |step| step)
    }

    pub(crate) fn hsl() -> Self {
        single(BuiltinEffect::Hsl, "adjust", |step| step)
    }

    pub(crate) fn split_toning() -> Self {
        single(BuiltinEffect::SplitToning, "tone", |step| step)
    }

    pub(crate) fn posterize() -> Self {
        single(BuiltinEffect::Posterize, "quantize", |step| step)
    }

    pub(crate) fn color_key() -> Self {
        single(BuiltinEffect::ColorKey, "key", |step| step)
    }

    pub(crate) fn cdl() -> Self {
        single(BuiltinEffect::Cdl, "decide", |step| step)
    }

    pub(crate) fn white_balance() -> Self {
        single(BuiltinEffect::WhiteBalance, "adapt", |step| step)
    }
}
