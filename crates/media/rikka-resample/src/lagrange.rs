pub(crate) fn cubic(x: f64) -> [f32; 4] {
    let x2 = x * x;
    let x3 = x2 * x;
    [
        (-x / 3.0 + 0.5 * x2 - x3 / 6.0) as f32,
        (1.0 - 0.5 * x - x2 + 0.5 * x3) as f32,
        (x + 0.5 * x2 - 0.5 * x3) as f32,
        (-x / 6.0 + x3 / 6.0) as f32,
    ]
}
