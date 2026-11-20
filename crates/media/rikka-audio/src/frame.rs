use crate::error::{Error, Result, Role};

pub(crate) const MAX_CHANNELS: usize = 16;

pub(crate) type Frame = [f32; MAX_CHANNELS];

pub(crate) fn check_channels(channels: u16, role: Role) -> Result<()> {
    match channels {
        0 => Err(Error::NoChannels(role)),
        _ if usize::from(channels) > MAX_CHANNELS => Err(Error::TooManyChannels {
            role,
            channels,
            most: MAX_CHANNELS,
        }),
        _ => Ok(()),
    }
}

#[inline]
pub(crate) fn interpolate(
    xm1: &[f32],
    x0: &[f32],
    x1: &[f32],
    x2: &[f32],
    t: f32,
    out: &mut Frame,
) {
    for (channel, value) in out.iter_mut().enumerate().take(x0.len()) {
        *value = hermite(xm1[channel], x0[channel], x1[channel], x2[channel], t);
    }
}

#[inline]
fn hermite(xm1: f32, x0: f32, x1: f32, x2: f32, t: f32) -> f32 {
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * t + c2) * t + c1) * t + x0
}
