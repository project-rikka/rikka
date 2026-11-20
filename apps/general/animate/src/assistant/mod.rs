mod host;

use std::sync::Arc;

use animate_assistant::{KIT, ToolHost, functions, offering};
use rikka_app::Sender;
use rikka_app::prelude::*;
use rikka_assist::ToolRegistration;

use crate::animate::Animate;
use host::WindowToolHost;

pub(crate) fn seat(sender: Sender<Animate>, title: String) -> Option<ToolRegistration> {
    let host: Arc<dyn ToolHost> = Arc::new(WindowToolHost::new(sender));
    let seat = offering(tr!("animate.title").as_str()).and_then(|offering| {
        Ok(ToolRegistration::register(
            offering,
            title,
            functions(&host)?,
        )?)
    });
    seat.inspect_err(|error| tracing::warn!(%error, "cannot offer tools to the desktop assistant"))
        .ok()
}

pub(crate) fn open_assistant() {
    spawn_blocking(
        || rikka_assist::open_assistant(KIT.name()),
        |_: &mut Animate, summoned: rikka_assist::Result<()>| {
            if let Err(error) = summoned {
                tracing::warn!(%error, "cannot open the desktop assistant");
            }
            skip_view();
        },
    );
}
