pub(crate) type Cubic = [f64; 4];

pub(crate) fn at(c: Cubic, u: f64) -> f64 {
    let v = 1.0 - u;
    v * v * v * c[0] + 3.0 * v * v * u * c[1] + 3.0 * v * u * u * c[2] + u * u * u * c[3]
}

pub(crate) fn slope(c: Cubic, u: f64) -> f64 {
    let v = 1.0 - u;
    3.0 * (v * v * (c[1] - c[0]) + 2.0 * v * u * (c[2] - c[1]) + u * u * (c[3] - c[2]))
}

pub(crate) fn split(c: Cubic, u: f64) -> (Cubic, Cubic) {
    let lerp = |a: f64, b: f64| a + (b - a) * u;
    let (ab, bc, cd) = (lerp(c[0], c[1]), lerp(c[1], c[2]), lerp(c[2], c[3]));
    let (abc, bcd) = (lerp(ab, bc), lerp(bc, cd));
    let middle = lerp(abc, bcd);
    ([c[0], ab, abc, middle], [middle, bcd, cd, c[3]])
}

pub(crate) fn solve(c: Cubic, x: f64) -> f64 {
    if x <= c[0] {
        return 0.0;
    }
    if x >= c[3] {
        return 1.0;
    }
    let tolerance = 1e-12 * (c[3] - c[0]).max(1.0);
    let (mut low, mut high) = (0.0, 1.0);
    let mut u = (x - c[0]) / (c[3] - c[0]);
    for _ in 0..100 {
        let error = at(c, u) - x;
        if error.abs() <= tolerance {
            break;
        }
        match error > 0.0 {
            true => high = u,
            false => low = u,
        }
        let rate = slope(c, u);
        let guess = u - error / rate;
        u = match rate > 0.0 && guess > low && guess < high {
            true => guess,
            false => (low + high) / 2.0,
        };
    }
    u
}

pub(crate) fn segment(c: Cubic, start: f64, end: f64) -> Cubic {
    let (_, right) = split(c, start);
    match 1.0 - start > 1e-12 {
        true => split(right, (end - start) / (1.0 - start)).0,
        false => [right[3]; 4],
    }
}
