use crate::common::FFT_LEN;

#[expect(
    clippy::excessive_precision,
    reason = "values copied verbatim from WebRTC"
)]
const RDFT_W: [f32; 64] = [
    1.0000000000,
    0.0000000000,
    0.7071067691,
    0.7071067691,
    0.9238795638,
    0.3826834559,
    0.3826834559,
    0.9238795638,
    0.9807852507,
    0.1950903237,
    0.5555702448,
    0.8314695954,
    0.8314695954,
    0.5555702448,
    0.1950903237,
    0.9807852507,
    0.9951847196,
    0.0980171412,
    0.6343933344,
    0.7730104327,
    0.8819212914,
    0.4713967443,
    0.2902846634,
    0.9569403529,
    0.9569403529,
    0.2902846634,
    0.4713967443,
    0.8819212914,
    0.7730104327,
    0.6343933344,
    0.0980171412,
    0.9951847196,
    0.7071067691,
    0.4993977249,
    0.4975923598,
    0.4945882559,
    0.4903926253,
    0.4850156307,
    0.4784701765,
    0.4707720280,
    0.4619397819,
    0.4519946277,
    0.4409606457,
    0.4288643003,
    0.4157347977,
    0.4016037583,
    0.3865052164,
    0.3704755902,
    0.3535533845,
    0.3357794881,
    0.3171966672,
    0.2978496552,
    0.2777851224,
    0.2570513785,
    0.2356983721,
    0.2137775421,
    0.1913417280,
    0.1684449315,
    0.1451423317,
    0.1214900985,
    0.0975451618,
    0.0733652338,
    0.0490085706,
    0.0245338380,
];

#[expect(
    clippy::excessive_precision,
    reason = "values copied verbatim from WebRTC"
)]
const RDFT_WK3RI_FIRST: [f32; 16] = [
    1.000000000,
    0.000000000,
    0.382683456,
    0.923879564,
    0.831469536,
    0.555570245,
    -0.195090353,
    0.980785251,
    0.956940353,
    0.290284693,
    0.098017156,
    0.995184720,
    0.634393334,
    0.773010492,
    -0.471396863,
    0.881921172,
];

#[expect(
    clippy::excessive_precision,
    reason = "values copied verbatim from WebRTC"
)]
const RDFT_WK3RI_SECOND: [f32; 16] = [
    -0.707106769,
    0.707106769,
    -0.923879564,
    -0.382683456,
    -0.980785251,
    0.195090353,
    -0.555570245,
    -0.831469536,
    -0.881921172,
    0.471396863,
    -0.773010492,
    -0.634393334,
    -0.995184720,
    -0.098017156,
    -0.290284693,
    -0.956940353,
];

pub(crate) fn forward(a: &mut [f32; FFT_LEN]) {
    bitrv2_128(a);
    cftfsub_128(a);
    rftfsub_128(a);
    let xi = a[0] - a[1];
    a[0] += a[1];
    a[1] = xi;
}

pub(crate) fn inverse(a: &mut [f32; FFT_LEN]) {
    a[1] = 0.5 * (a[0] - a[1]);
    a[0] -= a[1];
    rftbsub_128(a);
    bitrv2_128(a);
    cftbsub_128(a);
}

fn cft1st_128(a: &mut [f32; FFT_LEN]) {
    let x0r = a[0] + a[2];
    let x0i = a[1] + a[3];
    let x1r = a[0] - a[2];
    let x1i = a[1] - a[3];
    let x2r = a[4] + a[6];
    let x2i = a[5] + a[7];
    let x3r = a[4] - a[6];
    let x3i = a[5] - a[7];
    a[0] = x0r + x2r;
    a[1] = x0i + x2i;
    a[4] = x0r - x2r;
    a[5] = x0i - x2i;
    a[2] = x1r - x3i;
    a[3] = x1i + x3r;
    a[6] = x1r + x3i;
    a[7] = x1i - x3r;
    let wk1r = RDFT_W[2];
    let x0r = a[8] + a[10];
    let x0i = a[9] + a[11];
    let x1r = a[8] - a[10];
    let x1i = a[9] - a[11];
    let x2r = a[12] + a[14];
    let x2i = a[13] + a[15];
    let x3r = a[12] - a[14];
    let x3i = a[13] - a[15];
    a[8] = x0r + x2r;
    a[9] = x0i + x2i;
    a[12] = x2i - x0i;
    a[13] = x0r - x2r;
    let y0r = x1r - x3i;
    let y0i = x1i + x3r;
    a[10] = wk1r * (y0r - y0i);
    a[11] = wk1r * (y0r + y0i);
    let y0r = x3i + x1r;
    let y0i = x3r - x1i;
    a[14] = wk1r * (y0i - y0r);
    a[15] = wk1r * (y0i + y0r);
    let mut k1 = 0;
    for j in (16..FFT_LEN).step_by(16) {
        k1 += 2;
        let k2 = 2 * k1;
        let wk2r = RDFT_W[k1];
        let wk2i = RDFT_W[k1 + 1];
        let first = [
            RDFT_W[k2],
            RDFT_W[k2 + 1],
            wk2r,
            wk2i,
            RDFT_WK3RI_FIRST[k1],
            RDFT_WK3RI_FIRST[k1 + 1],
        ];
        twiddle(a, j, 2, first, false);
        let second = [
            RDFT_W[k2 + 2],
            RDFT_W[k2 + 3],
            wk2r,
            wk2i,
            RDFT_WK3RI_SECOND[k1],
            RDFT_WK3RI_SECOND[k1 + 1],
        ];
        twiddle(a, j + 8, 2, second, true);
    }
}

fn cross(a: &[f32; FFT_LEN], j0: usize, stride: usize) -> [f32; 8] {
    let (j1, j2, j3) = (j0 + stride, j0 + 2 * stride, j0 + 3 * stride);
    [
        a[j0] + a[j1],
        a[j0 + 1] + a[j1 + 1],
        a[j0] - a[j1],
        a[j0 + 1] - a[j1 + 1],
        a[j2] + a[j3],
        a[j2 + 1] + a[j3 + 1],
        a[j2] - a[j3],
        a[j2 + 1] - a[j3 + 1],
    ]
}

fn twiddle(a: &mut [f32; FFT_LEN], j0: usize, stride: usize, w: [f32; 6], rotated: bool) {
    let [wk1r, wk1i, wk2r, wk2i, wk3r, wk3i] = w;
    let (j1, j2, j3) = (j0 + stride, j0 + 2 * stride, j0 + 3 * stride);
    let [x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i] = cross(a, j0, stride);
    a[j0] = x0r + x2r;
    a[j0 + 1] = x0i + x2i;
    let y0r = x0r - x2r;
    let y0i = x0i - x2i;
    if rotated {
        a[j2] = -wk2i * y0r - wk2r * y0i;
        a[j2 + 1] = -wk2i * y0i + wk2r * y0r;
    } else {
        a[j2] = wk2r * y0r - wk2i * y0i;
        a[j2 + 1] = wk2r * y0i + wk2i * y0r;
    }
    let y0r = x1r - x3i;
    let y0i = x1i + x3r;
    a[j1] = wk1r * y0r - wk1i * y0i;
    a[j1 + 1] = wk1r * y0i + wk1i * y0r;
    let y0r = x1r + x3i;
    let y0i = x1i - x3r;
    a[j3] = wk3r * y0r - wk3i * y0i;
    a[j3 + 1] = wk3r * y0i + wk3i * y0r;
}

fn cftmdl_128(a: &mut [f32; FFT_LEN]) {
    const L: usize = 8;
    const M: usize = 32;
    for j0 in (0..L).step_by(2) {
        let (j1, j2, j3) = (j0 + 8, j0 + 16, j0 + 24);
        let [x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i] = cross(a, j0, 8);
        a[j0] = x0r + x2r;
        a[j0 + 1] = x0i + x2i;
        a[j2] = x0r - x2r;
        a[j2 + 1] = x0i - x2i;
        a[j1] = x1r - x3i;
        a[j1 + 1] = x1i + x3r;
        a[j3] = x1r + x3i;
        a[j3 + 1] = x1i - x3r;
    }
    let wk1r = RDFT_W[2];
    for j0 in (M..L + M).step_by(2) {
        let (j1, j2, j3) = (j0 + 8, j0 + 16, j0 + 24);
        let [x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i] = cross(a, j0, 8);
        a[j0] = x0r + x2r;
        a[j0 + 1] = x0i + x2i;
        a[j2] = x2i - x0i;
        a[j2 + 1] = x0r - x2r;
        let y0r = x1r - x3i;
        let y0i = x1i + x3r;
        a[j1] = wk1r * (y0r - y0i);
        a[j1 + 1] = wk1r * (y0r + y0i);
        let y0r = x3i + x1r;
        let y0i = x3r - x1i;
        a[j3] = wk1r * (y0i - y0r);
        a[j3 + 1] = wk1r * (y0i + y0r);
    }
    let mut k1 = 0;
    for k in (2 * M..FFT_LEN).step_by(2 * M) {
        k1 += 2;
        let k2 = 2 * k1;
        let (wk2r, wk2i) = (RDFT_W[k1], RDFT_W[k1 + 1]);
        let first = [
            RDFT_W[k2],
            RDFT_W[k2 + 1],
            wk2r,
            wk2i,
            RDFT_WK3RI_FIRST[k1],
            RDFT_WK3RI_FIRST[k1 + 1],
        ];
        for j0 in (k..L + k).step_by(2) {
            twiddle(a, j0, 8, first, false);
        }
        let second = [
            RDFT_W[k2 + 2],
            RDFT_W[k2 + 3],
            wk2r,
            wk2i,
            RDFT_WK3RI_SECOND[k1],
            RDFT_WK3RI_SECOND[k1 + 1],
        ];
        for j0 in (k + M..L + k + M).step_by(2) {
            twiddle(a, j0, 8, second, true);
        }
    }
}

fn rftfsub_128(a: &mut [f32; FFT_LEN]) {
    let c = &RDFT_W[32..];
    let mut j1 = 1;
    let mut j2 = 2;
    while j2 < 64 {
        let k2 = 128 - j2;
        let k1 = 32 - j1;
        let wkr = 0.5 - c[k1];
        let wki = c[j1];
        let xr = a[j2] - a[k2];
        let xi = a[j2 + 1] + a[k2 + 1];
        let yr = wkr * xr - wki * xi;
        let yi = wkr * xi + wki * xr;
        a[j2] -= yr;
        a[j2 + 1] -= yi;
        a[k2] += yr;
        a[k2 + 1] -= yi;
        j1 += 1;
        j2 += 2;
    }
}

fn rftbsub_128(a: &mut [f32; FFT_LEN]) {
    let c = &RDFT_W[32..];
    a[1] = -a[1];
    let mut j1 = 1;
    let mut j2 = 2;
    while j2 < 64 {
        let k2 = 128 - j2;
        let k1 = 32 - j1;
        let wkr = 0.5 - c[k1];
        let wki = c[j1];
        let xr = a[j2] - a[k2];
        let xi = a[j2 + 1] + a[k2 + 1];
        let yr = wkr * xr + wki * xi;
        let yi = wkr * xi - wki * xr;
        a[j2] -= yr;
        a[j2 + 1] = yi - a[j2 + 1];
        a[k2] += yr;
        a[k2 + 1] = yi - a[k2 + 1];
        j1 += 1;
        j2 += 2;
    }
    a[65] = -a[65];
}

fn cftfsub_128(a: &mut [f32; FFT_LEN]) {
    cft1st_128(a);
    cftmdl_128(a);
    const L: usize = 32;
    for j in (0..L).step_by(2) {
        let (j1, j2, j3) = (j + L, j + 2 * L, j + 3 * L);
        let x0r = a[j] + a[j1];
        let x0i = a[j + 1] + a[j1 + 1];
        let x1r = a[j] - a[j1];
        let x1i = a[j + 1] - a[j1 + 1];
        let x2r = a[j2] + a[j3];
        let x2i = a[j2 + 1] + a[j3 + 1];
        let x3r = a[j2] - a[j3];
        let x3i = a[j2 + 1] - a[j3 + 1];
        a[j] = x0r + x2r;
        a[j + 1] = x0i + x2i;
        a[j2] = x0r - x2r;
        a[j2 + 1] = x0i - x2i;
        a[j1] = x1r - x3i;
        a[j1 + 1] = x1i + x3r;
        a[j3] = x1r + x3i;
        a[j3 + 1] = x1i - x3r;
    }
}

fn cftbsub_128(a: &mut [f32; FFT_LEN]) {
    cft1st_128(a);
    cftmdl_128(a);
    const L: usize = 32;
    for j in (0..L).step_by(2) {
        let (j1, j2, j3) = (j + L, j + 2 * L, j + 3 * L);
        let x0r = a[j] + a[j1];
        let x0i = -a[j + 1] - a[j1 + 1];
        let x1r = a[j] - a[j1];
        let x1i = -a[j + 1] + a[j1 + 1];
        let x2r = a[j2] + a[j3];
        let x2i = a[j2 + 1] + a[j3 + 1];
        let x3r = a[j2] - a[j3];
        let x3i = a[j2 + 1] - a[j3 + 1];
        a[j] = x0r + x2r;
        a[j + 1] = x0i - x2i;
        a[j2] = x0r - x2r;
        a[j2 + 1] = x0i + x2i;
        a[j1] = x1r - x3i;
        a[j1 + 1] = x1i - x3r;
        a[j3] = x1r + x3i;
        a[j3 + 1] = x1i + x3r;
    }
}

fn swap_pair(a: &mut [f32; FFT_LEN], j: usize, k: usize) {
    a.swap(j, k);
    a.swap(j + 1, k + 1);
}

#[expect(
    clippy::needless_range_loop,
    reason = "mirrors the indexing of the WebRTC Ooura FFT"
)]
fn bitrv2_128(a: &mut [f32; FFT_LEN]) {
    const IP: [usize; 4] = [0, 64, 32, 96];
    for k in 0..4 {
        for j in 0..k {
            let mut j1 = 2 * j + IP[k];
            let mut k1 = 2 * k + IP[j];
            swap_pair(a, j1, k1);
            j1 += 8;
            k1 += 16;
            swap_pair(a, j1, k1);
            j1 += 8;
            k1 -= 8;
            swap_pair(a, j1, k1);
            j1 += 8;
            k1 += 16;
            swap_pair(a, j1, k1);
        }
        let j1 = 2 * k + 8 + IP[k];
        swap_pair(a, j1, j1 + 8);
    }
}
