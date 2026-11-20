use crate::pixels::{Pixels, Sample};
use crate::raster::Image;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Blend,
    Copy,
}

impl Image {
    #[must_use]
    pub fn overlay(&self, top: &Image, x: i64, y: i64) -> Image {
        let mut composed = self.clone();
        compose(&mut composed, top, x, y, Mode::Blend);
        composed
    }

    pub fn paste_in_place(&mut self, top: &Image, x: i64, y: i64) {
        compose(self, top, x, y, Mode::Copy);
    }
}

#[derive(Clone, Copy)]
struct Overlap {
    bottom_x: usize,
    bottom_y: usize,
    top_x: usize,
    top_y: usize,
    width: usize,
    height: usize,
}

impl Overlap {
    fn of(bottom: (u32, u32), top: (u32, u32), x: i64, y: i64) -> Option<Overlap> {
        let left = x.max(0);
        let upper = y.max(0);
        let right = (x + i64::from(top.0)).min(i64::from(bottom.0));
        let lower = (y + i64::from(top.1)).min(i64::from(bottom.1));
        (right > left && lower > upper).then(|| Overlap {
            bottom_x: left as usize,
            bottom_y: upper as usize,
            top_x: (left - x) as usize,
            top_y: (upper - y) as usize,
            width: (right - left) as usize,
            height: (lower - upper) as usize,
        })
    }
}

fn compose(bottom: &mut Image, top: &Image, x: i64, y: i64, mode: Mode) {
    let Some(overlap) = Overlap::of(bottom.dimensions(), top.dimensions(), x, y) else {
        return;
    };
    let color = bottom.color();
    let blends = mode == Mode::Blend && top.has_alpha();
    let top_color = match blends {
        true => color.with_alpha(),
        false => color,
    };
    let top = top.in_color(top_color);
    let widths = (bottom.width() as usize, top.width() as usize);
    let channels = (
        usize::from(color.channel_count()),
        usize::from(top_color.channel_count()),
    );
    match (&mut bottom.pixels, &top.pixels) {
        (Pixels::U8(bottom), Pixels::U8(top)) => {
            rows(bottom, top, widths, channels, overlap, blends);
        }
        (Pixels::U16(bottom), Pixels::U16(top)) => {
            rows(bottom, top, widths, channels, overlap, blends);
        }
        (Pixels::F32(bottom), Pixels::F32(top)) => {
            rows(bottom, top, widths, channels, overlap, blends);
        }
        _ => unreachable!("top was converted to the bottom's depth"),
    }
}

fn rows<S: Sample>(
    bottom: &mut [S],
    top: &[S],
    (bottom_width, top_width): (usize, usize),
    (bottom_channels, top_channels): (usize, usize),
    overlap: Overlap,
    blends: bool,
) {
    for row in 0..overlap.height {
        let bottom_at =
            ((overlap.bottom_y + row) * bottom_width + overlap.bottom_x) * bottom_channels;
        let top_at = ((overlap.top_y + row) * top_width + overlap.top_x) * top_channels;
        let target = &mut bottom[bottom_at..bottom_at + overlap.width * bottom_channels];
        let source = &top[top_at..top_at + overlap.width * top_channels];
        if !blends {
            target.copy_from_slice(source);
            continue;
        }
        for (under, over) in target
            .chunks_exact_mut(bottom_channels)
            .zip(source.chunks_exact(top_channels))
        {
            blend(under, over);
        }
    }
}

#[inline]
fn blend<S: Sample>(under: &mut [S], over: &[S]) {
    let colors = over.len() - 1;
    let over_alpha = over[colors].to_unit();
    if under.len() == colors {
        for (under, over) in under.iter_mut().zip(over) {
            let mixed = over.to_unit() * over_alpha + under.to_unit() * (1.0 - over_alpha);
            *under = S::from_unit(mixed);
        }
        return;
    }
    let under_alpha = under[colors].to_unit();
    let alpha = over_alpha + under_alpha * (1.0 - over_alpha);
    if alpha <= 0.0 {
        return;
    }
    for (under, over) in under[..colors].iter_mut().zip(over) {
        let mixed =
            over.to_unit() * over_alpha + under.to_unit() * under_alpha * (1.0 - over_alpha);
        *under = S::from_unit(mixed / alpha);
    }
    under[colors] = S::from_unit(alpha);
}
