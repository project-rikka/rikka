pub(crate) const NUM_LPC_COEFFICIENTS: usize = 5;

pub(crate) fn lpc_coefficients(x: &[f32]) -> [f32; NUM_LPC_COEFFICIENTS] {
    let mut auto_corr = [0.0f32; NUM_LPC_COEFFICIENTS];
    for (lag, value) in auto_corr.iter_mut().enumerate() {
        *value = x[..x.len() - lag]
            .iter()
            .zip(&x[lag..])
            .fold(0.0, |sum, (a, b)| sum + a * b);
    }
    if auto_corr[0] == 0.0 {
        return [0.0; NUM_LPC_COEFFICIENTS];
    }
    denoise(&mut auto_corr);
    let mut pre = inverse_filter(&auto_corr);
    pre[0] *= 0.9;
    pre[1] *= 0.9 * 0.9;
    pre[2] *= 0.9 * 0.9 * 0.9;
    pre[3] *= 0.9 * 0.9 * 0.9 * 0.9;
    const C: f32 = 0.8;
    [
        pre[0] + C,
        pre[1] + C * pre[0],
        pre[2] + C * pre[1],
        pre[3] + C * pre[2],
        C * pre[3],
    ]
}

pub(crate) fn lp_residual(lpc: &[f32; NUM_LPC_COEFFICIENTS], x: &[f32], y: &mut [f32]) {
    y[0] = x[0];
    for i in 1..x.len() {
        let taps = i.min(NUM_LPC_COEFFICIENTS);
        let mut value = x[i];
        for (k, coefficient) in lpc[..taps].iter().enumerate() {
            value += x[i - 1 - k] * coefficient;
        }
        y[i] = value;
    }
}

fn denoise(auto_corr: &mut [f32; NUM_LPC_COEFFICIENTS]) {
    auto_corr[0] *= 1.0001;
    auto_corr[1] -= auto_corr[1] * 0.000_064;
    auto_corr[2] -= auto_corr[2] * 0.000_256;
    auto_corr[3] -= auto_corr[3] * 0.000_576;
    auto_corr[4] -= auto_corr[4] * 0.001_024;
}

fn inverse_filter(auto_corr: &[f32; NUM_LPC_COEFFICIENTS]) -> [f32; NUM_LPC_COEFFICIENTS - 1] {
    const MIN_ERROR_MAGNITUDE: f32 = 1e-6;
    let mut lpc = [0.0f32; NUM_LPC_COEFFICIENTS - 1];
    let mut error = auto_corr[0];
    for i in 0..NUM_LPC_COEFFICIENTS - 1 {
        let mut reflection = 0.0f32;
        for j in 0..i {
            reflection += lpc[j] * auto_corr[i - j];
        }
        reflection += auto_corr[i + 1];
        if error.abs() < MIN_ERROR_MAGNITUDE {
            error = MIN_ERROR_MAGNITUDE.copysign(error);
        }
        reflection /= -error;
        lpc[i] = reflection;
        for j in 0..(i + 1) >> 1 {
            let tmp1 = lpc[j];
            let tmp2 = lpc[i - 1 - j];
            lpc[j] = tmp1 + reflection * tmp2;
            lpc[i - 1 - j] = tmp2 + reflection * tmp1;
        }
        error -= reflection * reflection * error;
        if error < 0.001 * auto_corr[0] {
            break;
        }
    }
    lpc
}
