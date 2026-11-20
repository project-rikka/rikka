use std::sync::{Arc, LazyLock};

use browser_translators::{HANDLER, PROBE_SCRIPT, WORLD};
use rikka_web::{Frames, InjectAt, MessageHandler, Session, UserScript, World};

const SCRIPT: &str = "rikka-translate";

static SOURCE: LazyLock<Arc<str>> = LazyLock::new(|| Arc::from(PROBE_SCRIPT));

pub(crate) fn wire(session: &Session) {
    session.set_user_content(
        session
            .user_content()
            .script(
                UserScript::new(SCRIPT, Arc::clone(&SOURCE))
                    .world(World::isolated(WORLD))
                    .frames(Frames::All)
                    .inject_at(InjectAt::End),
            )
            .handler(
                MessageHandler::new(HANDLER)
                    .world(World::isolated(WORLD))
                    .with_replies(),
            ),
    );
}

pub(crate) fn is_translation_message(name: &str) -> bool {
    name == HANDLER
}
