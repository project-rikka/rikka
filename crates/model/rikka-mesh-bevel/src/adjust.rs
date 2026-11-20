use rikka_mesh_edit::VertId;

use crate::model::BoundId;
use crate::state::Bevel;

const MATCH_SPEC_WEIGHT: f64 = 0.2;

struct LeastSquares {
    rows: usize,
    columns: usize,
    matrix: Vec<f64>,
    rhs: Vec<f64>,
}

impl LeastSquares {
    fn new(rows: usize, columns: usize) -> LeastSquares {
        LeastSquares {
            rows,
            columns,
            matrix: vec![0.0; rows * columns],
            rhs: vec![0.0; rows],
        }
    }

    fn add(&mut self, row: usize, column: usize, value: f64) {
        self.matrix[row * self.columns + column] += value;
    }

    fn add_rhs(&mut self, row: usize, value: f64) {
        self.rhs[row] += value;
    }

    fn solve(&self) -> Vec<f64> {
        let n = self.columns;
        let mut normal = vec![0.0f64; n * n];
        let mut target = vec![0.0f64; n];
        for row in 0..self.rows {
            let values = &self.matrix[row * n..(row + 1) * n];
            for i in 0..n {
                if values[i] == 0.0 {
                    continue;
                }
                target[i] += values[i] * self.rhs[row];
                for j in 0..n {
                    normal[i * n + j] += values[i] * values[j];
                }
            }
        }
        for column in 0..n {
            let pivot = (column..n)
                .max_by(|&a, &b| {
                    normal[a * n + column]
                        .abs()
                        .total_cmp(&normal[b * n + column].abs())
                })
                .unwrap_or(column);
            if normal[pivot * n + column] == 0.0 {
                continue;
            }
            if pivot != column {
                for k in 0..n {
                    normal.swap(column * n + k, pivot * n + k);
                }
                target.swap(column, pivot);
            }
            let diagonal = normal[column * n + column];
            for row in column + 1..n {
                let factor = normal[row * n + column] / diagonal;
                if factor == 0.0 {
                    continue;
                }
                for k in column..n {
                    normal[row * n + k] -= factor * normal[column * n + k];
                }
                target[row] -= factor * target[column];
            }
        }
        let mut solution = vec![0.0f64; n];
        for row in (0..n).rev() {
            let diagonal = normal[row * n + row];
            if diagonal == 0.0 {
                continue;
            }
            let mut sum = target[row];
            for k in row + 1..n {
                sum -= normal[row * n + k] * solution[k];
            }
            solution[row] = sum / diagonal;
        }
        solution
    }
}

impl Bevel<'_> {
    fn chain(&self, start: BoundId) -> Vec<BoundId> {
        let mut chain = vec![start];
        let mut current = self.bound(start).adjchain;
        while let Some(next) = current {
            if next == start {
                break;
            }
            chain.push(next);
            current = self.bound(next).adjchain;
        }
        chain
    }

    fn adjust_cycle_or_chain(&mut self, start: BoundId, cycle: bool) {
        let chain = self.chain(start);
        let np = chain.len();
        let rows = match cycle {
            true => 3 * np,
            false => (3 * np).saturating_sub(3),
        };
        if rows == 0 {
            return;
        }
        let mut solver = LeastSquares::new(rows, np);
        let weight = MATCH_SPEC_WEIGHT;
        for (i, &v) in chain.iter().enumerate() {
            if cycle || i < np - 1 {
                let bound = self.bound(v);
                let eright = bound.efirst.expect("chain bound has a right edge");
                let next = bound.adjchain.expect("chain continues");
                let enextleft = self.bound(next).elast.expect("chain bound has a left edge");
                solver.add(i, i, 1.0);
                solver.add_rhs(i, 0.0);
                let sinratio = f64::from(bound.sinratio);
                if cycle {
                    solver.add(if i > 0 { i - 1 } else { np - 1 }, i, -sinratio);
                } else if i > 0 {
                    solver.add(i - 1, i, -sinratio);
                }
                let row = match cycle {
                    true => np + 2 * i,
                    false => np - 1 + 2 * i,
                };
                solver.add(row, i, weight);
                solver.add_rhs(row, weight * f64::from(self.half(eright).offset_r));
                let column = match i == np - 1 {
                    true => 0,
                    false => i + 1,
                };
                solver.add(
                    row + 1,
                    column,
                    weight * f64::from(self.bound(next).sinratio),
                );
                solver.add_rhs(row + 1, weight * f64::from(self.half(enextleft).offset_l));
            } else {
                solver.add(i - 1, i, -1.0);
            }
        }
        let solution = solver.solve();
        for (i, &v) in chain.iter().enumerate() {
            let value = solution[i];
            let bound = self.bound(v).clone();
            if cycle || i < np - 1 {
                let eright = bound.efirst.expect("chain bound has a right edge");
                self.half_mut(eright).offset_r = value as f32;
                if cycle || v != start {
                    let eleft = bound.elast.expect("chain bound has a left edge");
                    self.half_mut(eleft).offset_l = (f64::from(bound.sinratio) * value) as f32;
                }
            } else {
                let eleft = bound.elast.expect("chain bound has a left edge");
                self.half_mut(eleft).offset_l = value as f32;
            }
        }
    }

    pub(crate) fn adjust_offsets(&mut self, tagged: &[VertId]) {
        for &vert in tagged {
            let Some(bev) = self.bev_of(vert) else {
                continue;
            };
            for anchor in self.bev_ring(bev) {
                if self.bound(anchor).visited || self.bound(anchor).eon.is_none() {
                    continue;
                }
                let mut v = anchor;
                let mut chain_start = anchor;
                let mut chain_end = anchor;
                let mut cycle = false;
                let mut chain_len = 1;
                while self.bound(v).eon.is_some() && !self.bound(v).visited && !cycle {
                    self.bound_mut(v).visited = true;
                    let Some(efirst) = self.bound(v).efirst else {
                        break;
                    };
                    let Some(enext) = self.other_end(efirst) else {
                        break;
                    };
                    let Some(vnext) = self.half(enext).leftv else {
                        break;
                    };
                    self.bound_mut(v).adjchain = Some(vnext);
                    chain_end = vnext;
                    chain_len += 1;
                    if self.bound(vnext).visited {
                        if vnext != chain_start {
                            break;
                        }
                        self.adjust_cycle_or_chain(chain_start, true);
                        cycle = true;
                    }
                    v = vnext;
                }
                if !cycle {
                    self.bound_mut(v).adjchain = None;
                    v = chain_start;
                    loop {
                        self.bound_mut(v).visited = true;
                        let Some(elast) = self.bound(v).elast else {
                            break;
                        };
                        let Some(enext) = self.other_end(elast) else {
                            break;
                        };
                        let Some(vnext) = self.half(enext).rightv else {
                            break;
                        };
                        self.bound_mut(vnext).adjchain = Some(v);
                        chain_len += 1;
                        chain_start = vnext;
                        v = vnext;
                        if self.bound(v).visited || self.bound(v).eon.is_none() {
                            break;
                        }
                    }
                    if chain_len >= 3
                        && self.bound(chain_start).eon.is_none()
                        && self.bound(chain_end).eon.is_none()
                    {
                        self.adjust_cycle_or_chain(chain_start, false);
                    }
                }
            }
        }
        for &vert in tagged {
            if let Some(bev) = self.bev_of(vert) {
                self.build_boundary(bev, false);
            }
        }
    }
}
