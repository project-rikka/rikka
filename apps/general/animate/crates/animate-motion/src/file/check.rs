use crate::error::{Error, Result};

const UNIT_SLACK: f32 = 1e-3;

pub(super) fn finite(values: &[f32]) -> bool {
    values.iter().all(|value| value.is_finite())
}

pub(super) fn unit(rotation: [f32; 4]) -> bool {
    finite(&rotation)
        && (rotation.iter().map(|part| part * part).sum::<f32>().sqrt() - 1.0).abs() <= UNIT_SLACK
}

pub(super) fn sound(track: &str, frame: u32, fine: bool) -> Result<()> {
    match fine {
        true => Ok(()),
        false => Err(Error::Malformed {
            track: track.to_owned(),
            frame,
        }),
    }
}
