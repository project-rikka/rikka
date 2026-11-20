use rusqlite::{OptionalExtension as _, Transaction, params};

use super::Store;
use crate::error::Result;
use crate::store::journal::EntryId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DraftBlock {
    pub index: u32,
    pub hash: [u8; 32],
    pub size: u64,
    pub digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadDraft {
    pub link: String,
    pub revision: String,
    pub blocks: Vec<DraftBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftProgress {
    Created { link: String, revision: String },
    BlockUploaded(DraftBlock),
}

impl Store {
    pub fn record_draft_progress(
        &mut self,
        entry: EntryId,
        progress: &[DraftProgress],
    ) -> Result<()> {
        let transaction = self.connection.transaction()?;
        for step in progress {
            match step {
                DraftProgress::Created { link, revision } => {
                    record_draft(&transaction, entry, link, revision)?;
                }
                DraftProgress::BlockUploaded(block) => {
                    record_uploaded_block(&transaction, entry, block)?;
                }
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn draft(&self, entry: EntryId) -> Result<Option<UploadDraft>> {
        let stored: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT link, revision FROM drafts WHERE entry = ?1",
                [entry.0],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((link, revision)) = stored else {
            return Ok(None);
        };
        let mut statement = self.connection.prepare_cached(
            "SELECT block, hash, size, digest FROM draft_blocks WHERE entry = ?1 ORDER BY block",
        )?;
        let blocks = statement
            .query_map([entry.0], |row| {
                Ok(DraftBlock {
                    index: row.get(0)?,
                    hash: row.get(1)?,
                    size: super::size_from(row.get(2)?),
                    digest: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Some(UploadDraft {
            link,
            revision,
            blocks,
        }))
    }

    pub fn delete_draft(&self, entry: EntryId) -> Result<()> {
        self.connection
            .execute("DELETE FROM drafts WHERE entry = ?1", [entry.0])?;
        Ok(())
    }
}

fn record_draft(
    transaction: &Transaction<'_>,
    entry: EntryId,
    link: &str,
    revision: &str,
) -> Result<()> {
    let stored: Option<(String, String)> = transaction
        .query_row(
            "SELECT link, revision FROM drafts WHERE entry = ?1",
            [entry.0],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let is_same_draft = stored
        .as_ref()
        .is_some_and(|(stored_link, stored_revision)| {
            stored_link == link && stored_revision == revision
        });
    if !is_same_draft {
        transaction.execute("DELETE FROM drafts WHERE entry = ?1", [entry.0])?;
        transaction.execute(
            "INSERT INTO drafts (entry, link, revision) VALUES (?1, ?2, ?3)",
            params![entry.0, link, revision],
        )?;
    }
    Ok(())
}

fn record_uploaded_block(
    transaction: &Transaction<'_>,
    entry: EntryId,
    block: &DraftBlock,
) -> Result<()> {
    transaction.execute(
        "INSERT OR REPLACE INTO draft_blocks (entry, block, hash, size, digest)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            entry.0,
            block.index,
            block.hash.as_slice(),
            super::size_column(block.size),
            block.digest.as_slice(),
        ],
    )?;
    Ok(())
}
