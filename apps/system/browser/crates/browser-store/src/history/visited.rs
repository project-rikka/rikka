use rusqlite::{OptionalExtension, params};

use crate::error::Result;
use crate::ids::ProfileId;
use crate::store::Store;
use crate::time::TimeRange;
use crate::url;

impl Store {
    pub fn visited(&self, profile: ProfileId, urls: &[String]) -> Result<Vec<bool>> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT 1 FROM places WHERE profile_id = ?1 AND url = ?2 AND visit_count > 0",
            )?;
            urls.iter()
                .map(|url| {
                    let visit: Option<i64> = statement
                        .query_row(params![profile, url], |row| row.get(0))
                        .optional()?;
                    Ok(visit.is_some())
                })
                .collect()
        })
    }

    pub fn host_visited(&self, profile: ProfileId, host: &str, range: TimeRange) -> Result<bool> {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        if host.is_empty() {
            return Ok(false);
        }
        let host = match host.contains(':') && !host.starts_with('[') {
            true => format!("[{host}]"),
            false => host,
        };
        let key = url::input_key(&host);
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT p.url FROM places p \
                 WHERE p.profile_id = ?1 AND p.url_key >= ?2 AND p.url_key < ?3 \
                 AND EXISTS (SELECT 1 FROM visits v WHERE v.place_id = p.id \
                 AND v.visited_at >= ?4 AND v.visited_at < ?5)",
            )?;
            let mut rows = statement.query(params![
                profile,
                key,
                url::prefix_end(&key),
                range.from,
                range.to
            ])?;
            while let Some(row) = rows.next()? {
                let visited: String = row.get(0)?;
                if url::host(&visited).as_deref() == Some(host.as_str()) {
                    return Ok(true);
                }
            }
            Ok(false)
        })
    }
}
