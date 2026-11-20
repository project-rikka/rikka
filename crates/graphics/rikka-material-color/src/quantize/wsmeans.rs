use std::cmp::Ordering;
use std::collections::HashMap;

use crate::color::{Lab, Rgb};

const ITERATIONS: usize = 10;
const SEED: i64 = 0x42688;
const MULTIPLIER: i64 = 0x5_DEEC_E66D;
const INCREMENT: i64 = 0xB;
const STATE_MASK: i64 = (1 << 48) - 1;

struct Random(i64);

impl Random {
    const fn new(seed: i64) -> Self {
        Self((seed ^ MULTIPLIER) & STATE_MASK)
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.0 = (self.0.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT)) & STATE_MASK;
        ((self.0 as u64) >> (48 - bits)) as i32
    }

    fn below(&mut self, range: i32) -> i32 {
        if range.count_ones() == 1 {
            return (i64::from(range).wrapping_mul(i64::from(self.next(31))) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let value = bits % range;
            if (!bits).wrapping_sub(value).wrapping_add(range - 1) < 0 {
                return value;
            }
        }
    }
}

fn distance(one: Lab, two: Lab) -> f64 {
    (one.b - two.b).mul_add(
        one.b - two.b,
        (one.l - two.l).mul_add(one.l - two.l, (one.a - two.a).powi(2)),
    )
}

pub(crate) fn refine(pixels: &[Rgb], max_colors: usize, starting: &[Rgb]) -> Vec<(Rgb, u32)> {
    let mut seen: HashMap<Rgb, usize> = HashMap::new();
    let mut points: Vec<Lab> = Vec::new();
    let mut counts: Vec<u32> = Vec::new();
    for &pixel in pixels {
        match seen.get(&pixel) {
            Some(&point) => counts[point] += 1,
            None => {
                seen.insert(pixel, points.len());
                points.push(pixel.lab());
                counts.push(1);
            }
        }
    }
    let cluster_count = max_colors.min(points.len());
    if cluster_count == 0 {
        return Vec::new();
    }
    let mut clusters: Vec<Lab> = starting.iter().map(|color| color.lab()).collect();
    let missing = cluster_count.saturating_sub(clusters.len());
    if missing > 0 {
        let mut random = Random::new(SEED);
        let mut chosen: Vec<usize> = Vec::new();
        for _ in 0..missing {
            let mut index = random.below(points.len() as i32) as usize;
            while chosen.contains(&index) {
                index = random.below(points.len() as i32) as usize;
            }
            chosen.push(index);
        }
        clusters.extend(chosen.into_iter().map(|index| points[index]));
    }

    let mut assigned: Vec<usize> = (0..points.len())
        .map(|index| index % cluster_count)
        .collect();
    let mut neighbours: Vec<Vec<(f64, usize)>> = (0..cluster_count)
        .map(|_| (0..cluster_count).map(|index| (0.0, index)).collect())
        .collect();
    let mut populations = vec![0u32; cluster_count];
    for iteration in 0..ITERATIONS {
        for i in 0..cluster_count {
            for j in (i + 1)..cluster_count {
                let between = distance(clusters[i], clusters[j]);
                neighbours[j][i] = (between, i);
                neighbours[i][j] = (between, j);
            }
            neighbours[i]
                .sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap_or(Ordering::Equal));
        }
        let mut moved = 0;
        for (point, cluster) in points.iter().zip(&mut assigned) {
            let previous = *cluster;
            let previous_distance = distance(*point, clusters[previous]);
            let mut nearest = previous_distance;
            let mut better = None;
            for (candidate, center) in clusters.iter().enumerate().take(cluster_count) {
                if neighbours[previous][candidate].0 >= 4.0 * previous_distance {
                    continue;
                }
                let to_center = distance(*point, *center);
                if to_center < nearest {
                    nearest = to_center;
                    better = Some(candidate);
                }
            }
            if let Some(better) = better {
                moved += 1;
                *cluster = better;
            }
        }
        if moved == 0 && iteration > 0 {
            break;
        }
        let mut sums = vec![[0.0f64; 3]; cluster_count];
        populations.fill(0);
        for ((point, &cluster), &count) in points.iter().zip(&assigned).zip(&counts) {
            populations[cluster] += count;
            sums[cluster][0] += point.l * f64::from(count);
            sums[cluster][1] += point.a * f64::from(count);
            sums[cluster][2] += point.b * f64::from(count);
        }
        for ((cluster, sum), &population) in clusters.iter_mut().zip(&sums).zip(&populations) {
            *cluster = match population {
                0 => Lab {
                    l: 0.0,
                    a: 0.0,
                    b: 0.0,
                },
                population => Lab {
                    l: sum[0] / f64::from(population),
                    a: sum[1] / f64::from(population),
                    b: sum[2] / f64::from(population),
                },
            };
        }
    }

    let mut colors: Vec<(Rgb, u32)> = Vec::new();
    for (&cluster, &population) in clusters.iter().zip(&populations) {
        if population == 0 {
            continue;
        }
        let color = Rgb::from_lab(cluster);
        if !colors.iter().any(|&(known, _)| known == color) {
            colors.push((color, population));
        }
    }
    colors
}
