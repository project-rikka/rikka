use shell_kit::WaitingDescriptor;
pub(crate) use shell_kit::{ARRIVED, EDIT};

pub(crate) const CAPABILITY: &str = "rikka.markup";

pub(crate) const AWAITED: WaitingDescriptor = WaitingDescriptor {
    capability: CAPABILITY,
    summary: "The desktop's screenshot editor",
    subject: "the screenshot editor",
    edits: "Opens the picture in `file` for drawing on, and shows the editor",
    arrives: "A picture is waiting to be drawn on",
};
