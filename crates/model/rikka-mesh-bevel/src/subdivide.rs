use glam::Vec3;

use crate::math::{EPSILON, bilinear_quad};
use crate::model::Grid;
use crate::state::Bevel;

fn avg4(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> Vec3 {
    let mut sum = a + b;
    sum += c;
    sum += d;
    sum * 0.25
}

fn sabin_gamma(n: usize) -> f32 {
    match n {
        0..=2 => 0.0,
        3 => 0.065_247_58,
        4 => 0.25,
        5 => 0.401_983_44,
        6 => 0.523_423_25,
        _ => {
            let k = (std::f64::consts::PI / n as f64).cos();
            let k2 = k * k;
            let k4 = k2 * k2;
            let k6 = k4 * k2;
            let y = (3.0f64.sqrt() * (64.0 * k6 - 144.0 * k4 + 135.0 * k2 - 27.0).sqrt() + 9.0 * k)
                .powf(1.0 / 3.0);
            let x = 0.480_749_856_769_136 * y - (0.231_120_424_783_545 * (12.0 * k2 - 9.0)) / y;
            ((k * x + 2.0 * k2 - 1.0) / (x * x * (k * x + 1.0))) as f32
        }
    }
}

fn smooth_boundary(co: Vec3, before: Vec3, after: Vec3) -> Vec3 {
    let mut acc = before + after;
    acc += co * -2.0;
    co + acc * (-1.0 / 6.0)
}

fn fill_grid_fracs(grid: &Grid, i: usize) -> Vec<f32> {
    let ns = grid.seg;
    let mut fracs = vec![0.0f32; ns + 1];
    let mut total = 0.0f32;
    for k in 0..ns {
        total += grid.co(i, 0, k).distance(grid.co(i, 0, k + 1));
        fracs[k + 1] = total;
    }
    normalize_fracs(&mut fracs, total);
    fracs
}

fn normalize_fracs(fracs: &mut [f32], total: f32) {
    let ns = fracs.len() - 1;
    if total > 0.0 {
        for frac in fracs.iter_mut().skip(1) {
            *frac /= total;
        }
    } else {
        fracs[ns] = 1.0;
    }
}

fn interp_range(fracs: &[f32], n: usize, f: f32) -> (usize, f32) {
    for i in 0..n {
        if f <= fracs[i + 1] {
            let rest = f - fracs[i];
            let mut rest = match rest == 0.0 {
                true => 0.0,
                false => rest / (fracs[i + 1] - fracs[i]),
            };
            let mut index = i;
            if i == n - 1 && rest == 1.0 {
                index = n;
                rest = 0.0;
            }
            return (index, rest);
        }
    }
    (n, 0.0)
}

impl Bevel<'_> {
    fn fill_profile_fracs(&self, bound: crate::model::BoundId, ns: usize) -> Vec<f32> {
        let mut fracs = vec![0.0f32; ns + 1];
        let mut total = 0.0f32;
        let mut co = self.bound(bound).co;
        for k in 0..ns {
            let next = self.profile_point(&self.bound(bound).profile, k + 1, ns);
            total += co.distance(next);
            fracs[k + 1] = total;
            co = next;
        }
        normalize_fracs(&mut fracs, total);
        fracs
    }

    pub(crate) fn interp_vmesh(&self, input: &Grid, nseg: usize) -> Grid {
        let n_bndv = input.count;
        let ns_in = input.seg;
        let nseg2 = nseg / 2;
        let odd = nseg % 2;
        let mut output = Grid::new(n_bndv, nseg, input.boundstart);
        let mut prev_frac = fill_grid_fracs(input, n_bndv - 1);
        let start = input.boundstart.expect("interpolated grid has a boundary");
        let mut bound = start;
        let mut prev_new_frac = self.fill_profile_fracs(self.bound(bound).prev, nseg);
        for i in 0..n_bndv {
            let frac = fill_grid_fracs(input, i);
            let new_frac = self.fill_profile_fracs(bound, nseg);
            for j in 0..nseg2 + odd {
                for (k, &fraction) in new_frac.iter().enumerate().take(nseg2 + 1) {
                    let (k_in, restk) = interp_range(&frac, ns_in, fraction);
                    let (k_in_prev, restkprev) =
                        interp_range(&prev_frac, ns_in, prev_new_frac[nseg - j]);
                    let mut j_in = ns_in - k_in_prev;
                    let mut restj = -restkprev;
                    if restj > -EPSILON {
                        restj = 0.0;
                    } else {
                        j_in -= 1;
                        restj += 1.0;
                    }
                    let co = if restj < EPSILON && restk < EPSILON {
                        input.canon_co(i, j_in, k_in)
                    } else {
                        let j0inc = usize::from(!(restj < EPSILON || j_in == ns_in));
                        let k0inc = usize::from(!(restk < EPSILON || k_in == ns_in));
                        let quad = [
                            input.canon_co(i, j_in, k_in),
                            input.canon_co(i, j_in, k_in + k0inc),
                            input.canon_co(i, j_in + j0inc, k_in + k0inc),
                            input.canon_co(i, j_in + j0inc, k_in),
                        ];
                        bilinear_quad(quad, restk, restj)
                    };
                    output.set_co(i, j, k, co);
                }
            }
            bound = self.bound(bound).next;
            prev_frac = frac;
            prev_new_frac = new_frac;
        }
        if odd == 0 {
            let center = input.center();
            output.set_co(0, nseg2, nseg2, center);
        }
        output.copy_equivalent();
        output
    }

    pub(crate) fn cubic_subdiv(&self, input: &mut Grid) -> Grid {
        let n_boundary = input.count;
        let ns_in = input.seg;
        let ns_in2 = ns_in / 2;
        let ns_out = 2 * ns_in;
        let custom = self.params.is_custom();
        let mut output = Grid::new(n_boundary, ns_out, input.boundstart);
        for i in 0..n_boundary {
            output.set_co(i, 0, 0, input.co(i, 0, 0));
            for k in 1..ns_in {
                let mut co = input.co(i, 0, k);
                if !custom {
                    co = smooth_boundary(co, input.co(i, 0, k - 1), input.co(i, 0, k + 1));
                }
                output.set_canon_co(i, 0, 2 * k, co);
            }
        }
        let start = output.boundstart.expect("subdivided grid has a boundary");
        let mut bound = start;
        for i in 0..n_boundary {
            for k in (1..ns_out).step_by(2) {
                let mut co = self.profile_point(&self.bound(bound).profile, k, ns_out);
                if !custom {
                    co = smooth_boundary(
                        co,
                        output.canon_co(i, 0, k - 1),
                        output.canon_co(i, 0, k + 1),
                    );
                }
                output.set_canon_co(i, 0, k, co);
            }
            bound = self.bound(bound).next;
        }
        output.copy_equivalent();
        for i in 0..n_boundary {
            for k in 0..ns_in {
                let co = output.co(i, 0, 2 * k);
                input.set_co(i, 0, k, co);
            }
        }
        input.copy_equivalent();
        for i in 0..n_boundary {
            for j in 0..ns_in2 {
                for k in 0..ns_in2 {
                    let co = avg4(
                        input.co(i, j, k),
                        input.co(i, j, k + 1),
                        input.co(i, j + 1, k),
                        input.co(i, j + 1, k + 1),
                    );
                    output.set_co(i, 2 * j + 1, 2 * k + 1, co);
                }
            }
        }
        for i in 0..n_boundary {
            for j in 0..ns_in2 {
                for k in 1..=ns_in2 {
                    let co = avg4(
                        input.co(i, j, k),
                        input.co(i, j + 1, k),
                        output.canon_co(i, 2 * j + 1, 2 * k - 1),
                        output.canon_co(i, 2 * j + 1, 2 * k + 1),
                    );
                    output.set_co(i, 2 * j + 1, 2 * k, co);
                }
            }
        }
        for i in 0..n_boundary {
            for j in 1..ns_in2 {
                for k in 0..ns_in2 {
                    let co = avg4(
                        input.co(i, j, k),
                        input.co(i, j, k + 1),
                        output.canon_co(i, 2 * j - 1, 2 * k + 1),
                        output.canon_co(i, 2 * j + 1, 2 * k + 1),
                    );
                    output.set_co(i, 2 * j, 2 * k + 1, co);
                }
            }
        }
        let gamma = 0.25f32;
        let beta = -gamma;
        for i in 0..n_boundary {
            for j in 1..ns_in2 {
                for k in 1..=ns_in2 {
                    let co1 = avg4(
                        output.canon_co(i, 2 * j, 2 * k - 1),
                        output.canon_co(i, 2 * j, 2 * k + 1),
                        output.canon_co(i, 2 * j - 1, 2 * k),
                        output.canon_co(i, 2 * j + 1, 2 * k),
                    );
                    let co2 = avg4(
                        output.canon_co(i, 2 * j - 1, 2 * k - 1),
                        output.canon_co(i, 2 * j + 1, 2 * k - 1),
                        output.canon_co(i, 2 * j - 1, 2 * k + 1),
                        output.canon_co(i, 2 * j + 1, 2 * k + 1),
                    );
                    let mut co = co1;
                    co += co2 * beta;
                    co += input.co(i, j, k) * gamma;
                    output.set_co(i, 2 * j, 2 * k, co);
                }
            }
        }
        output.copy_equivalent();
        let gamma = sabin_gamma(n_boundary);
        let beta = -gamma;
        let mut co1 = Vec3::ZERO;
        let mut co2 = Vec3::ZERO;
        for i in 0..n_boundary {
            co1 += output.co(i, ns_in, ns_in - 1);
            co2 += output.co(i, ns_in - 1, ns_in - 1);
            co2 += output.co(i, ns_in - 1, ns_in + 1);
        }
        let mut co = co1 * (1.0 / n_boundary as f32);
        co += co2 * (beta / (2.0 * n_boundary as f32));
        co += input.co(0, ns_in2, ns_in2) * gamma;
        for i in 0..n_boundary {
            output.set_co(i, ns_in, ns_in, co);
        }
        let mut bound = start;
        for i in 0..n_boundary {
            let inext = (i + 1) % n_boundary;
            for k in 0..=ns_out {
                let co = self.profile_point(&self.bound(bound).profile, k, ns_out);
                output.set_co(i, 0, k, co);
                if k >= ns_in && k < ns_out {
                    output.set_co(inext, ns_out - k, 0, co);
                }
            }
            bound = self.bound(bound).next;
        }
        output
    }
}
