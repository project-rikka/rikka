pub(super) type Corner = (i64, i64);

pub(super) fn area(mut corners: Vec<Corner>) -> f32 {
    corners.sort_unstable();
    corners.dedup();
    if corners.len() < 3 {
        return 0.0;
    }
    let lower = chain(corners.iter().copied());
    let upper = chain(corners.iter().rev().copied());
    let ring: Vec<Corner> = lower[..lower.len() - 1]
        .iter()
        .chain(&upper[..upper.len() - 1])
        .copied()
        .collect();
    let twice: i64 = ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
        .sum();
    twice.unsigned_abs() as f32 / 2.0
}

fn chain(corners: impl Iterator<Item = Corner>) -> Vec<Corner> {
    let mut hull: Vec<Corner> = Vec::new();
    for corner in corners {
        while let [.., before, last] = hull[..] {
            if turn(before, last, corner) > 0 {
                break;
            }
            hull.pop();
        }
        hull.push(corner);
    }
    hull
}

fn turn(o: Corner, a: Corner, b: Corner) -> i64 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}
