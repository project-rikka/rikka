use std::f64::consts::PI;

use crate::{backward, forward};

const FACTOR_ORDER: [usize; 4] = [4, 2, 3, 5];

#[derive(Clone, Debug)]
pub struct RealFft {
    size: usize,
    twiddles: Vec<f32>,
    factors: Vec<usize>,
    work: [Vec<f32>; 2],
}

impl RealFft {
    #[must_use]
    pub fn new(size: usize) -> RealFft {
        let factors = decompose(size);
        let twiddles = twiddles(size, &factors);
        RealFft {
            size,
            twiddles,
            factors,
            work: [vec![0.0; size], vec![0.0; size]],
        }
    }

    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn forward(&mut self, input: &[f32], output: &mut [f32]) {
        let size = self.size;
        output.copy_from_slice(&self.transform(input)[..size]);
    }

    pub fn forward_ordered(&mut self, input: &[f32], output: &mut [f32]) {
        let size = self.size;
        let spectrum = self.transform(input);
        output[0] = spectrum[0];
        output[1] = spectrum[size - 1];
        output[2..].copy_from_slice(&spectrum[1..size - 1]);
    }

    fn transform(&mut self, input: &[f32]) -> &[f32] {
        let [first, second] = &mut self.work;
        first.copy_from_slice(input);
        let mut source = first;
        let mut target = second;
        let mut l2 = self.size;
        let mut iw = self.size - 1;
        for &ip in self.factors.iter().rev() {
            let l1 = l2 / ip;
            let ido = self.size / l2;
            iw -= (ip - 1) * ido;
            let wa = &self.twiddles[iw..];
            match ip {
                5 => forward::radix5(ido, l1, source, target, wa),
                4 => forward::radix4(ido, l1, source, target, wa),
                3 => forward::radix3(ido, l1, source, target, wa),
                _ => forward::radix2(ido, l1, source, target, wa),
            }
            l2 = l1;
            std::mem::swap(&mut source, &mut target);
        }
        source
    }

    pub fn backward(&mut self, data: &mut [f32]) {
        let [first, second] = &mut self.work;
        first.copy_from_slice(data);
        let mut source = first;
        let mut target = second;
        let mut l1 = 1;
        let mut iw = 0;
        for &ip in &self.factors {
            let l2 = ip * l1;
            let ido = self.size / l2;
            let wa = &self.twiddles[iw..];
            match ip {
                4 => backward::radix4(ido, l1, source, target, wa),
                2 => backward::radix2(ido, l1, source, target, wa),
                _ => unreachable!("only power-of-two sizes are transformed backward"),
            }
            l1 = l2;
            iw += (ip - 1) * ido;
            std::mem::swap(&mut source, &mut target);
        }
        data.copy_from_slice(source);
    }

    pub fn convolve_accumulate(&self, a: &[f32], b: &[f32], ab: &mut [f32], scaling: f32) {
        let last = self.size - 1;
        ab[0] += a[0] * b[0] * scaling;
        ab[last] += a[last] * b[last] * scaling;
        for k in 0..self.size / 2 - 1 {
            let (ar, ai) = (a[1 + 2 * k], a[2 + 2 * k]);
            let (br, bi) = (b[1 + 2 * k], b[2 + 2 * k]);
            let tmp = ar * bi;
            let real = ar * br - ai * bi;
            let imaginary = ai * br + tmp;
            ab[1 + 2 * k] += real * scaling;
            ab[2 + 2 * k] += imaginary * scaling;
        }
    }
}

fn decompose(size: usize) -> Vec<usize> {
    let mut factors: Vec<usize> = Vec::new();
    let mut remaining = size;
    for &factor in &FACTOR_ORDER {
        while remaining != 1 && remaining.is_multiple_of(factor) {
            factors.push(factor);
            remaining /= factor;
            if factor == 2 && factors.len() != 1 {
                factors.rotate_right(1);
            }
        }
    }
    assert_eq!(remaining, 1, "the FFT size must factor into 2, 3 and 5");
    factors
}

fn twiddles(size: usize, factors: &[usize]) -> Vec<f32> {
    let mut wa = vec![0.0f32; size];
    let argh = ((2.0 * PI) / size as f64) as f32;
    let mut is = 0;
    let mut l1 = 1;
    for &ip in &factors[..factors.len() - 1] {
        let mut ld = 0;
        let l2 = l1 * ip;
        let ido = size / l2;
        for _ in 1..ip {
            let mut i = is;
            let mut fi = 0;
            ld += l1;
            let argld = ld as f32 * argh;
            for _ in (3..=ido).step_by(2) {
                i += 2;
                fi += 1;
                let angle = f64::from(fi as f32 * argld);
                wa[i - 2] = angle.cos() as f32;
                wa[i - 1] = angle.sin() as f32;
            }
            is += ido;
        }
        l1 = l2;
    }
    wa
}
