use shell_kit::WaitingDescriptor;
pub(crate) use shell_kit::{ARRIVED, EDIT};

pub(crate) const CAPABILITY: &str = "rikka.trim";

pub(crate) const AWAITED: WaitingDescriptor = WaitingDescriptor {
    capability: CAPABILITY,
    summary: "The desktop's recording editor",
    subject: "the recording editor",
    edits: "Opens the recording in `file` for cutting, and shows the editor",
    arrives: "A recording is waiting to be cut",
};
