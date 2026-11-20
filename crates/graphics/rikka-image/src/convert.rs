use crate::color_type::ColorType;
use crate::pixels::{Pixels, Sample, by_depth};

pub(crate) fn convert(pixels: &Pixels, from: ColorType, to: ColorType) -> Pixels {
    by_depth!(pixels, source => match to.bits_per_channel() {
        8 => Pixels::U8(relaid::<_, u8>(source, from, to)),
        16 => Pixels::U16(relaid::<_, u16>(source, from, to)),
        _ => Pixels::F32(relaid::<_, f32>(source, from, to)),
    })
}

#[inline]
fn mapped<S: Sample, D: Sample, const F: usize, const T: usize>(
    source: &[S],
    pixel: impl Fn(&[S; F]) -> [D; T],
) -> Vec<D> {
    let mut out = vec![D::default(); source.len() / F * T];
    for (target, source) in out
        .as_chunks_mut::<T>()
        .0
        .iter_mut()
        .zip(source.as_chunks::<F>().0)
    {
        *target = pixel(source);
    }
    out
}

fn relaid<S: Sample, D: Sample>(source: &[S], from: ColorType, to: ColorType) -> Vec<D> {
    let full = D::FULL;
    let luma = |pixel: &[S]| S::luma([pixel[0], pixel[1], pixel[2]]).cast::<D>();
    match (from.channel_count(), to.channel_count()) {
        (1, 1) => mapped(source, |&[gray]: &[S; 1]| [gray.cast()]),
        (1, 2) => mapped(source, |&[gray]: &[S; 1]| [gray.cast(), full]),
        (1, 3) => mapped(source, |&[gray]: &[S; 1]| [gray.cast(); 3]),
        (1, _) => mapped(source, |&[gray]: &[S; 1]| {
            let gray = gray.cast();
            [gray, gray, gray, full]
        }),
        (2, 1) => mapped(source, |&[gray, _]: &[S; 2]| [gray.cast()]),
        (2, 2) => mapped(source, |&[gray, alpha]: &[S; 2]| {
            [gray.cast(), alpha.cast()]
        }),
        (2, 3) => mapped(source, |&[gray, _]: &[S; 2]| [gray.cast(); 3]),
        (2, _) => mapped(source, |&[gray, alpha]: &[S; 2]| {
            let gray = gray.cast();
            [gray, gray, gray, alpha.cast()]
        }),
        (3, 1) => mapped(source, |pixel: &[S; 3]| [luma(pixel)]),
        (3, 2) => mapped(source, |pixel: &[S; 3]| [luma(pixel), full]),
        (3, 3) => mapped(source, |pixel: &[S; 3]| pixel.map(S::cast)),
        (3, _) => mapped(source, |&[red, green, blue]: &[S; 3]| {
            [red.cast(), green.cast(), blue.cast(), full]
        }),
        (_, 1) => mapped(source, |pixel: &[S; 4]| [luma(pixel)]),
        (_, 2) => mapped(source, |pixel: &[S; 4]| [luma(pixel), pixel[3].cast()]),
        (_, 3) => mapped(source, |&[red, green, blue, _]: &[S; 4]| {
            [red.cast(), green.cast(), blue.cast()]
        }),
        (_, _) => mapped(source, |pixel: &[S; 4]| pixel.map(S::cast)),
    }
}
