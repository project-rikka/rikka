const TAUR: f32 = -0.5;
const TAUI: f32 = 0.866_025_4;
const MINUS_HSQT2: f32 = -0.707_106_781_186_547_5_f64 as f32;
const TR11: f32 = 0.309_017;
const TI11: f32 = 0.951_056_54;
const TR12: f32 = -0.809_017;
const TI12: f32 = 0.587_785_24;

fn mul_conj(ar: f32, ai: f32, br: f32, bi: f32) -> (f32, f32) {
    let tmp = ar * bi;
    (ar * br + ai * bi, ai * br - tmp)
}

pub(crate) fn radix2(ido: usize, l1: usize, cc: &[f32], ch: &mut [f32], wa: &[f32]) {
    let l1ido = l1 * ido;
    for k in (0..l1ido).step_by(ido) {
        let a = cc[k];
        let b = cc[k + l1ido];
        ch[2 * k] = a + b;
        ch[2 * (k + ido) - 1] = a - b;
    }
    if ido < 2 {
        return;
    }
    if ido != 2 {
        for k in (0..l1ido).step_by(ido) {
            for i in (2..ido).step_by(2) {
                let (tr2, ti2) = mul_conj(
                    cc[i - 1 + k + l1ido],
                    cc[i + k + l1ido],
                    wa[i - 2],
                    wa[i - 1],
                );
                let br = cc[i - 1 + k];
                let bi = cc[i + k];
                ch[i + 2 * k] = bi + ti2;
                ch[2 * (k + ido) - i] = ti2 - bi;
                ch[i - 1 + 2 * k] = br + tr2;
                ch[2 * (k + ido) - i - 1] = br - tr2;
            }
        }
        if ido % 2 == 1 {
            return;
        }
    }
    for k in (0..l1ido).step_by(ido) {
        ch[2 * k + ido] = -cc[ido - 1 + k + l1ido];
        ch[2 * k + ido - 1] = cc[k + ido - 1];
    }
}

pub(crate) fn radix3(ido: usize, l1: usize, cc: &[f32], ch: &mut [f32], wa: &[f32]) {
    let (wa1, wa2) = (wa, &wa[ido..]);
    for k in 0..l1 {
        let cr2 = cc[(k + l1) * ido] + cc[(k + 2 * l1) * ido];
        ch[3 * k * ido] = cc[k * ido] + cr2;
        ch[(3 * k + 2) * ido] = TAUI * (cc[(k + l1 * 2) * ido] - cc[(k + l1) * ido]);
        ch[ido - 1 + (3 * k + 1) * ido] = cc[k * ido] + TAUR * cr2;
    }
    if ido == 1 {
        return;
    }
    for k in 0..l1 {
        for i in (2..ido).step_by(2) {
            let ic = ido - i;
            let (dr2, di2) = mul_conj(
                cc[i - 1 + (k + l1) * ido],
                cc[i + (k + l1) * ido],
                wa1[i - 2],
                wa1[i - 1],
            );
            let (dr3, di3) = mul_conj(
                cc[i - 1 + (k + l1 * 2) * ido],
                cc[i + (k + l1 * 2) * ido],
                wa2[i - 2],
                wa2[i - 1],
            );
            let cr2 = dr2 + dr3;
            let ci2 = di2 + di3;
            ch[i - 1 + 3 * k * ido] = cc[i - 1 + k * ido] + cr2;
            ch[i + 3 * k * ido] = cc[i + k * ido] + ci2;
            let tr2 = cc[i - 1 + k * ido] + TAUR * cr2;
            let ti2 = cc[i + k * ido] + TAUR * ci2;
            let tr3 = TAUI * (di2 - di3);
            let ti3 = TAUI * (dr3 - dr2);
            ch[i - 1 + (3 * k + 2) * ido] = tr2 + tr3;
            ch[ic - 1 + (3 * k + 1) * ido] = tr2 - tr3;
            ch[i + (3 * k + 2) * ido] = ti2 + ti3;
            ch[ic + (3 * k + 1) * ido] = ti3 - ti2;
        }
    }
}

pub(crate) fn radix4(ido: usize, l1: usize, cc: &[f32], ch: &mut [f32], wa: &[f32]) {
    let (wa1, wa2, wa3) = (wa, &wa[ido..], &wa[2 * ido..]);
    let l1ido = l1 * ido;
    for k in 0..l1 {
        let c = k * ido;
        let h = 4 * k * ido;
        let a0 = cc[c];
        let a1 = cc[c + l1ido];
        let a2 = cc[c + 2 * l1ido];
        let a3 = cc[c + 3 * l1ido];
        let tr1 = a1 + a3;
        let tr2 = a0 + a2;
        ch[h + 2 * ido - 1] = a0 - a2;
        ch[h + 2 * ido] = a3 - a1;
        ch[h] = tr1 + tr2;
        ch[h + 4 * ido - 1] = tr2 - tr1;
    }
    if ido < 2 {
        return;
    }
    if ido != 2 {
        for k in (0..l1ido).step_by(ido) {
            for i in (2..ido).step_by(2) {
                let pc = k + i - 1;
                let ic = ido - i;
                let (cr2, ci2) =
                    mul_conj(cc[pc + l1ido], cc[pc + l1ido + 1], wa1[i - 2], wa1[i - 1]);
                let (cr3, ci3) = mul_conj(
                    cc[pc + 2 * l1ido],
                    cc[pc + 2 * l1ido + 1],
                    wa2[i - 2],
                    wa2[i - 1],
                );
                let (cr4, ci4) = mul_conj(
                    cc[pc + 3 * l1ido],
                    cc[pc + 3 * l1ido + 1],
                    wa3[i - 2],
                    wa3[i - 1],
                );
                let tr1 = cr2 + cr4;
                let tr4 = cr4 - cr2;
                let tr2 = cc[pc] + cr3;
                let tr3 = cc[pc] - cr3;
                ch[i - 1 + 4 * k] = tr1 + tr2;
                ch[ic - 1 + 4 * k + 3 * ido] = tr2 - tr1;
                let ti1 = ci2 + ci4;
                let ti4 = ci2 - ci4;
                ch[i - 1 + 4 * k + 2 * ido] = ti4 + tr3;
                ch[ic - 1 + 4 * k + ido] = tr3 - ti4;
                let ti2 = cc[pc + 1] + ci3;
                let ti3 = cc[pc + 1] - ci3;
                ch[i + 4 * k] = ti1 + ti2;
                ch[ic + 4 * k + 3 * ido] = ti1 - ti2;
                ch[i + 4 * k + 2 * ido] = tr4 + ti3;
                ch[ic + 4 * k + ido] = tr4 - ti3;
            }
        }
        if ido % 2 == 1 {
            return;
        }
    }
    for k in (0..l1ido).step_by(ido) {
        let a = cc[ido - 1 + k + l1ido];
        let b = cc[ido - 1 + k + 3 * l1ido];
        let c = cc[ido - 1 + k];
        let d = cc[ido - 1 + k + 2 * l1ido];
        let ti1 = MINUS_HSQT2 * (a + b);
        let tr1 = MINUS_HSQT2 * (b - a);
        ch[ido - 1 + 4 * k] = tr1 + c;
        ch[ido - 1 + 4 * k + 2 * ido] = c - tr1;
        ch[4 * k + ido] = ti1 - d;
        ch[4 * k + 3 * ido] = ti1 + d;
    }
}

pub(crate) fn radix5(ido: usize, l1: usize, cc: &[f32], ch: &mut [f32], wa: &[f32]) {
    let (wa1, wa2, wa3, wa4) = (wa, &wa[ido..], &wa[2 * ido..], &wa[3 * ido..]);
    let cc_at =
        |a1: usize, a2: usize, a3: usize| ((a3 * l1 + a2) * ido + a1) - (1 + ido * (1 + l1));
    let ch_at = |a1: usize, a2: usize, a3: usize| ((a3 * 5 + a2) * ido + a1) - (1 + ido * 6);
    for k in 1..=l1 {
        let cr2 = cc[cc_at(1, k, 5)] + cc[cc_at(1, k, 2)];
        let ci5 = cc[cc_at(1, k, 5)] - cc[cc_at(1, k, 2)];
        let cr3 = cc[cc_at(1, k, 4)] + cc[cc_at(1, k, 3)];
        let ci4 = cc[cc_at(1, k, 4)] - cc[cc_at(1, k, 3)];
        let c1 = cc[cc_at(1, k, 1)];
        ch[ch_at(1, 1, k)] = c1 + (cr2 + cr3);
        ch[ch_at(ido, 2, k)] = c1 + (TR11 * cr2 + TR12 * cr3);
        ch[ch_at(1, 3, k)] = TI11 * ci5 + TI12 * ci4;
        ch[ch_at(ido, 4, k)] = c1 + (TR12 * cr2 + TR11 * cr3);
        ch[ch_at(1, 5, k)] = TI12 * ci5 - TI11 * ci4;
    }
    if ido == 1 {
        return;
    }
    let idp2 = ido + 2;
    for k in 1..=l1 {
        for i in (3..=ido).step_by(2) {
            let ic = idp2 - i;
            let (dr2, di2) = mul_conj(
                wa1[i - 3],
                wa1[i - 2],
                cc[cc_at(i - 1, k, 2)],
                cc[cc_at(i, k, 2)],
            );
            let (dr3, di3) = mul_conj(
                wa2[i - 3],
                wa2[i - 2],
                cc[cc_at(i - 1, k, 3)],
                cc[cc_at(i, k, 3)],
            );
            let (dr4, di4) = mul_conj(
                wa3[i - 3],
                wa3[i - 2],
                cc[cc_at(i - 1, k, 4)],
                cc[cc_at(i, k, 4)],
            );
            let (dr5, di5) = mul_conj(
                wa4[i - 3],
                wa4[i - 2],
                cc[cc_at(i - 1, k, 5)],
                cc[cc_at(i, k, 5)],
            );
            let cr2 = dr2 + dr5;
            let ci5 = dr5 - dr2;
            let cr5 = di2 - di5;
            let ci2 = di2 + di5;
            let cr3 = dr3 + dr4;
            let ci4 = dr4 - dr3;
            let cr4 = di3 - di4;
            let ci3 = di3 + di4;
            let real = cc[cc_at(i - 1, k, 1)];
            let imaginary = cc[cc_at(i, k, 1)];
            ch[ch_at(i - 1, 1, k)] = real + (cr2 + cr3);
            ch[ch_at(i, 1, k)] = imaginary - (ci2 + ci3);
            let tr2 = real + (TR11 * cr2 + TR12 * cr3);
            let ti2 = imaginary - (TR11 * ci2 + TR12 * ci3);
            let tr3 = real + (TR12 * cr2 + TR11 * cr3);
            let ti3 = imaginary - (TR12 * ci2 + TR11 * ci3);
            let tr5 = TI11 * cr5 + TI12 * cr4;
            let ti5 = TI11 * ci5 + TI12 * ci4;
            let tr4 = TI12 * cr5 - TI11 * cr4;
            let ti4 = TI12 * ci5 - TI11 * ci4;
            ch[ch_at(i - 1, 3, k)] = tr2 - tr5;
            ch[ch_at(ic - 1, 2, k)] = tr2 + tr5;
            ch[ch_at(i, 3, k)] = ti2 + ti5;
            ch[ch_at(ic, 2, k)] = ti5 - ti2;
            ch[ch_at(i - 1, 5, k)] = tr3 - tr4;
            ch[ch_at(ic - 1, 4, k)] = tr3 + tr4;
            ch[ch_at(i, 5, k)] = ti3 + ti4;
            ch[ch_at(ic, 4, k)] = ti4 - ti3;
        }
    }
}
