use rusqlite::Connection;

use crate::error::Result;

pub(crate) const INDEX_VERSION: i64 = 1;

const KEPT: &str = "
CREATE TABLE IF NOT EXISTS identities (
    id INTEGER PRIMARY KEY,
    path BLOB NOT NULL,
    device INTEGER NOT NULL,
    inode INTEGER NOT NULL,
    size INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    print INTEGER NOT NULL,
    added_at INTEGER NOT NULL,
    gone_at INTEGER
);
CREATE INDEX IF NOT EXISTS identities_by_path ON identities(path);
CREATE INDEX IF NOT EXISTS identities_by_inode ON identities(device, inode);
CREATE INDEX IF NOT EXISTS identities_by_print ON identities(size, print);
CREATE TABLE IF NOT EXISTS listening (
    track INTEGER PRIMARY KEY,
    favourite_at INTEGER,
    plays INTEGER NOT NULL DEFAULT 0,
    last_played INTEGER
);
CREATE TABLE IF NOT EXISTS folders (
    path BLOB PRIMARY KEY,
    added_at INTEGER NOT NULL
);
";

const INDEX_TABLES: [&str; 13] = [
    "search",
    "track_genres",
    "track_artists",
    "album_artists",
    "track_names",
    "genres",
    "artists",
    "albums",
    "tracks",
    "failures",
    "folder_art",
    "artwork",
    "roots",
];

const INDEX: &str = "
CREATE TABLE roots (
    path BLOB PRIMARY KEY,
    online INTEGER NOT NULL,
    remote INTEGER NOT NULL,
    device INTEGER
);
CREATE TABLE tracks (
    id INTEGER PRIMARY KEY,
    root BLOB NOT NULL,
    path BLOB NOT NULL UNIQUE,
    folder BLOB NOT NULL,
    file_name TEXT NOT NULL,
    device INTEGER NOT NULL,
    inode INTEGER NOT NULL,
    size INTEGER NOT NULL,
    modified INTEGER NOT NULL,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    album TEXT,
    album_artist TEXT,
    track_number INTEGER,
    track_total INTEGER,
    disc_number INTEGER,
    disc_total INTEGER,
    disc_subtitle TEXT,
    year INTEGER,
    original_year INTEGER,
    composer TEXT,
    isrc TEXT,
    mb_recording TEXT,
    mb_track TEXT,
    mb_release TEXT,
    mb_release_group TEXT,
    label TEXT,
    catalogue TEXT,
    compilation INTEGER NOT NULL,
    sort_title TEXT,
    sort_artist TEXT,
    sort_album TEXT,
    sort_album_artist TEXT,
    track_gain REAL,
    track_peak REAL,
    album_gain REAL,
    album_peak REAL,
    codec TEXT NOT NULL,
    lossless INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    sample_rate INTEGER,
    bit_depth INTEGER,
    channels INTEGER,
    bitrate INTEGER,
    cover TEXT,
    match_title TEXT NOT NULL,
    album_id INTEGER
);
CREATE INDEX tracks_by_root ON tracks(root);
CREATE INDEX tracks_by_folder ON tracks(folder);
CREATE INDEX tracks_by_album ON tracks(album_id);
CREATE INDEX tracks_by_isrc ON tracks(isrc);
CREATE INDEX tracks_by_match ON tracks(match_title);
CREATE TABLE track_names (
    track INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE ON UPDATE CASCADE,
    role INTEGER NOT NULL,
    position INTEGER NOT NULL,
    name TEXT NOT NULL,
    PRIMARY KEY (track, role, position)
);
CREATE TABLE failures (
    path BLOB PRIMARY KEY,
    size INTEGER NOT NULL,
    modified INTEGER NOT NULL
);
CREATE TABLE folder_art (
    folder BLOB PRIMARY KEY,
    image BLOB NOT NULL,
    size INTEGER NOT NULL,
    modified INTEGER NOT NULL,
    hash TEXT NOT NULL
);
CREATE TABLE artwork (
    hash TEXT PRIMARY KEY,
    extension TEXT NOT NULL,
    width INTEGER NOT NULL
);
CREATE TABLE albums (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    year INTEGER,
    compilation INTEGER NOT NULL,
    tracks INTEGER NOT NULL,
    discs INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    codec TEXT NOT NULL,
    lossless INTEGER NOT NULL,
    bit_depth INTEGER,
    sample_rate INTEGER,
    artwork TEXT,
    added_at INTEGER NOT NULL,
    sort_title TEXT NOT NULL,
    sort_artist TEXT NOT NULL,
    folded TEXT NOT NULL
);
CREATE TABLE album_artists (
    album INTEGER NOT NULL,
    position INTEGER NOT NULL,
    artist INTEGER NOT NULL,
    PRIMARY KEY (album, position)
);
CREATE INDEX album_artists_by_artist ON album_artists(artist);
CREATE TABLE artists (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    sort_name TEXT NOT NULL,
    folded TEXT NOT NULL,
    albums INTEGER NOT NULL,
    tracks INTEGER NOT NULL,
    artwork TEXT
);
CREATE TABLE track_artists (
    track INTEGER NOT NULL,
    position INTEGER NOT NULL,
    artist INTEGER NOT NULL,
    PRIMARY KEY (track, position)
);
CREATE INDEX track_artists_by_artist ON track_artists(artist);
CREATE TABLE genres (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    folded TEXT NOT NULL
);
CREATE TABLE track_genres (
    track INTEGER NOT NULL,
    genre INTEGER NOT NULL,
    PRIMARY KEY (track, genre)
);
CREATE INDEX track_genres_by_genre ON track_genres(genre);
CREATE VIRTUAL TABLE search USING fts5(words, tokenize = 'trigram');
";

pub(crate) fn prepare(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(KEPT)?;
    let version: i64 = transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version != INDEX_VERSION {
        for table in INDEX_TABLES {
            transaction.execute_batch(&format!("DROP TABLE IF EXISTS {table};"))?;
        }
        transaction.execute_batch(INDEX)?;
        transaction.pragma_update(None, "user_version", INDEX_VERSION)?;
    }
    transaction.commit()?;
    Ok(())
}
