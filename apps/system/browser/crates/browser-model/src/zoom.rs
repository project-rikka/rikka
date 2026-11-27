pub const STEPS: [f64; 16] = [
    0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0,
];
const SAME: f64 = 0.001;

pub fn same(left: f64, right: f64) -> bool {
    (left - right).abs() < SAME
}

pub fn step(current: f64, by: i32, default: f64) -> f64 {
    match by {
        0 => default,
        by if by > 0 => STEPS
            .iter()
            .copied()
            .find(|zoom| *zoom > current + SAME)
            .unwrap_or(current),
        _ => STEPS
            .iter()
            .rev()
            .copied()
            .find(|zoom| *zoom < current - SAME)
            .unwrap_or(current),
    }
}
