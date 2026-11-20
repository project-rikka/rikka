mod assemble;
mod chain;
mod error;
mod fec;
mod feedback;
mod packetize;
mod quic;

pub use assemble::{Assembler, REORDER_GRACE, SHARD_SILENCE};
pub use chain::Chain;
pub use error::LinkError;
pub use fec::{Cauchy, FecError, MOST_SHARDS};
pub use feedback::{FeedbackBuilder, FrameReport, acknowledgement};
pub use packetize::{MOST_DATA_SHARDS, Packetizer, VideoFrame, parity_for};
pub use quic::{
    IDLE_TIMEOUT, KEEP_ALIVE, Pin, SERVER_NAME, ServerIdentity, Trust, client_config,
    client_endpoint, read_message, server_endpoint,
};

pub fn stamp(datagram: &mut [u8], seq: u32) {
    if let Some(field) = datagram.get_mut(1..5) {
        field.copy_from_slice(&seq.to_le_bytes());
    }
}
