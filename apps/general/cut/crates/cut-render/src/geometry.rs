use rikka_types::Rect;

pub(crate) const LARGEST: f32 = 8192.0;

pub(crate) fn centred(width: f32, height: f32) -> Rect {
    Rect::new(-width / 2.0, -height / 2.0, width, height)
}

pub(crate) fn stepped_up(value: f32, steps: f32) -> f32 {
    ((value.log2() * steps).ceil() / steps).exp2()
}
