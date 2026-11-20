mod albums;
mod artists;
mod counts;
mod genres;
mod listing;
mod rows;
mod search;
mod tracks;

pub(crate) use albums::{albums, detail as album, recently_added};
pub(crate) use artists::{artist, artists};
pub(crate) use counts::counts;
pub(crate) use genres::genres;
pub(crate) use listing::{count_under, listing, under};
pub(crate) use search::search;
pub(crate) use tracks::{
    by_artist, by_isrc, favourites, matching, most_played, of_albums, one as track, songs,
};
