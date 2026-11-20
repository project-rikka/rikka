use animate_assistant::{ToolError, ToolHost, ToolRequest, ToolResponse};
use rikka_app::Sender;
use tokio::sync::oneshot;

use crate::animate::{Animate, ServeError};

pub(crate) struct WindowToolHost {
    sender: Sender<Animate>,
}

impl WindowToolHost {
    pub(crate) fn new(sender: Sender<Animate>) -> Self {
        Self { sender }
    }
}

impl ToolHost for WindowToolHost {
    fn handle(&self, request: ToolRequest) -> ToolResponse {
        let (reply, replied) = oneshot::channel();
        let sent = self.sender.send(move |animate: &mut Animate| {
            animate.handle_tool_request(request, reply);
        });
        Box::pin(async move {
            sent.map_err(|_| ToolError::host(ServeError::WindowClosed))?;
            replied
                .await
                .unwrap_or(Err(ServeError::NoReply))
                .map_err(ToolError::host)
        })
    }
}
