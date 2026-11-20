use rusqlite::{Connection, OptionalExtension, params};

use crate::error::Result;
use crate::history::Transition;
use crate::ids::{PlaceId, ProfileId};
use crate::time::Timestamp;

const SAMPLED_VISITS: i64 = 10;
const DAY_MILLIS: i64 = 24 * 60 * 60 * 1000;
const BUCKETS: [(i64, f64); 4] = [(4, 100.0), (14, 70.0), (31, 50.0), (90, 30.0)];
const OLDEST_WEIGHT: f64 = 10.0;
const DAILY_DECAY: f64 = 0.975;

fn weight(age_millis: i64) -> f64 {
    let days = age_millis.max(0) / DAY_MILLIS;
    BUCKETS
        .iter()
        .find(|(limit, _)| days < *limit)
        .map_or(OLDEST_WEIGHT, |(_, weight)| *weight)
}

fn bonus(transition: Transition) -> f64 {
    match transition {
        Transition::Typed => 2000.0,
        Transition::Link | Transition::Form | Transition::Generated => 100.0,
        Transition::BackForward => 50.0,
        Transition::Reload | Transition::Redirect => 0.0,
    }
}

pub(crate) fn recompute(connection: &Connection, place: PlaceId, now: Timestamp) -> Result<()> {
    let visit_count: i64 = connection
        .prepare_cached("SELECT visit_count FROM places WHERE id = ?1")?
        .query_row([place], |row| row.get(0))?;
    let mut statement = connection.prepare_cached(
        "SELECT visited_at, transition FROM visits WHERE place_id = ?1 \
         ORDER BY visited_at DESC LIMIT ?2",
    )?;
    let mut samples = 0_u32;
    let mut points = 0.0;
    let rows = statement.query_map(params![place, SAMPLED_VISITS], |row| {
        Ok((row.get::<_, Timestamp>(0)?, row.get::<_, i64>(1)?))
    })?;
    for row in rows {
        let (visited_at, transition) = row?;
        samples += 1;
        let age = now.unix_millis().saturating_sub(visited_at.unix_millis());
        points += bonus(Transition::from_code(transition)) / 100.0 * weight(age);
    }
    let frecency = if samples == 0 {
        0
    } else {
        (visit_count as f64 * points / f64::from(samples)).ceil() as i64
    };
    connection
        .prepare_cached("UPDATE places SET frecency = ?2 WHERE id = ?1")?
        .execute(params![place, frecency])?;
    Ok(())
}

pub(crate) fn decay_if_due(
    connection: &Connection,
    profile: ProfileId,
    now: Timestamp,
) -> Result<()> {
    let decayed_at: Option<Timestamp> = connection
        .prepare_cached("SELECT decayed_at FROM frecency_decays WHERE profile_id = ?1")?
        .query_row([profile], |row| row.get(0))
        .optional()?;
    let Some(decayed_at) = decayed_at else {
        connection
            .prepare_cached("INSERT INTO frecency_decays (profile_id, decayed_at) VALUES (?1, ?2)")?
            .execute(params![profile, now])?;
        return Ok(());
    };
    let days = now.unix_millis().saturating_sub(decayed_at.unix_millis()) / DAY_MILLIS;
    if days < 1 {
        return Ok(());
    }
    let exponent = i32::try_from(days).unwrap_or(i32::MAX);
    scale(connection, profile, DAILY_DECAY.powi(exponent))?;
    connection
        .prepare_cached("UPDATE frecency_decays SET decayed_at = ?2 WHERE profile_id = ?1")?
        .execute(params![
            profile,
            Timestamp::from_unix_millis(
                decayed_at
                    .unix_millis()
                    .saturating_add(days.saturating_mul(DAY_MILLIS))
            )
        ])?;
    Ok(())
}

fn scale(connection: &Connection, profile: ProfileId, factor: f64) -> Result<()> {
    connection
        .prepare_cached(
            "UPDATE places SET frecency = CAST(round(frecency * ?1) AS INTEGER) \
             WHERE profile_id = ?2 AND frecency > 0",
        )?
        .execute(params![factor, profile])?;
    Ok(())
}
