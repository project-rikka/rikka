pub(crate) fn colour(items: &[Vec<u32>], particles: usize) -> Vec<u32> {
    let mut touching: Vec<Vec<u32>> = vec![Vec::new(); particles];
    for (item, ends) in items.iter().enumerate() {
        for &particle in ends {
            touching[particle as usize].push(item as u32);
        }
    }
    let neighbours: Vec<Vec<u32>> = items
        .iter()
        .enumerate()
        .map(|(item, ends)| {
            let mut met: Vec<u32> = ends
                .iter()
                .flat_map(|&particle| touching[particle as usize].iter().copied())
                .filter(|&other| other as usize != item)
                .collect();
            met.sort_unstable();
            met.dedup();
            met
        })
        .collect();
    let order = smallest_last(&neighbours);
    let mut colours = vec![u32::MAX; items.len()];
    let mut taken: Vec<bool> = Vec::new();
    for &item in order.iter().rev() {
        taken.clear();
        for &other in &neighbours[item as usize] {
            let colour = colours[other as usize];
            if colour != u32::MAX {
                let at = colour as usize;
                if taken.len() <= at {
                    taken.resize(at + 1, false);
                }
                taken[at] = true;
            }
        }
        colours[item as usize] = taken.iter().position(|used| !used).unwrap_or(taken.len()) as u32;
    }
    colours
}

fn smallest_last(neighbours: &[Vec<u32>]) -> Vec<u32> {
    let mut degree: Vec<usize> = neighbours.iter().map(Vec::len).collect();
    let most = degree.iter().copied().max().unwrap_or(0);
    let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); most + 1];
    for item in (0..neighbours.len()).rev() {
        buckets[degree[item]].push(item as u32);
    }
    let mut removed = vec![false; neighbours.len()];
    let mut order = Vec::with_capacity(neighbours.len());
    let mut lowest = 0;
    while order.len() < neighbours.len() {
        let Some(item) = take_lowest(&mut buckets, &mut lowest, &degree, &removed) else {
            break;
        };
        removed[item as usize] = true;
        order.push(item);
        for &other in &neighbours[item as usize] {
            let other = other as usize;
            if !removed[other] {
                degree[other] -= 1;
                buckets[degree[other]].push(other as u32);
                lowest = lowest.min(degree[other]);
            }
        }
    }
    order
}

fn take_lowest(
    buckets: &mut [Vec<u32>],
    lowest: &mut usize,
    degree: &[usize],
    removed: &[bool],
) -> Option<u32> {
    while *lowest < buckets.len() {
        while let Some(item) = buckets[*lowest].pop() {
            if !removed[item as usize] && degree[item as usize] == *lowest {
                return Some(item);
            }
        }
        *lowest += 1;
    }
    None
}
