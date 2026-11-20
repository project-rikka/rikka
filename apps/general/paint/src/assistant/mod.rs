mod host;

use std::sync::Arc;

use paint_assistant::{KIT, ToolHost, functions, offering};
use rikka_app::Sender;
use rikka_app::prelude::*;
use rikka_assist::ToolRegistration;

use crate::paint::Paint;
use host::WindowToolHost;

pub(crate) fn seat(sender: Sender<Paint>, title: String) -> Option<ToolRegistration> {
    let host: Arc<dyn ToolHost> = Arc::new(WindowToolHost::new(sender));
    let seated = offering(tr!("paint.title").as_str()).and_then(|offering| {
        Ok(ToolRegistration::register(
            offering,
            title,
            functions(&host)?,
        )?)
    });
    seated
        .inspect_err(|error| tracing::warn!(%error, "cannot offer window to desktop assistant"))
        .ok()
}

pub(crate) fn open_assistant() {
    spawn_blocking(
        || rikka_assist::open_assistant(KIT.name()),
        |_: &mut Paint, summoned: rikka_assist::Result<()>| {
            if let Err(error) = summoned {
                tracing::warn!(%error, "cannot open the desktop assistant");
            }
            skip_view();
        },
    );
}
