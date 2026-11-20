use crate::media::Viewport;

const SMALLEST_SIDE: u32 = 16;
const SLACK: f64 = 1.02;
const FASTEST: u32 = 240;
const SLOWEST: u32 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Size {
    pub(crate) width: u32,
    pub(crate) height: u32,
}

pub(crate) fn encoded_size(source: Size, viewport: Viewport, largest: Size) -> Size {
    let ratio = |limit: u32, side: u32| f64::from(limit) / f64::from(side);
    let fit =
        ratio(viewport.width, source.width).min(ratio(viewport.height, source.height)) * SLACK;
    let decodable = ratio(largest.width, source.width).min(ratio(largest.height, source.height));
    let scale = fit.min(1.0).min(decodable);
    Size {
        width: even((f64::from(source.width) * scale).floor() as u32),
        height: even((f64::from(source.height) * scale).floor() as u32),
    }
}

pub(crate) fn frame_rate(display_hz: f64, viewport: Viewport) -> u32 {
    let display = display_hz.round() as u32;
    let client = viewport.refresh_hz.round() as u32;
    display.min(client).clamp(SLOWEST, FASTEST)
}

fn even(side: u32) -> u32 {
    (side & !1).max(SMALLEST_SIDE)
}
