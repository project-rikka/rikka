mod frecency;
mod text;
mod visited;

use std::collections::HashSet;

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};

use crate::error::Result;
use crate::favicons;
use crate::ids::{PlaceId, ProfileId, VisitId};
use crate::records;
use crate::store::Store;
use crate::time::{TimeRange, Timestamp};
use crate::url;
use crate::writer::Coalesce;

use text::TextFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Transition {
    Link,
    Typed,
    Reload,
    Redirect,
    Form,
    BackForward,
    Generated,
}

impl Transition {
    fn code(self) -> i64 {
        match self {
            Self::Link => 0,
            Self::Typed => 1,
            Self::Reload => 2,
            Self::Redirect => 3,
            Self::Form => 4,
            Self::BackForward => 5,
            Self::Generated => 6,
        }
    }

    fn from_code(code: i64) -> Self {
        match code {
            1 => Self::Typed,
            2 => Self::Reload,
            3 => Self::Redirect,
            4 => Self::Form,
            5 => Self::BackForward,
            6 => Self::Generated,
            _ => Self::Link,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Visit {
    url: String,
    title: Option<String>,
    transition: Transition,
    at: Timestamp,
}

impl Visit {
    pub fn new(url: impl Into<String>, transition: Transition) -> Self {
        Self {
            url: url.into(),
            title: None,
            transition,
            at: Timestamp::now(),
        }
    }

    pub fn title(self, title: impl Into<String>) -> Self {
        Self {
            title: Some(title.into()),
            ..self
        }
    }

    pub fn at(self, at: Timestamp) -> Self {
        Self { at, ..self }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub id: PlaceId,
    pub url: String,
    pub title: String,
    pub visit_count: u32,
    pub typed_count: u32,
    pub last_visit: Timestamp,
    pub frecency: i64,
}

const PLACE_COLUMNS: &str =
    "p.id, p.url, p.title, p.visit_count, p.typed_count, p.last_visit, p.frecency";

fn place(row: &Row<'_>) -> rusqlite::Result<Place> {
    Ok(Place {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        visit_count: row.get(3)?,
        typed_count: row.get(4)?,
        last_visit: row.get(5)?,
        frecency: row.get(6)?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Match {
    Prefix,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub place: Place,
    pub matched: Match,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HistoryCursor {
    at: Timestamp,
    visit: VisitId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub visit: VisitId,
    pub place: PlaceId,
    pub url: String,
    pub title: String,
    pub visited_at: Timestamp,
    pub transition: Transition,
}

impl HistoryEntry {
    #[must_use]
    pub fn cursor(&self) -> HistoryCursor {
        HistoryCursor {
            at: self.visited_at,
            visit: self.visit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct HistoryQuery {
    text: String,
    range: TimeRange,
    after: Option<HistoryCursor>,
    limit: usize,
}

impl HistoryQuery {
    pub fn new(limit: usize) -> Self {
        Self {
            text: String::new(),
            range: TimeRange::all(),
            after: None,
            limit,
        }
    }

    pub fn text(self, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..self
        }
    }

    pub fn range(self, range: TimeRange) -> Self {
        Self { range, ..self }
    }

    pub fn after(self, cursor: HistoryCursor) -> Self {
        Self {
            after: Some(cursor),
            ..self
        }
    }
}

fn count(limit: usize) -> Value {
    Value::Integer(records::count(limit))
}

impl Store {
    pub fn record_visit(&self, profile: ProfileId, visit: Visit) {
        self.write_behind(None, move |connection, _| {
            record(connection, profile, &visit)
        });
    }

    pub fn set_title(&self, profile: ProfileId, url: impl Into<String>, title: impl Into<String>) {
        let (url, title) = (url.into(), title.into());
        self.write_behind(
            Some(Coalesce::Title(profile, url.clone())),
            move |connection, _| {
                connection
                    .prepare_cached(
                        "UPDATE places SET title = ?3 \
                         WHERE profile_id = ?1 AND url = ?2 AND title IS NOT ?3",
                    )?
                    .execute(params![profile, url, title])?;
                Ok(())
            },
        );
    }

    pub fn history(&self, profile: ProfileId, query: &HistoryQuery) -> Result<Vec<HistoryEntry>> {
        let mut sql = String::from(
            "SELECT v.id, v.place_id, p.url, p.title, v.visited_at, v.transition \
             FROM visits v JOIN places p ON p.id = v.place_id \
             WHERE v.profile_id = ? AND v.visited_at >= ? AND v.visited_at < ?",
        );
        let mut values = vec![
            Value::Integer(profile.get()),
            Value::Integer(query.range.from.unix_millis()),
            Value::Integer(query.range.to.unix_millis()),
        ];
        if let Some(cursor) = query.after {
            sql.push_str(" AND (v.visited_at, v.id) < (?, ?)");
            values.push(Value::Integer(cursor.at.unix_millis()));
            values.push(Value::Integer(cursor.visit.get()));
        }
        TextFilter::parse(&query.text).push_sql(&mut sql, &mut values);
        sql.push_str(" ORDER BY v.visited_at DESC, v.id DESC LIMIT ?");
        values.push(count(query.limit));
        self.read(|connection| {
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(params_from_iter(values), |row| {
                Ok(HistoryEntry {
                    visit: row.get(0)?,
                    place: row.get(1)?,
                    url: row.get(2)?,
                    title: row.get(3)?,
                    visited_at: row.get(4)?,
                    transition: Transition::from_code(row.get(5)?),
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    pub fn suggest(
        &self,
        profile: ProfileId,
        input: &str,
        limit: usize,
    ) -> Result<Vec<Suggestion>> {
        let key = url::input_key(input);
        let filter = TextFilter::parse(input);
        if limit == 0 || (key.is_empty() && filter.is_empty()) {
            return Ok(Vec::new());
        }
        self.read(|connection| {
            let mut suggestions = Vec::with_capacity(limit);
            let mut seen = HashSet::new();
            if !key.is_empty() && !key.contains(char::is_whitespace) {
                let sql = format!(
                    "SELECT {PLACE_COLUMNS} FROM places p \
                     WHERE p.profile_id = ?1 AND p.url_key >= ?2 AND p.url_key < ?3 \
                     AND p.frecency > 0 \
                     ORDER BY p.frecency DESC, p.last_visit DESC LIMIT ?4"
                );
                let mut statement = connection.prepare_cached(&sql)?;
                let rows = statement.query_map(
                    params_from_iter([
                        Value::Integer(profile.get()),
                        Value::Text(key.clone()),
                        Value::Text(url::prefix_end(&key)),
                        count(limit),
                    ]),
                    place,
                )?;
                for row in rows {
                    let place = row?;
                    seen.insert(place.id);
                    suggestions.push(Suggestion {
                        place,
                        matched: Match::Prefix,
                    });
                }
            }
            if suggestions.len() < limit && !filter.is_empty() {
                let mut sql = format!(
                    "SELECT {PLACE_COLUMNS} FROM places p WHERE p.profile_id = ? AND p.frecency > 0"
                );
                let mut values = vec![Value::Integer(profile.get())];
                filter.push_sql(&mut sql, &mut values);
                sql.push_str(" ORDER BY p.frecency DESC, p.last_visit DESC LIMIT ?");
                values.push(count(limit + seen.len()));
                let mut statement = connection.prepare(&sql)?;
                let rows = statement.query_map(params_from_iter(values), place)?;
                for row in rows {
                    let place = row?;
                    if suggestions.len() == limit {
                        break;
                    }
                    if seen.insert(place.id) {
                        suggestions.push(Suggestion {
                            place,
                            matched: Match::Text,
                        });
                    }
                }
            }
            Ok(suggestions)
        })
    }

    pub fn top_sites(&self, profile: ProfileId, limit: usize) -> Result<Vec<Place>> {
        self.read(|connection| {
            let sql = format!(
                "SELECT {PLACE_COLUMNS} FROM places p \
                 WHERE p.profile_id = ?1 AND p.frecency > 0 AND p.hidden_from_top = 0 \
                 ORDER BY p.frecency DESC, p.last_visit DESC LIMIT ?2"
            );
            let mut statement = connection.prepare_cached(&sql)?;
            let rows = statement.query_map(
                params_from_iter([Value::Integer(profile.get()), count(limit)]),
                place,
            )?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    pub fn hide_top_site(&self, profile: ProfileId, url: impl Into<String>) {
        let url = url.into();
        self.write(move |connection, _| {
            connection
                .prepare_cached(
                    "UPDATE places SET hidden_from_top = 1 WHERE profile_id = ?1 AND url = ?2",
                )?
                .execute(params![profile, url])?;
            Ok(())
        });
    }

    pub fn show_hidden_top_sites(&self, profile: ProfileId) {
        self.write(move |connection, _| {
            connection
                .prepare_cached(
                    "UPDATE places SET hidden_from_top = 0 \
                     WHERE profile_id = ?1 AND hidden_from_top = 1",
                )?
                .execute(params![profile])?;
            Ok(())
        });
    }

    pub fn delete_url(&self, profile: ProfileId, url: impl Into<String>) {
        let url = url.into();
        self.write(move |connection, _| {
            connection.execute(
                "DELETE FROM places WHERE profile_id = ?1 AND url = ?2",
                params![profile, url],
            )?;
            Ok(())
        });
    }

    pub fn delete_range(&self, profile: ProfileId, range: TimeRange) {
        self.write(move |connection, _| {
            delete_range(connection, profile, range)?;
            favicons::collect_garbage(connection)
        });
    }

    pub fn clear_history(&self, profile: ProfileId) {
        self.delete_range(profile, TimeRange::all());
    }

    pub fn prune_history(&self, profile: ProfileId, older_than: Timestamp) {
        self.delete_range(profile, TimeRange::before(older_than));
    }

    pub fn decay_frecency_if_due(&self, profile: ProfileId) {
        let now = Timestamp::now();
        self.write(move |connection, _| frecency::decay_if_due(connection, profile, now));
    }
}

fn record(connection: &Connection, profile: ProfileId, visit: &Visit) -> Result<()> {
    let existing: Option<PlaceId> = connection
        .prepare_cached("SELECT id FROM places WHERE profile_id = ?1 AND url = ?2")?
        .query_row(params![profile, visit.url], |row| row.get(0))
        .optional()?;
    let place = match existing {
        Some(place) => {
            if let Some(title) = &visit.title {
                connection
                    .prepare_cached(
                        "UPDATE places SET title = ?2 WHERE id = ?1 AND title IS NOT ?2",
                    )?
                    .execute(params![place, title])?;
            }
            place
        }
        None => connection
            .prepare_cached(
                "INSERT INTO places (profile_id, url, url_key, title) \
                 VALUES (?1, ?2, ?3, ?4) RETURNING id",
            )?
            .query_row(
                params![
                    profile,
                    visit.url,
                    url::key(&visit.url),
                    visit.title.as_deref().unwrap_or_default()
                ],
                |row| row.get(0),
            )?,
    };
    connection
        .prepare_cached(
            "INSERT INTO visits (profile_id, place_id, visited_at, transition) \
             VALUES (?1, ?2, ?3, ?4)",
        )?
        .execute(params![profile, place, visit.at, visit.transition.code()])?;
    connection
        .prepare_cached(
            "UPDATE places SET visit_count = visit_count + 1, typed_count = typed_count + ?2, \
             last_visit = max(last_visit, ?3) WHERE id = ?1",
        )?
        .execute(params![
            place,
            i64::from(visit.transition == Transition::Typed),
            visit.at
        ])?;
    frecency::recompute(connection, place, Timestamp::now())
}

pub(crate) fn delete_range(
    connection: &Connection,
    profile: ProfileId,
    range: TimeRange,
) -> Result<()> {
    if range == TimeRange::all() {
        return clear(connection, profile);
    }
    let affected: Vec<PlaceId> = {
        let mut statement = connection.prepare_cached(
            "SELECT DISTINCT place_id FROM visits \
             WHERE profile_id = ?1 AND visited_at >= ?2 AND visited_at < ?3",
        )?;
        let rows = statement.query_map(params![profile, range.from, range.to], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    connection
        .prepare_cached(
            "DELETE FROM visits WHERE profile_id = ?1 AND visited_at >= ?2 AND visited_at < ?3",
        )?
        .execute(params![profile, range.from, range.to])?;
    let now = Timestamp::now();
    let mut recount = connection.prepare_cached(
        "UPDATE places SET \
         visit_count = (SELECT count(*) FROM visits WHERE place_id = ?1), \
         typed_count = (SELECT count(*) FROM visits WHERE place_id = ?1 AND transition = ?2), \
         last_visit = coalesce((SELECT max(visited_at) FROM visits WHERE place_id = ?1), 0) \
         WHERE id = ?1 RETURNING visit_count",
    )?;
    let mut remove = connection.prepare_cached("DELETE FROM places WHERE id = ?1")?;
    for place in affected {
        let visits: i64 =
            recount.query_row(params![place, Transition::Typed.code()], |row| row.get(0))?;
        if visits == 0 {
            remove.execute([place])?;
        } else {
            frecency::recompute(connection, place, now)?;
        }
    }
    Ok(())
}

fn clear(connection: &Connection, profile: ProfileId) -> Result<()> {
    connection.execute("DELETE FROM visits WHERE profile_id = ?1", [profile])?;
    connection.execute("DELETE FROM places WHERE profile_id = ?1", [profile])?;
    Ok(())
}
