use crate::error::{Error, Result};

pub(crate) const NONE: u8 = 0;
pub(crate) const SUB: u8 = 1;
pub(crate) const UP: u8 = 2;
pub(crate) const AVERAGE: u8 = 3;
pub(crate) const PAETH: u8 = 4;

pub(crate) fn unfilter(rows: &mut [u8], row_bytes: usize, stride: usize) -> Result<()> {
    let pitch = row_bytes + 1;
    let mut previous: Option<&[u8]> = None;
    for row in rows.chunks_exact_mut(pitch) {
        let (filter, line) = row.split_first_mut().ok_or(Error::Truncated)?;
        match (*filter, previous) {
            (NONE, _) | (UP, None) => {}
            (SUB, _) | (PAETH, None) => sub(line, stride),
            (UP, Some(above)) => {
                for (value, &up) in line.iter_mut().zip(above) {
                    *value = value.wrapping_add(up);
                }
            }
            (AVERAGE, None) => {
                for at in stride..line.len() {
                    line[at] = line[at].wrapping_add(line[at - stride] / 2);
                }
            }
            (AVERAGE, Some(above)) => match stride {
                1 => average::<1>(line, above),
                2 => average::<2>(line, above),
                3 => average::<3>(line, above),
                4 => average::<4>(line, above),
                6 => average::<6>(line, above),
                _ => average::<8>(line, above),
            },
            (PAETH, Some(above)) => match stride {
                1 => paeth::<1>(line, above),
                2 => paeth::<2>(line, above),
                3 => paeth::<3>(line, above),
                4 => paeth::<4>(line, above),
                6 => paeth::<6>(line, above),
                _ => paeth::<8>(line, above),
            },
            (unknown, _) => return Err(Error::Filter(unknown)),
        }
        previous = Some(line);
    }
    Ok(())
}

fn sub(line: &mut [u8], stride: usize) {
    match stride {
        1 => sub_by::<1>(line),
        2 => sub_by::<2>(line),
        3 => sub_by::<3>(line),
        4 => sub_by::<4>(line),
        6 => sub_by::<6>(line),
        _ => sub_by::<8>(line),
    }
}

#[inline]
fn sub_by<const N: usize>(line: &mut [u8]) {
    let mut left = [0u8; N];
    for pixel in line.as_chunks_mut::<N>().0 {
        for (value, left) in pixel.iter_mut().zip(&mut left) {
            *value = value.wrapping_add(*left);
            *left = *value;
        }
    }
}

#[inline]
fn average<const N: usize>(line: &mut [u8], above: &[u8]) {
    let mut left = [0u8; N];
    for (pixel, up) in line
        .as_chunks_mut::<N>()
        .0
        .iter_mut()
        .zip(above.as_chunks::<N>().0)
    {
        for channel in 0..N {
            let average = ((u16::from(left[channel]) + u16::from(up[channel])) / 2) as u8;
            pixel[channel] = pixel[channel].wrapping_add(average);
        }
        left = *pixel;
    }
}

#[inline]
fn paeth<const N: usize>(line: &mut [u8], above: &[u8]) {
    let mut left = [0u8; N];
    let mut up_left = [0u8; N];
    for (pixel, up) in line
        .as_chunks_mut::<N>()
        .0
        .iter_mut()
        .zip(above.as_chunks::<N>().0)
    {
        for channel in 0..N {
            let predicted = predictor(left[channel], up[channel], up_left[channel]);
            pixel[channel] = pixel[channel].wrapping_add(predicted);
        }
        left = *pixel;
        up_left = *up;
    }
}

#[inline]
pub(crate) fn predictor(left: u8, up: u8, up_left: u8) -> u8 {
    let estimate = i16::from(left) + i16::from(up) - i16::from(up_left);
    let distance_left = (estimate - i16::from(left)).abs();
    let distance_up = (estimate - i16::from(up)).abs();
    let distance_up_left = (estimate - i16::from(up_left)).abs();
    if distance_left <= distance_up && distance_left <= distance_up_left {
        left
    } else if distance_up <= distance_up_left {
        up
    } else {
        up_left
    }
}
