use futures_util::{StreamExt as _, stream};
use rikka_proton_mail::MailClient;

use crate::error::Result;
use crate::ops::read;
use crate::row::Row;
use crate::thread_cache::ThreadCache;

const TOP_ROWS: usize = 10;
const NEIGHBOURS: [isize; 3] = [1, -1, 2];
const CONCURRENT_FETCHES: usize = 2;

#[must_use]
pub fn prefetch_candidates(
    rows: &[Row],
    selected: Option<usize>,
    readings: &ThreadCache,
) -> Vec<Row> {
    let around = selected.into_iter().flat_map(|at| {
        NEIGHBOURS
            .iter()
            .filter_map(move |step| at.checked_add_signed(*step))
    });
    let mut candidates: Vec<Row> = Vec::new();

    for at in around.chain(0..TOP_ROWS) {
        let Some(row) = rows.get(at) else {
            continue;
        };
        if Some(at) == selected
            || readings.contains(&row.id)
            || candidates.iter().any(|queued| queued.id == row.id)
        {
            continue;
        }
        candidates.push(row.clone());
    }

    candidates
}

pub async fn prefetch(client: &MailClient, readings: &ThreadCache, rows: Vec<Row>) -> Result<()> {
    let mut reading = stream::iter(rows)
        .map(|row| {
            let client = client.clone();
            async move {
                let id = row.id.clone();
                readings
                    .read(&id, async move { read(&client, &row).await })
                    .await
            }
        })
        .buffered(CONCURRENT_FETCHES);

    while let Some(read) = reading.next().await {
        read?;
    }
    Ok(())
}
