pub(crate) type Matrix = [[f64; 3]; 3];

pub(crate) fn apply(m: &Matrix, v: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|row| m[row][0] * v[0] + m[row][1] * v[1] + m[row][2] * v[2])
}

pub(crate) fn product(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.0; 3]; 3];
    for row in 0..3 {
        for column in 0..3 {
            out[row][column] = (0..3).map(|k| a[row][k] * b[k][column]).sum();
        }
    }
    out
}

pub(crate) fn inverse(m: &Matrix) -> Matrix {
    let [[a, b, c], [d, e, f], [g, h, i]] = *m;
    let determinant = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    let k = 1.0 / determinant;
    [
        [
            (e * i - f * h) * k,
            (c * h - b * i) * k,
            (b * f - c * e) * k,
        ],
        [
            (f * g - d * i) * k,
            (a * i - c * g) * k,
            (c * d - a * f) * k,
        ],
        [
            (d * h - e * g) * k,
            (b * g - a * h) * k,
            (a * e - b * d) * k,
        ],
    ]
}
