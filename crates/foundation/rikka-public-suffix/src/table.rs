use std::cmp::Ordering;

use crate::rules::{ENDS, FLAGS, NAMES};

pub(crate) fn flags(candidate: &str) -> u8 {
    let mut low = 0;
    let mut high = ENDS.len();
    while low < high {
        let middle = low + (high - low) / 2;
        match compare(name(middle), candidate) {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => return FLAGS[middle],
        }
    }
    0
}

fn name(index: usize) -> &'static str {
    let start = index
        .checked_sub(1)
        .map_or(0, |previous| ENDS[previous] as usize);
    &NAMES[start..ENDS[index] as usize]
}

fn compare(name: &str, candidate: &str) -> Ordering {
    name.bytes()
        .cmp(candidate.bytes().map(|byte| byte.to_ascii_lowercase()))
}
