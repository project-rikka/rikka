use rikka_vector::Transform;

#[inline]
pub(crate) fn device(transform: &rikka_text_font::Transform) -> Transform {
    Transform {
        xx: transform.xx,
        yx: transform.yx,
        xy: transform.xy,
        yy: transform.yy,
        dx: transform.dx,
        dy: transform.dy,
    }
}
