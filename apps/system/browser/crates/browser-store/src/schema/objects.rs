pub(super) const OBJECTS: &[&str] = &[
    r"CREATE TABLE profiles (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        created_at INTEGER NOT NULL
    )",
    r"CREATE TABLE profile_settings (
        profile_id INTEGER PRIMARY KEY REFERENCES profiles(id) ON DELETE CASCADE,
        document TEXT NOT NULL
    )",
    r"CREATE TABLE spaces (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        icon_kind INTEGER NOT NULL,
        icon TEXT NOT NULL,
        profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
        position INTEGER NOT NULL
    )",
    r"CREATE INDEX spaces_profile ON spaces(profile_id)",
    r"CREATE TABLE favorites (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
        url TEXT NOT NULL,
        title TEXT NOT NULL,
        position INTEGER NOT NULL
    )",
    r"CREATE INDEX favorites_profile ON favorites(profile_id, position)",
    r"CREATE TABLE pinned_folders (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        space_id INTEGER NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
        parent_id INTEGER REFERENCES pinned_folders(id) ON DELETE CASCADE,
        name TEXT NOT NULL,
        expanded INTEGER NOT NULL,
        position INTEGER NOT NULL
    )",
    r"CREATE INDEX pinned_folders_space ON pinned_folders(space_id, parent_id)",
    r"CREATE INDEX pinned_folders_parent ON pinned_folders(parent_id)",
    r"CREATE TABLE pinned_tabs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        space_id INTEGER NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
        folder_id INTEGER REFERENCES pinned_folders(id) ON DELETE CASCADE,
        url TEXT NOT NULL,
        title TEXT,
        position INTEGER NOT NULL
    )",
    r"CREATE INDEX pinned_tabs_space ON pinned_tabs(space_id, folder_id)",
    r"CREATE INDEX pinned_tabs_folder ON pinned_tabs(folder_id)",
    r"CREATE TABLE session_windows (
        position INTEGER PRIMARY KEY,
        active_space INTEGER REFERENCES spaces(id) ON DELETE SET NULL,
        x INTEGER,
        y INTEGER,
        width INTEGER,
        height INTEGER,
        maximized INTEGER NOT NULL
    )",
    r"CREATE INDEX session_windows_space ON session_windows(active_space)",
    r"CREATE TABLE session_spaces (
        window_index INTEGER NOT NULL,
        space_id INTEGER NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
        position INTEGER NOT NULL,
        active_tab INTEGER,
        PRIMARY KEY (window_index, space_id)
    ) WITHOUT ROWID",
    r"CREATE INDEX session_spaces_space ON session_spaces(space_id)",
    r"CREATE TABLE session_tabs (
        id INTEGER PRIMARY KEY,
        window_index INTEGER NOT NULL,
        space_id INTEGER NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
        pinned_id INTEGER REFERENCES pinned_tabs(id) ON DELETE SET NULL,
        position INTEGER NOT NULL,
        url TEXT NOT NULL,
        title TEXT NOT NULL,
        state BLOB,
        last_active INTEGER NOT NULL,
        split_group INTEGER,
        split_order INTEGER,
        favorite_id INTEGER REFERENCES favorites(id) ON DELETE SET NULL
    )",
    r"CREATE INDEX session_tabs_space ON session_tabs(space_id)",
    r"CREATE INDEX session_tabs_pinned ON session_tabs(pinned_id)",
    r"CREATE INDEX session_tabs_favorite ON session_tabs(favorite_id)",
    r"CREATE TABLE session_splits (
        window_index INTEGER NOT NULL,
        split_group INTEGER NOT NULL,
        position INTEGER NOT NULL,
        axis INTEGER NOT NULL,
        shares BLOB NOT NULL,
        PRIMARY KEY (window_index, split_group)
    ) WITHOUT ROWID",
    r"CREATE TABLE archived_tabs (
        id INTEGER PRIMARY KEY,
        space_id INTEGER NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
        url TEXT NOT NULL,
        title TEXT NOT NULL,
        state BLOB,
        archived_at INTEGER NOT NULL
    )",
    r"CREATE INDEX archived_tabs_time ON archived_tabs(archived_at)",
    r"CREATE INDEX archived_tabs_space ON archived_tabs(space_id, archived_at)",
    r"CREATE TABLE closed_tabs (
        id INTEGER PRIMARY KEY,
        space_id INTEGER NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
        url TEXT NOT NULL,
        title TEXT NOT NULL,
        state BLOB,
        closed_at INTEGER NOT NULL
    )",
    r"CREATE INDEX closed_tabs_time ON closed_tabs(closed_at)",
    r"CREATE INDEX closed_tabs_space ON closed_tabs(space_id)",
    r"CREATE TABLE places (
        id INTEGER PRIMARY KEY,
        profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
        url TEXT NOT NULL,
        url_key TEXT NOT NULL,
        title TEXT NOT NULL DEFAULT '',
        visit_count INTEGER NOT NULL DEFAULT 0,
        typed_count INTEGER NOT NULL DEFAULT 0,
        last_visit INTEGER NOT NULL DEFAULT 0,
        frecency INTEGER NOT NULL DEFAULT 0,
        hidden_from_top INTEGER NOT NULL DEFAULT 0,
        UNIQUE (profile_id, url)
    )",
    r"CREATE INDEX places_url_key ON places(profile_id, url_key)",
    r"CREATE INDEX places_frecency ON places(profile_id, frecency)",
    r"CREATE TABLE frecency_decays (
        profile_id INTEGER PRIMARY KEY REFERENCES profiles(id) ON DELETE CASCADE,
        decayed_at INTEGER NOT NULL
    )",
    r"CREATE TABLE visits (
        id INTEGER PRIMARY KEY,
        profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
        place_id INTEGER NOT NULL REFERENCES places(id) ON DELETE CASCADE,
        visited_at INTEGER NOT NULL,
        transition INTEGER NOT NULL
    )",
    r"CREATE INDEX visits_time ON visits(profile_id, visited_at)",
    r"CREATE INDEX visits_place ON visits(place_id, visited_at)",
    r"CREATE VIRTUAL TABLE places_text USING fts5(
        url,
        title,
        content = 'places',
        content_rowid = 'id',
        tokenize = 'trigram'
    )",
    r"CREATE TRIGGER places_text_insert AFTER INSERT ON places BEGIN
        INSERT INTO places_text(rowid, url, title) VALUES (new.id, new.url, new.title);
    END",
    r"CREATE TRIGGER places_text_delete AFTER DELETE ON places BEGIN
        INSERT INTO places_text(places_text, rowid, url, title)
        VALUES ('delete', old.id, old.url, old.title);
    END",
    r"CREATE TRIGGER places_text_update AFTER UPDATE OF url, title ON places BEGIN
        INSERT INTO places_text(places_text, rowid, url, title)
        VALUES ('delete', old.id, old.url, old.title);
        INSERT INTO places_text(rowid, url, title) VALUES (new.id, new.url, new.title);
    END",
    r"CREATE TABLE favicon_pictures (
        id INTEGER PRIMARY KEY,
        hash INTEGER NOT NULL,
        width INTEGER NOT NULL,
        height INTEGER NOT NULL,
        rgba BLOB NOT NULL
    )",
    r"CREATE INDEX favicon_pictures_hash ON favicon_pictures(hash)",
    r"CREATE TABLE favicon_pages (
        url TEXT PRIMARY KEY,
        picture_id INTEGER NOT NULL REFERENCES favicon_pictures(id) ON DELETE CASCADE
    ) WITHOUT ROWID",
    r"CREATE INDEX favicon_pages_picture ON favicon_pages(picture_id)",
    r"CREATE TABLE favicon_hosts (
        host TEXT PRIMARY KEY,
        picture_id INTEGER NOT NULL REFERENCES favicon_pictures(id) ON DELETE CASCADE
    ) WITHOUT ROWID",
    r"CREATE INDEX favicon_hosts_picture ON favicon_hosts(picture_id)",
    r"CREATE TABLE site_permissions (
        profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
        origin TEXT NOT NULL,
        top_site TEXT NOT NULL DEFAULT '',
        kind INTEGER NOT NULL,
        host TEXT NOT NULL,
        decision INTEGER NOT NULL,
        changed_at INTEGER NOT NULL,
        PRIMARY KEY (profile_id, origin, top_site, kind)
    ) WITHOUT ROWID",
    r"CREATE INDEX site_permissions_host ON site_permissions(profile_id, host)",
    r"CREATE TABLE site_hosts (
        profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
        host TEXT NOT NULL,
        zoom REAL,
        https_only_exception INTEGER NOT NULL DEFAULT 0,
        changed_at INTEGER NOT NULL,
        PRIMARY KEY (profile_id, host)
    ) WITHOUT ROWID",
    r"CREATE TABLE downloads (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
        url TEXT NOT NULL,
        path BLOB NOT NULL,
        mime TEXT NOT NULL,
        total INTEGER,
        received INTEGER NOT NULL,
        state INTEGER NOT NULL,
        started_at INTEGER NOT NULL,
        finished_at INTEGER
    )",
    r"CREATE INDEX downloads_started ON downloads(profile_id, started_at)",
];
