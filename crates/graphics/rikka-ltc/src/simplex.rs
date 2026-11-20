const REFLECT: f64 = 1.0;
const EXPAND: f64 = 2.0;
const CONTRACT: f64 = 0.5;
const SHRINK: f64 = 0.5;

pub(crate) struct Search {
    pub step: f64,
    pub tolerance: f64,
    pub iterations: usize,
}

pub(crate) fn minimize<const N: usize>(
    start: [f64; N],
    search: &Search,
    mut cost: impl FnMut(&[f64; N]) -> f64,
) -> [f64; N] {
    const { assert!(N < 8) };
    let mut points = [[0.0; N]; 8];
    let mut costs = [0.0; 8];
    let count = N + 1;
    for (index, point) in points.iter_mut().take(count).enumerate() {
        *point = start;
        if index > 0 {
            point[index - 1] += search.step;
        }
    }
    for index in 0..count {
        costs[index] = cost(&points[index]);
    }
    let mut lowest = 0;
    for _ in 0..search.iterations {
        let mut order: [usize; 8] = std::array::from_fn(|index| index);
        order[..count].sort_by(|&a, &b| costs[a].total_cmp(&costs[b]));
        lowest = order[0];
        let highest = order[count - 1];
        let next = order[count - 2];
        let (best, worst) = (costs[lowest].abs(), costs[highest].abs());
        if 2.0 * (best - worst).abs() < (best + worst) * search.tolerance {
            break;
        }
        let mut centroid = [0.0; N];
        for &index in &order[..count - 1] {
            for axis in 0..N {
                centroid[axis] += points[index][axis] / N as f64;
            }
        }
        let toward = |scale: f64| -> [f64; N] {
            std::array::from_fn(|axis| {
                centroid[axis] + scale * (centroid[axis] - points[highest][axis])
            })
        };
        let reflected = toward(REFLECT);
        let reflected_cost = cost(&reflected);
        if reflected_cost < costs[next] {
            if reflected_cost < costs[lowest] {
                let expanded = toward(EXPAND);
                let expanded_cost = cost(&expanded);
                if expanded_cost < reflected_cost {
                    points[highest] = expanded;
                    costs[highest] = expanded_cost;
                    continue;
                }
            }
            points[highest] = reflected;
            costs[highest] = reflected_cost;
            continue;
        }
        let contracted = toward(-CONTRACT);
        let contracted_cost = cost(&contracted);
        if contracted_cost < costs[highest] {
            points[highest] = contracted;
            costs[highest] = contracted_cost;
            continue;
        }
        for index in 0..count {
            if index == lowest {
                continue;
            }
            for axis in 0..N {
                points[index][axis] =
                    points[lowest][axis] + SHRINK * (points[index][axis] - points[lowest][axis]);
            }
            costs[index] = cost(&points[index]);
        }
    }
    let best = (0..count)
        .min_by(|&a, &b| costs[a].total_cmp(&costs[b]))
        .unwrap_or(lowest);
    points[best]
}
