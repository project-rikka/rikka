use super::level::Level;

const VISITED: usize = 1 << (usize::BITS - 1);

pub fn visual_to_logical(levels: &[Level], order: &mut Vec<usize>) {
    order.clear();
    order.extend(0..levels.len());
    let numbers = || levels.iter().map(|level| level.number());
    let (Some(highest), Some(lowest_odd)) =
        (numbers().max(), numbers().filter(|n| n & 1 == 1).min())
    else {
        return;
    };
    for level in (lowest_odd..=highest).rev() {
        let mut index = 0;
        while index < order.len() {
            if levels[order[index]].number() < level {
                index += 1;
                continue;
            }
            let start = index;
            while index < order.len() && levels[order[index]].number() >= level {
                index += 1;
            }
            order[start..index].reverse();
        }
    }
}

pub fn logical_to_visual(levels: &[Level], order: &mut Vec<usize>) {
    visual_to_logical(levels, order);
    for start in 0..order.len() {
        if order[start] & VISITED != 0 {
            continue;
        }
        let mut position = start;
        let mut logical = order[start];
        while logical != start {
            let next = order[logical];
            order[logical] = position | VISITED;
            position = logical;
            logical = next;
        }
        order[start] = position | VISITED;
    }
    for position in order.iter_mut() {
        *position &= !VISITED;
    }
}
