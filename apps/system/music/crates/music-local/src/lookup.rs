use std::time::Duration;

use music_core::{Result, Track};
use music_library::Library;

use crate::off_thread::failed;

const WINDOW: Duration = Duration::from_secs(10);
const SEARCHED: usize = 20;
const ASIDES: [&str; 4] = [" (", " [", " - ", " feat"];

pub(crate) fn candidates(library: &Library, target: &Track) -> Result<Vec<music_library::Track>> {
    if let Some(isrc) = &target.isrc {
        let same = library.by_isrc(isrc).map_err(failed)?;
        if !same.is_empty() {
            return Ok(same);
        }
    }
    let artist = target
        .artists
        .first()
        .map_or("", |artist| artist.name.as_str());
    let matched = library
        .matching(&target.name, artist, target.duration, WINDOW)
        .map_err(failed)?;
    if !matched.is_empty() {
        return Ok(matched);
    }
    let title = bare_title(&target.name);
    let named = library
        .search(&format!("{title} {artist}"), SEARCHED)
        .map_err(failed)?;
    if !named.tracks.is_empty() {
        return Ok(named.tracks);
    }
    Ok(library.search(title, SEARCHED).map_err(failed)?.tracks)
}

fn bare_title(name: &str) -> &str {
    let lowered = name.to_ascii_lowercase();
    let cut = ASIDES
        .iter()
        .filter_map(|aside| lowered.find(aside))
        .filter(|at| *at > 0)
        .min()
        .unwrap_or(name.len());
    name[..cut].trim()
}
