mod mapping;
mod primaries;
mod transfer;

fn close(actual: f64, expected: f64, tolerance: f64) -> bool {
    (actual - expected).abs() <= tolerance
}
