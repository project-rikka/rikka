const MINUS_SQRT2: f32 = -std::f32::consts::SQRT_2;

fn mul(ar: f32, ai: f32, br: f32, bi: f32) -> (f32, f32) {
    let tmp = ar * bi;
    (ar * br - ai * bi, ai * br + tmp)
}

pub(crate) fn radix2(ido: usize, l1: usize, cc: &[f32], ch: &mut [f32], wa: &[f32]) {
    let l1ido = l1 * ido;
    for k in (0..l1ido).step_by(ido) {
        let a = cc[2 * k];
        let b = cc[2 * (k + ido) - 1];
        ch[k] = a + b;
        ch[k + l1ido] = a - b;
    }
    if ido < 2 {
        return;
    }
    if ido != 2 {
        for k in (0..l1ido).step_by(ido) {
            for i in (2..ido).step_by(2) {
                let a = cc[i - 1 + 2 * k];
                let b = cc[2 * (k + ido) - i - 1];
                let c = cc[i + 2 * k];
                let d = cc[2 * (k + ido) - i];
                ch[i - 1 + k] = a + b;
                ch[i + k] = c - d;
                let (tr2, ti2) = mul(a - b, c + d, wa[i - 2], wa[i - 1]);
                ch[i - 1 + k + l1ido] = tr2;
                ch[i + k + l1ido] = ti2;
            }
        }
        if ido % 2 == 1 {
            return;
        }
    }
    for k in (0..l1ido).step_by(ido) {
        let a = cc[2 * k + ido - 1];
        let b = cc[2 * k + ido];
        ch[k + ido - 1] = a + a;
        ch[k + ido - 1 + l1ido] = -2.0 * b;
    }
}

pub(crate) fn radix4(ido: usize, l1: usize, cc: &[f32], ch: &mut [f32], wa: &[f32]) {
    let (wa1, wa2, wa3) = (wa, &wa[ido..], &wa[2 * ido..]);
    let l1ido = l1 * ido;
    for k in 0..l1 {
        let c = 4 * k * ido;
        let h = k * ido;
        let a = cc[c];
        let b = cc[c + 4 * ido - 1];
        let cv = cc[c + 2 * ido];
        let d = cc[c + 2 * ido - 1];
        let tr3 = 2.0 * d;
        let tr2 = a + b;
        let tr1 = a - b;
        let tr4 = 2.0 * cv;
        ch[h] = tr2 + tr3;
        ch[h + 2 * l1ido] = tr2 - tr3;
        ch[h + l1ido] = tr1 - tr4;
        ch[h + 3 * l1ido] = tr1 + tr4;
    }
    if ido < 2 {
        return;
    }
    if ido != 2 {
        for k in (0..l1ido).step_by(ido) {
            for i in (2..ido).step_by(2) {
                let pc = |offset: usize| cc[4 * k + offset - 1];
                let ph = k + i - 1;
                let tr1 = pc(i) - pc(4 * ido - i);
                let tr2 = pc(i) + pc(4 * ido - i);
                let ti4 = pc(2 * ido + i) - pc(2 * ido - i);
                let tr3 = pc(2 * ido + i) + pc(2 * ido - i);
                ch[ph] = tr2 + tr3;
                let cr3 = tr2 - tr3;
                let ti3 = pc(2 * ido + i + 1) - pc(2 * ido - i + 1);
                let tr4 = pc(2 * ido + i + 1) + pc(2 * ido - i + 1);
                let cr2 = tr1 - tr4;
                let cr4 = tr1 + tr4;
                let ti1 = pc(i + 1) + pc(4 * ido - i + 1);
                let ti2 = pc(i + 1) - pc(4 * ido - i + 1);
                ch[ph + 1] = ti2 + ti3;
                let ci3 = ti2 - ti3;
                let ci2 = ti1 + ti4;
                let ci4 = ti1 - ti4;
                let (cr2, ci2) = mul(cr2, ci2, wa1[i - 2], wa1[i - 1]);
                ch[ph + l1ido] = cr2;
                ch[ph + l1ido + 1] = ci2;
                let (cr3, ci3) = mul(cr3, ci3, wa2[i - 2], wa2[i - 1]);
                ch[ph + 2 * l1ido] = cr3;
                ch[ph + 2 * l1ido + 1] = ci3;
                let (cr4, ci4) = mul(cr4, ci4, wa3[i - 2], wa3[i - 1]);
                ch[ph + 3 * l1ido] = cr4;
                ch[ph + 3 * l1ido + 1] = ci4;
            }
        }
        if ido % 2 == 1 {
            return;
        }
    }
    for k in (0..l1ido).step_by(ido) {
        let i0 = 4 * k + ido;
        let c = cc[i0 - 1];
        let d = cc[i0 + 2 * ido - 1];
        let a = cc[i0];
        let b = cc[i0 + 2 * ido];
        let tr1 = c - d;
        let tr2 = c + d;
        let ti1 = b + a;
        let ti2 = b - a;
        ch[ido - 1 + k] = tr2 + tr2;
        ch[ido - 1 + k + l1ido] = MINUS_SQRT2 * (ti1 - tr1);
        ch[ido - 1 + k + 2 * l1ido] = ti2 + ti2;
        ch[ido - 1 + k + 3 * l1ido] = MINUS_SQRT2 * (ti1 + tr1);
    }
}
