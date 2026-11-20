use rusqlite::{Transaction, params_from_iter};

use crate::artwork::ArtworkCache;
use crate::error::Result;
use crate::model::Found;
use crate::read::rows::{self, ARTISTS};
use crate::read::{albums, tracks};
use music_core::fold;

const TRIGRAM: usize = 3;

pub(crate) fn search(
    transaction: &Transaction<'_>,
    artwork: &ArtworkCache,
    text: &str,
    limit: usize,
) -> Result<Found> {
    let folded = fold(text);
    let words: Vec<&str> = folded.split_whitespace().collect();
    if words.is_empty() {
        return Ok(Found::default());
    }
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let (long, short): (Vec<&str>, Vec<&str>) = words
        .iter()
        .partition(|word| word.chars().count() >= TRIGRAM);
    let mut values: Vec<String> = Vec::new();
    let mut conditions: Vec<String> = Vec::new();
    if !long.is_empty() {
        values.push(
            long.iter()
                .map(|word| format!("\"{}\"", word.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(" AND "),
        );
        conditions.push(format!("search MATCH ?{}", values.len()));
    }
    for word in &short {
        values.push((*word).to_owned());
        conditions.push(format!("instr(words, ?{}) > 0", values.len()));
    }
    values.push(words[0].to_owned());
    let first = values.len();
    let tracks = tracks::select(
        transaction,
        artwork,
        &format!(
            "WHERE t.id IN (SELECT rowid FROM search WHERE {})
             ORDER BY instr(t.match_title, ?{first}) = 0, t.match_title LIMIT {limit}",
            conditions.join(" AND "),
        ),
        params_from_iter(values.iter()),
    )?;

    let within = |column: &str| {
        (1..=words.len())
            .map(|at| format!("instr({column}, ?{at}) > 0"))
            .collect::<Vec<_>>()
            .join(" AND ")
    };
    let albums = albums::select(
        transaction,
        artwork,
        &format!(
            "WHERE {} ORDER BY a.sort_artist, a.sort_title LIMIT {limit}",
            within("a.folded")
        ),
        params_from_iter(words.iter()),
    )?;
    let artists = transaction
        .prepare(&format!(
            "{ARTISTS} WHERE {} ORDER BY ar.sort_name LIMIT {limit}",
            within("ar.folded")
        ))?
        .query_map(params_from_iter(words.iter()), |row| {
            rows::artist(row, artwork)
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Found {
        tracks,
        albums,
        artists,
    })
}
