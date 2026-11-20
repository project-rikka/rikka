use crate::device::DeviceId;
use crate::prompt::Prompt;
use crate::request::Action;
use crate::snapshot::Snapshot;

#[derive(Clone, Debug)]
pub enum Event {
    Changed(Box<Snapshot>),
    Answered {
        action: Action,
        device: Option<DeviceId>,
        error: Option<String>,
    },
    Asked(Prompt),
    Withdrew(u64),
}
