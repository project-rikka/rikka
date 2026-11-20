use rikka_ui::RuntimeMessage;
use rikka_ui_host::TaskSink;
use rikka_wayland_client::ObjectId;
use shell_kit::hosting::WakingSender;

pub(super) type TaskMessage = (ObjectId, RuntimeMessage);

#[derive(Debug)]
pub(super) struct TaskSender(WakingSender<TaskMessage>);

impl TaskSender {
    pub(crate) fn new(channel: WakingSender<TaskMessage>) -> Self {
        Self(channel)
    }
}

impl TaskSink<ObjectId> for TaskSender {
    fn deliver(&self, surface: ObjectId, inbound: RuntimeMessage) -> bool {
        self.0.post((surface, inbound)).is_ok()
    }
}
