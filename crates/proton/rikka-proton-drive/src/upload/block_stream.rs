use std::collections::HashMap;

use futures_util::future::try_join_all;
use rikka_proton_api::offload;
use rikka_proton_pgp::{Pgp, PrivateKey, PublicKey, SessionKey, Sha1};

use crate::crypto::{self, SealedBlock};
use crate::error::Result;
use crate::upload::BLOCK_SIZE;
use crate::upload::resume::UploadedBlock;
use crate::upload::source::Source;
use crate::upload::verifier::Verifier;

const SEALING_BATCH: usize = 4;

#[derive(Clone)]
pub(super) struct BlockSealer {
    pub(super) pgp: Pgp,
    pub(super) node: PublicKey,
    pub(super) content: SessionKey,
    pub(super) signer: PrivateKey,
    pub(super) verifier: Verifier,
}

pub(super) struct SealedChunk {
    pub(super) index: u32,
    pub(super) block: SealedBlock,
    pub(super) verifier_token: Vec<u8>,
    pub(super) size: u64,
    pub(super) digest: [u8; 32],
}

impl SealedChunk {
    pub(super) fn uploaded(&self) -> UploadedBlock {
        UploadedBlock::new(self.index, self.block.hash, self.size, self.digest)
    }
}

impl BlockSealer {
    fn seal(&self, index: u32, plain: &[u8], digest: [u8; 32]) -> Result<SealedChunk> {
        let block = crypto::seal_block(&self.pgp, &self.node, &self.content, &self.signer, plain)?;
        let verifier_token = self.verifier.token(&self.pgp, index, &block.data)?;

        Ok(SealedChunk {
            index,
            block,
            verifier_token,
            size: plain.len() as u64,
            digest,
        })
    }
}

pub(super) struct Batch(pub(super) Vec<SealedChunk>);

enum Slot {
    StoredAccount([u8; 32]),
    Sealing(usize),
}

pub(super) struct BlockStream {
    source: Source,
    sealer: BlockSealer,
    uploaded_blocks: HashMap<u32, UploadedBlock>,
    next_index: u32,
    manifest: Vec<u8>,
    sizes: Vec<u64>,
    digest: Sha1,
}

pub(super) struct SealedRevision {
    pub(super) manifest: Vec<u8>,
    pub(super) sizes: Vec<u64>,
    pub(super) sha1: String,
}

impl BlockStream {
    pub(super) fn new(
        source: Source,
        sealer: BlockSealer,
        uploaded_blocks: HashMap<u32, UploadedBlock>,
    ) -> BlockStream {
        BlockStream {
            source,
            sealer,
            uploaded_blocks,
            next_index: 1,
            manifest: Vec::new(),
            sizes: Vec::new(),
            digest: Sha1::new(),
        }
    }

    pub(super) async fn next_batch(&mut self) -> Result<Batch> {
        let mut slots = Vec::new();
        let mut sealing = Vec::with_capacity(SEALING_BATCH);
        while sealing.len() < SEALING_BATCH {
            let Some(chunk) = self.source.read(BLOCK_SIZE).await? else {
                break;
            };
            self.digest.update(&chunk);
            self.sizes.push(chunk.len() as u64);

            let index = self.next_index;
            self.next_index += 1;
            let uploaded_blocks = self.uploaded_blocks.get(&index).copied();
            let sealer = self.sealer.clone();
            let (digest, chunk) = offload(move || (rikka_proton_pgp::sha256(&chunk), chunk)).await;
            match uploaded_blocks {
                Some(uploaded_blocks)
                    if uploaded_blocks.digest() == digest
                        && uploaded_blocks.size() == chunk.len() as u64 =>
                {
                    slots.push(Slot::StoredAccount(uploaded_blocks.hash()));
                }
                _ => {
                    slots.push(Slot::Sealing(sealing.len()));
                    sealing.push(async move {
                        offload(move || sealer.seal(index, &chunk, digest)).await
                    });
                }
            }
        }

        let batch: Vec<SealedChunk> = try_join_all(sealing).await?;
        for slot in slots {
            match slot {
                Slot::StoredAccount(hash) => self.manifest.extend_from_slice(&hash),
                Slot::Sealing(at) => self.manifest.extend_from_slice(&batch[at].block.hash),
            }
        }
        Ok(Batch(batch))
    }

    pub(super) fn finish(self, thumbnails: &[[u8; 32]]) -> SealedRevision {
        let mut manifest: Vec<u8> = thumbnails.iter().flatten().copied().collect();
        manifest.extend_from_slice(&self.manifest);

        SealedRevision {
            manifest,
            sizes: self.sizes,
            sha1: rikka_types::hex::encode(self.digest.finalize()),
        }
    }
}
