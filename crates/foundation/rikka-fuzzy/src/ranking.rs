use std::cmp::Reverse;

use crate::score::Score;

#[must_use]
pub fn best<T>(candidates: impl IntoIterator<Item = (Score, T)>, limit: usize) -> Vec<(Score, T)> {
    let mut ranked: Vec<(Score, usize, T)> = candidates
        .into_iter()
        .enumerate()
        .map(|(at, (score, candidate))| (score, at, candidate))
        .collect();
    let order = |a: &(Score, usize, T), b: &(Score, usize, T)| b.0.cmp(&a.0).then(a.1.cmp(&b.1));
    if ranked.len() > limit {
        ranked.select_nth_unstable_by(limit, order);
        ranked.truncate(limit);
    }
    ranked.sort_unstable_by(order);
    ranked
        .into_iter()
        .map(|(score, _, candidate)| (score, candidate))
        .collect()
}

#[must_use]
pub fn ranked<T>(candidates: impl IntoIterator<Item = (Score, T)>) -> Vec<(Score, T)> {
    let mut ranked: Vec<(Score, T)> = candidates.into_iter().collect();
    ranked.sort_by_key(|(score, _)| Reverse(*score));
    ranked
}
