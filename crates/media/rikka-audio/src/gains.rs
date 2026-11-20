use crate::frame::{Frame, MAX_CHANNELS};

const SILENCE: f32 = 1e-4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fold {
    MonoToMulti,
    ToMono,
    Matched,
    Wrapping,
}

impl Fold {
    pub(crate) fn select(source_channels: usize, out_channels: usize) -> Fold {
        if out_channels == 1 {
            Fold::ToMono
        } else if source_channels == 1 {
            Fold::MonoToMulti
        } else if source_channels == out_channels {
            Fold::Matched
        } else {
            Fold::Wrapping
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BlockGains {
    ll: f32,
    lr: f32,
    rl: f32,
    rr: f32,
    rest: f32,
}

impl BlockGains {
    pub(crate) const ZERO: BlockGains = BlockGains {
        ll: 0.0,
        lr: 0.0,
        rl: 0.0,
        rr: 0.0,
        rest: 0.0,
    };

    pub(crate) fn target(amplitude: f32, pan: f32, mono_source: bool) -> BlockGains {
        if mono_source {
            let theta = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
            BlockGains {
                ll: amplitude * theta.cos(),
                lr: 0.0,
                rl: amplitude * theta.sin(),
                rr: 0.0,
                rest: amplitude,
            }
        } else if pan >= 0.0 {
            let theta = pan.min(1.0) * std::f32::consts::FRAC_PI_2;
            BlockGains {
                ll: amplitude * theta.cos(),
                lr: 0.0,
                rl: amplitude * theta.sin(),
                rr: amplitude,
                rest: amplitude,
            }
        } else {
            let theta = (-pan).min(1.0) * std::f32::consts::FRAC_PI_2;
            BlockGains {
                ll: amplitude,
                lr: amplitude * theta.sin(),
                rl: 0.0,
                rr: amplitude * theta.cos(),
                rest: amplitude,
            }
        }
    }

    pub(crate) fn step_towards(&self, target: &BlockGains, frames: usize) -> BlockGains {
        let scale = if frames > 0 { 1.0 / frames as f32 } else { 0.0 };
        BlockGains {
            ll: (target.ll - self.ll) * scale,
            lr: (target.lr - self.lr) * scale,
            rl: (target.rl - self.rl) * scale,
            rr: (target.rr - self.rr) * scale,
            rest: (target.rest - self.rest) * scale,
        }
    }

    pub(crate) fn advance(&mut self, step: &BlockGains) {
        self.ll += step.ll;
        self.lr += step.lr;
        self.rl += step.rl;
        self.rr += step.rr;
        self.rest += step.rest;
    }

    pub(crate) fn is_silent(&self) -> bool {
        self.ll.abs() < SILENCE
            && self.lr.abs() < SILENCE
            && self.rl.abs() < SILENCE
            && self.rr.abs() < SILENCE
            && self.rest.abs() < SILENCE
    }

    #[inline]
    pub(crate) fn mix_frame(
        &self,
        destination: &mut [f32],
        source: &Frame,
        source_channels: usize,
        fold: Fold,
    ) {
        match fold {
            Fold::ToMono => {
                let value = if source_channels == 1 {
                    source[0]
                } else {
                    source[..source_channels].iter().sum::<f32>() / source_channels as f32
                };
                destination[0] += self.rest * value;
            }
            Fold::MonoToMulti => {
                let value = source[0];
                destination[0] += self.ll * value;
                destination[1] += self.rl * value;
                for out in &mut destination[2..] {
                    *out += self.rest * value;
                }
            }
            Fold::Matched => {
                destination[0] += self.ll * source[0] + self.lr * source[1];
                destination[1] += self.rl * source[0] + self.rr * source[1];
                for (out, &value) in destination[2..].iter_mut().zip(&source[2..source_channels]) {
                    *out += self.rest * value;
                }
            }
            Fold::Wrapping => {
                let out_channels = destination.len();
                let mut folded: Frame = [0.0; MAX_CHANNELS];
                let mut channel = 0;
                for &value in &source[..source_channels] {
                    folded[channel] += value;
                    channel += 1;
                    if channel == out_channels {
                        channel = 0;
                    }
                }
                destination[0] += self.ll * folded[0] + self.lr * folded[1];
                destination[1] += self.rl * folded[0] + self.rr * folded[1];
                for (out, &value) in destination[2..].iter_mut().zip(&folded[2..out_channels]) {
                    *out += self.rest * value;
                }
            }
        }
    }
}
