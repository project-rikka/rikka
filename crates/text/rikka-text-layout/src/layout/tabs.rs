use crate::fonts::FaceInstance;

#[inline]
pub(crate) fn tab_stop(face: &FaceInstance, size: f32, tab_width: u16) -> f32 {
    face.space_advance * size * f32::from(tab_width)
}

#[inline]
pub(crate) fn tab_advance(x: f32, stop: f32) -> f32 {
    if stop <= 0.0 {
        return 0.0;
    }
    ((x / stop).floor() + 1.0) * stop - x
}
