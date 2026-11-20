use super::key::{Handle, Incoming, Interpolation, Outgoing, Preset, Side, Smooth, Timing};
use super::kind::Kind;
use super::track::Track;

impl<K: Kind> Track<K> {
    pub(crate) fn interpolate(&mut self, at: i64, interpolation: Interpolation) -> bool {
        let Ok(index) = self.find(at) else {
            return false;
        };
        let incoming = index
            .checked_sub(1)
            .and_then(|before| self.effective(before))
            .map(|[_, into]| into);
        let outgoing = self.effective(index).map(|[out, _]| out);
        let (incoming, outgoing) = match (incoming, outgoing) {
            (Some(incoming), Some(outgoing)) => (incoming, outgoing),
            (Some(only), None) | (None, Some(only)) => (only, only),
            (None, None) => (Handle::EASED, Handle::EASED),
        };
        let timing = self.keys()[index].timing;
        let bezier = |smooth: Smooth| Timing {
            incoming: Incoming::Bezier(incoming),
            outgoing: Outgoing::Bezier(outgoing),
            smooth,
        };
        let timing = match interpolation {
            Interpolation::Linear => Timing::LINEAR,
            Interpolation::Bezier => bezier(Smooth::Manual),
            Interpolation::Auto => bezier(Smooth::Auto),
            Interpolation::Continuous => {
                let speed = (incoming.speed + outgoing.speed) / 2.0;
                Timing {
                    incoming: Incoming::Bezier(Handle { speed, ..incoming }),
                    outgoing: Outgoing::Bezier(Handle { speed, ..outgoing }),
                    smooth: Smooth::Continuous,
                }
            }
            Interpolation::Hold => Timing {
                outgoing: Outgoing::Hold,
                ..timing
            },
            Interpolation::EasyEase => Timing {
                incoming: Incoming::Bezier(Handle::EASED),
                outgoing: Outgoing::Bezier(Handle::EASED),
                smooth: Smooth::Continuous,
            },
            Interpolation::EasyEaseIn => Timing {
                incoming: Incoming::Bezier(Handle::EASED),
                outgoing: timing.outgoing,
                smooth: Smooth::Manual,
            },
            Interpolation::EasyEaseOut => Timing {
                incoming: timing.incoming,
                outgoing: Outgoing::Bezier(Handle::EASED),
                smooth: Smooth::Manual,
            },
            Interpolation::Preset(easing) => Timing {
                outgoing: Outgoing::Preset(Preset::whole(easing)),
                ..timing
            },
        };
        self.change(at, |key| key.timing = timing)
    }

    pub(crate) fn smooth(&mut self, at: i64, smooth: Smooth) -> bool {
        match smooth {
            Smooth::Manual => self.change(at, |key| key.timing.smooth = Smooth::Manual),
            Smooth::Continuous => self.interpolate(at, Interpolation::Continuous),
            Smooth::Auto => self.interpolate(at, Interpolation::Auto),
        }
    }

    pub(crate) fn steer(&mut self, at: i64, side: Side, handle: Handle) -> bool {
        let Ok(index) = self.find(at) else {
            return false;
        };
        let opposite = match side {
            Side::Outgoing => self
                .keys()
                .get(index + 1)
                .and_then(|next| next.timing.handle(Side::Incoming)),
            Side::Incoming => index
                .checked_sub(1)
                .and_then(|before| self.keys()[before].timing.handle(Side::Outgoing)),
        };
        let room = opposite.map_or(1.0, |other| 1.0 - other.influence);
        let handle = Handle {
            influence: handle.influence.min(room),
            ..handle
        };
        self.change(at, |key| key.timing = key.timing.with_handle(side, handle))
    }
}
