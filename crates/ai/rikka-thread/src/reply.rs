use std::sync::Arc;

use rikka_llm::{HostedPart, Trouble, Usage, Value};

use crate::call::CallResult;
use crate::replay::Replay;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Delta(String),
    Responded(Replay),
    TurnFinished(Usage),
    Compacting,
    Compacted(String),
    CompactFailed(String),
    Resumed,
    Started {
        id: Arc<str>,
        tool: Arc<str>,
        input: Arc<Value>,
    },
    Finished {
        id: Arc<str>,
        result: CallResult,
    },
    Hosted(HostedPart),
    Noticed(String),
    Interjected {
        id: u64,
        at: u64,
    },
    Done,
    Stopped,
    Failed(Trouble),
}
