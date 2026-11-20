use std::time::Duration;

use music_core::{Catalog, Result, Track, fold};

const SAME_TITLE: f32 = 0.75;
const SAME_ARTIST: f32 = 0.5;
const CLOSE: Duration = Duration::from_secs(3);
const NEAR: Duration = Duration::from_secs(10);
const SAME_CODE: f32 = 4.0;
const NOISE: [&str; 14] = [
    "feat",
    "ft",
    "featuring",
    "remaster",
    "remastered",
    "explicit",
    "clean",
    "album version",
    "single version",
    "official audio",
    "official video",
    "official music video",
    "lyric video",
    "visualizer",
];
const TOPIC: &str = " - topic";

pub async fn find(catalog: &dyn Catalog, target: &Track) -> Result<Option<Track>> {
    let candidates = catalog.lookup(target).await?;
    Ok(best(target, &candidates).cloned())
}

#[must_use]
pub(crate) fn best<'a>(target: &Track, candidates: &'a [Track]) -> Option<&'a Track> {
    candidates
        .iter()
        .filter_map(|candidate| score(target, candidate).map(|score| (score, candidate)))
        .max_by(|(left, _), (right, _)| left.total_cmp(right))
        .map(|(_, candidate)| candidate)
}

#[must_use]
pub(crate) fn score(target: &Track, candidate: &Track) -> Option<f32> {
    if !candidate.playable {
        return None;
    }
    if let (Some(target_isrc), Some(candidate_isrc)) = (&target.isrc, &candidate.isrc)
        && target_isrc == candidate_isrc
    {
        return Some(SAME_CODE);
    }
    let timing = timing(target.duration, candidate.duration)?;
    let title = dice(
        &words(&title_of(&target.name)),
        &words(&title_of(&candidate.name)),
    );
    if title < SAME_TITLE {
        return None;
    }
    let artist = artist_likeness(target, candidate);
    if artist < SAME_ARTIST {
        return None;
    }
    Some(title + artist / 2.0 + timing / 4.0)
}

fn timing(target: Duration, candidate: Duration) -> Option<f32> {
    if target.is_zero() || candidate.is_zero() {
        return Some(0.0);
    }
    match target.abs_diff(candidate) {
        apart if apart <= CLOSE => Some(1.0),
        apart if apart <= NEAR => Some(0.5),
        _ => None,
    }
}

fn artist_likeness(target: &Track, candidate: &Track) -> f32 {
    let named = |track: &Track| -> Vec<Vec<String>> {
        track
            .artists
            .iter()
            .map(|artist| words(artist_of(&artist.name)))
            .filter(|words| !words.is_empty())
            .collect()
    };
    let (target_names, candidate_names) = (named(target), named(candidate));
    let paired = target_names
        .iter()
        .flat_map(|left| candidate_names.iter().map(move |right| dice(left, right)))
        .fold(0.0, f32::max);
    let bylines = dice(&words(&target.byline()), &words(&candidate.byline()));
    paired.max(bylines)
}

fn artist_of(name: &str) -> &str {
    let lowered = name.len().checked_sub(TOPIC.len()).and_then(|cut| {
        name.get(cut..)
            .filter(|tail| tail.eq_ignore_ascii_case(TOPIC))
            .map(|_| &name[..cut])
    });
    lowered.unwrap_or(name)
}

fn title_of(name: &str) -> String {
    let folded = fold(name);
    let mut cleaned = String::with_capacity(folded.len());
    let mut depth = 0usize;
    let mut aside = String::new();
    for letter in folded.chars() {
        match letter {
            '(' | '[' | '{' => {
                if depth == 0 {
                    aside.clear();
                } else {
                    aside.push(letter);
                }
                depth += 1;
            }
            ')' | ']' | '}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    if !noisy(&aside) {
                        cleaned.push(' ');
                        cleaned.push_str(&aside);
                    }
                    cleaned.push(' ');
                } else {
                    aside.push(letter);
                }
            }
            _ if depth > 0 => aside.push(letter),
            _ => cleaned.push(letter),
        }
    }
    cleaned
        .split(" - ")
        .enumerate()
        .filter(|(at, part)| *at == 0 || !noisy(part))
        .map(|(_, part)| part)
        .collect::<Vec<_>>()
        .join(" ")
}

fn noisy(part: &str) -> bool {
    let message = words(part).join(" ");
    NOISE.iter().any(|noise| {
        message == *noise
            || message.starts_with(&format!("{noise} "))
            || message.ends_with(&format!(" {noise}"))
            || message.contains(&format!(" {noise} "))
    })
}

fn words(text: &str) -> Vec<String> {
    fold(text)
        .split(|letter: char| !letter.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

fn dice(left: &[String], right: &[String]) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let mut unused: Vec<&String> = right.iter().collect();
    let shared = left
        .iter()
        .filter(|word| {
            unused
                .iter()
                .position(|known| known == word)
                .map(|at| unused.swap_remove(at))
                .is_some()
        })
        .count();
    (2 * shared) as f32 / (left.len() + right.len()) as f32
}
