pub(crate) fn signum(value: f64) -> f64 {
    if value < 0.0 {
        -1.0
    } else if value == 0.0 {
        0.0
    } else {
        1.0
    }
}

pub(crate) fn lerp(start: f64, stop: f64, amount: f64) -> f64 {
    (1.0 - amount).mul_add(start, amount * stop)
}

pub(crate) const fn sanitize_degrees_int(degrees: i32) -> usize {
    if degrees < 0 {
        (degrees + 360) as usize
    } else {
        degrees as usize % 360
    }
}

pub(crate) fn sanitize_degrees(degrees: f64) -> f64 {
    if degrees < 0.0 {
        degrees + 360.0
    } else {
        degrees % 360.0
    }
}

pub(crate) fn rotate_direction(from: f64, to: f64) -> f64 {
    if sanitize_degrees(to - from) <= 180.0 {
        1.0
    } else {
        -1.0
    }
}

pub(crate) fn difference_degrees(a: f64, b: f64) -> f64 {
    180.0 - ((a - b).abs() - 180.0).abs()
}

pub(crate) fn matrix_multiply(row: [f64; 3], matrix: [[f64; 3]; 3]) -> [f64; 3] {
    matrix.map(|line| row[2].mul_add(line[2], row[0].mul_add(line[0], row[1] * line[1])))
}
