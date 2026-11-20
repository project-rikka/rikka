pub(crate) fn fitted(view: f64, span: f64, x: f64, width: f64, padding: f64) -> f64 {
    if span <= width {
        return x;
    }
    let padding = padded(span, width, padding);
    let left = x - padding;
    let right = x + width + padding;
    if view <= left && right <= view + span {
        return view;
    }
    if (view - left).abs() <= (view + span - right).abs() {
        left
    } else {
        right - span
    }
}

pub(crate) fn padded(span: f64, width: f64, padding: f64) -> f64 {
    ((span - width) / 2.0).clamp(0.0, padding)
}
