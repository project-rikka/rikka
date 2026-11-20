use std::f32::consts::FRAC_PI_4;

use crate::common::{BINS, FFT_SIZE};

const WORDS: usize = FFT_SIZE >> 2;
const COSINES: usize = FFT_SIZE >> 2;
const REVERSAL: usize = 8;

pub(crate) struct Fft {
    reversal: [usize; REVERSAL],
    table: [f32; WORDS + COSINES],
}

impl Fft {
    pub(crate) fn new() -> Fft {
        let mut table = [0.0; WORDS + COSINES];
        let (words, cosines) = table.split_at_mut(WORDS);
        make_twiddles(words);
        make_cosines(cosines);
        let mut reversal = [0; REVERSAL];
        reversal_indices(FFT_SIZE, &mut reversal);
        Fft { reversal, table }
    }

    pub(crate) fn forward(
        &self,
        time: &mut [f32; FFT_SIZE],
        real: &mut [f32; FFT_SIZE],
        imag: &mut [f32; FFT_SIZE],
    ) {
        self.rdft_forward(time);
        imag[0] = 0.0;
        real[0] = time[0];
        imag[BINS - 1] = 0.0;
        real[BINS - 1] = time[1];
        for i in 1..BINS - 1 {
            real[i] = time[2 * i];
            imag[i] = time[2 * i + 1];
        }
    }

    pub(crate) fn inverse(
        &self,
        real: &[f32; FFT_SIZE],
        imag: &[f32; FFT_SIZE],
        time: &mut [f32; FFT_SIZE],
    ) {
        const SCALING: f32 = 2.0 / FFT_SIZE as f32;
        time[0] = real[0];
        time[1] = real[BINS - 1];
        for i in 1..BINS - 1 {
            time[2 * i] = real[i];
            time[2 * i + 1] = imag[i];
        }
        self.rdft_inverse(time);
        for sample in time.iter_mut() {
            *sample *= SCALING;
        }
    }

    fn rdft_forward(&self, a: &mut [f32; FFT_SIZE]) {
        let (words, cosines) = self.table.split_at(WORDS);
        permute(FFT_SIZE, &self.reversal, a);
        complex_forward(a, words);
        real_forward(a, cosines);
        let xi = a[0] - a[1];
        a[0] += a[1];
        a[1] = xi;
    }

    fn rdft_inverse(&self, a: &mut [f32; FFT_SIZE]) {
        let (words, cosines) = self.table.split_at(WORDS);
        a[1] = 0.5 * (a[0] - a[1]);
        a[0] -= a[1];
        real_backward(a, cosines);
        permute(FFT_SIZE, &self.reversal, a);
        complex_backward(a, words);
    }
}

fn make_twiddles(w: &mut [f32]) {
    let nw = w.len();
    let nwh = nw >> 1;
    let delta = FRAC_PI_4 / nwh as f32;
    w[0] = 1.0;
    w[1] = 0.0;
    w[nwh] = (delta * nwh as f32).cos();
    w[nwh + 1] = w[nwh];
    for j in (2..nwh).step_by(2) {
        let x = (delta * j as f32).cos();
        let y = (delta * j as f32).sin();
        w[j] = x;
        w[j + 1] = y;
        w[nw - j] = y;
        w[nw - j + 1] = x;
    }
    let mut reversal = [0; REVERSAL];
    reversal_indices(nw, &mut reversal);
    permute(nw, &reversal, w);
}

fn make_cosines(c: &mut [f32]) {
    let nc = c.len();
    let nch = nc >> 1;
    let delta = FRAC_PI_4 / nch as f32;
    c[0] = (delta * nch as f32).cos();
    c[nch] = 0.5 * c[0];
    for j in 1..nch {
        c[j] = 0.5 * (delta * j as f32).cos();
        c[nc - j] = 0.5 * (delta * j as f32).sin();
    }
}

fn reversal_indices(n: usize, ip: &mut [usize]) -> (usize, usize) {
    ip[0] = 0;
    let mut l = n;
    let mut m = 1;
    while (m << 3) < l {
        l >>= 1;
        for j in 0..m {
            ip[m + j] = ip[j] + l;
        }
        m <<= 1;
    }
    (m, l)
}

fn swap_pair(a: &mut [f32], j1: usize, k1: usize) {
    a.swap(j1, k1);
    a.swap(j1 + 1, k1 + 1);
}

fn permute(n: usize, ip: &[usize], a: &mut [f32]) {
    let mut l = n;
    let mut m = 1;
    while (m << 3) < l {
        l >>= 1;
        m <<= 1;
    }
    let m2 = 2 * m;
    if (m << 3) == l {
        for k in 0..m {
            for j in 0..k {
                let mut j1 = 2 * j + ip[k];
                let mut k1 = 2 * k + ip[j];
                swap_pair(a, j1, k1);
                j1 += m2;
                k1 += 2 * m2;
                swap_pair(a, j1, k1);
                j1 += m2;
                k1 -= m2;
                swap_pair(a, j1, k1);
                j1 += m2;
                k1 += 2 * m2;
                swap_pair(a, j1, k1);
            }
            let j1 = 2 * k + m2 + ip[k];
            swap_pair(a, j1, j1 + m2);
        }
    } else {
        for k in 1..m {
            for j in 0..k {
                let j1 = 2 * j + ip[k];
                let k1 = 2 * k + ip[j];
                swap_pair(a, j1, k1);
                swap_pair(a, j1 + m2, k1 + m2);
            }
        }
    }
}

fn complex_forward(a: &mut [f32], w: &[f32]) {
    let n = a.len();
    let mut l = 2;
    while (l << 2) < n {
        complex_stage(n, l, a, w);
        l <<= 2;
    }
    if (l << 2) == n {
        for j in (0..l).step_by(2) {
            let j1 = j + l;
            let j2 = j1 + l;
            let j3 = j2 + l;
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
    } else {
        for j in (0..l).step_by(2) {
            let j1 = j + l;
            let x0r = a[j] - a[j1];
            let x0i = a[j + 1] - a[j1 + 1];
            a[j] += a[j1];
            a[j + 1] += a[j1 + 1];
            a[j1] = x0r;
            a[j1 + 1] = x0i;
        }
    }
}

fn complex_backward(a: &mut [f32], w: &[f32]) {
    let n = a.len();
    let mut l = 2;
    while (l << 2) < n {
        complex_stage(n, l, a, w);
        l <<= 2;
    }
    if (l << 2) == n {
        for j in (0..l).step_by(2) {
            let j1 = j + l;
            let j2 = j1 + l;
            let j3 = j2 + l;
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
    } else {
        for j in (0..l).step_by(2) {
            let j1 = j + l;
            let x0r = a[j] - a[j1];
            let x0i = -a[j + 1] + a[j1 + 1];
            a[j] += a[j1];
            a[j + 1] = -a[j + 1] - a[j1 + 1];
            a[j1] = x0r;
            a[j1 + 1] = x0i;
        }
    }
}

fn complex_stage(n: usize, l: usize, a: &mut [f32], w: &[f32]) {
    let m = l << 2;
    for j in (0..l).step_by(2) {
        let j1 = j + l;
        let j2 = j1 + l;
        let j3 = j2 + l;
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
    let wk1r = w[2];
    for j in (m..l + m).step_by(2) {
        let j1 = j + l;
        let j2 = j1 + l;
        let j3 = j2 + l;
        let mut x0r = a[j] + a[j1];
        let mut x0i = a[j + 1] + a[j1 + 1];
        let x1r = a[j] - a[j1];
        let x1i = a[j + 1] - a[j1 + 1];
        let x2r = a[j2] + a[j3];
        let x2i = a[j2 + 1] + a[j3 + 1];
        let x3r = a[j2] - a[j3];
        let x3i = a[j2 + 1] - a[j3 + 1];
        a[j] = x0r + x2r;
        a[j + 1] = x0i + x2i;
        a[j2] = x2i - x0i;
        a[j2 + 1] = x0r - x2r;
        x0r = x1r - x3i;
        x0i = x1i + x3r;
        a[j1] = wk1r * (x0r - x0i);
        a[j1 + 1] = wk1r * (x0r + x0i);
        x0r = x3i + x1r;
        x0i = x3r - x1i;
        a[j3] = wk1r * (x0i - x0r);
        a[j3 + 1] = wk1r * (x0i + x0r);
    }
    let mut k1 = 0;
    let m2 = 2 * m;
    for k in (m2..n).step_by(m2) {
        k1 += 2;
        let k2 = 2 * k1;
        let wk2 = [w[k1], w[k1 + 1]];
        let wk1 = [w[k2], w[k2 + 1]];
        let wk3 = [
            wk1[0] - 2.0 * wk2[1] * wk1[1],
            2.0 * wk2[1] * wk1[0] - wk1[1],
        ];
        for j in (k..l + k).step_by(2) {
            butterfly(a, j, l, wk1, wk2, wk3);
        }
        let wk1 = [w[k2 + 2], w[k2 + 3]];
        let wk3 = [
            wk1[0] - 2.0 * wk2[0] * wk1[1],
            2.0 * wk2[0] * wk1[0] - wk1[1],
        ];
        for j in (k + m..l + (k + m)).step_by(2) {
            butterfly(a, j, l, wk1, [-wk2[1], wk2[0]], wk3);
        }
    }
}

fn butterfly(a: &mut [f32], j: usize, l: usize, wk1: [f32; 2], wk2: [f32; 2], wk3: [f32; 2]) {
    let j1 = j + l;
    let j2 = j1 + l;
    let j3 = j2 + l;
    let mut x0r = a[j] + a[j1];
    let mut x0i = a[j + 1] + a[j1 + 1];
    let x1r = a[j] - a[j1];
    let x1i = a[j + 1] - a[j1 + 1];
    let x2r = a[j2] + a[j3];
    let x2i = a[j2 + 1] + a[j3 + 1];
    let x3r = a[j2] - a[j3];
    let x3i = a[j2 + 1] - a[j3 + 1];
    a[j] = x0r + x2r;
    a[j + 1] = x0i + x2i;
    x0r -= x2r;
    x0i -= x2i;
    a[j2] = wk2[0] * x0r - wk2[1] * x0i;
    a[j2 + 1] = wk2[0] * x0i + wk2[1] * x0r;
    x0r = x1r - x3i;
    x0i = x1i + x3r;
    a[j1] = wk1[0] * x0r - wk1[1] * x0i;
    a[j1 + 1] = wk1[0] * x0i + wk1[1] * x0r;
    x0r = x1r + x3i;
    x0i = x1i - x3r;
    a[j3] = wk3[0] * x0r - wk3[1] * x0i;
    a[j3 + 1] = wk3[0] * x0i + wk3[1] * x0r;
}

fn real_forward(a: &mut [f32], c: &[f32]) {
    let n = a.len();
    let nc = c.len();
    let m = n >> 1;
    let ks = 2 * nc / m;
    let mut kk = 0;
    for j in (2..m).step_by(2) {
        let k = n - j;
        kk += ks;
        let wkr = 0.5 - c[nc - kk];
        let wki = c[kk];
        let xr = a[j] - a[k];
        let xi = a[j + 1] + a[k + 1];
        let yr = wkr * xr - wki * xi;
        let yi = wkr * xi + wki * xr;
        a[j] -= yr;
        a[j + 1] -= yi;
        a[k] += yr;
        a[k + 1] -= yi;
    }
}

fn real_backward(a: &mut [f32], c: &[f32]) {
    let n = a.len();
    let nc = c.len();
    a[1] = -a[1];
    let m = n >> 1;
    let ks = 2 * nc / m;
    let mut kk = 0;
    for j in (2..m).step_by(2) {
        let k = n - j;
        kk += ks;
        let wkr = 0.5 - c[nc - kk];
        let wki = c[kk];
        let xr = a[j] - a[k];
        let xi = a[j + 1] + a[k + 1];
        let yr = wkr * xr + wki * xi;
        let yi = wkr * xi - wki * xr;
        a[j] -= yr;
        a[j + 1] = yi - a[j + 1];
        a[k] += yr;
        a[k + 1] = yi - a[k + 1];
    }
    a[m + 1] = -a[m + 1];
}
