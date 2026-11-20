use std::collections::VecDeque;
use std::sync::Arc;

use browser_model::{Favicon, LivePage};
use futures_util::{StreamExt, stream};
use rikka_web::{Event, Runtime, Session, SessionSettings};

use crate::link::Opening;
use crate::relay::Relay;

pub(crate) type Page = browser_model::Page<Live>;

#[derive(Debug)]
pub(crate) struct AdoptedSession {
    pub(crate) live: Live,
    pub(crate) address: String,
}

#[derive(Debug)]
pub(crate) struct LiveIcon {
    pub(crate) url: String,
    pub(crate) favicon: Option<Favicon>,
}

#[derive(Debug)]
pub(crate) struct Live {
    pub(crate) view: Session,
    pub(crate) muted: bool,
    pub(crate) icon: Option<LiveIcon>,
    pub(crate) openings: VecDeque<(String, Opening)>,
    events: Relay<Event>,
}

pub(crate) fn settings() -> SessionSettings {
    SessionSettings::new().handle_load_requests(true)
}

impl Live {
    pub(crate) fn open(runtime: &Runtime) -> rikka_web::Result<Live> {
        Ok(Live::adopt(runtime.session(settings())?))
    }

    pub(crate) fn adopt(session: Session) -> Live {
        crate::translate::wire(&session);
        Live {
            events: Relay::new(stream::iter(session.events()).flatten()),
            view: session,
            muted: false,
            icon: None,
            openings: VecDeque::new(),
        }
    }

    pub(crate) fn events(&self) -> &Relay<Event> {
        &self.events
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Activation {
    Load(String),
    Restore { session: Arc<[u8]>, address: String },
}

impl LivePage for Live {
    type View = Session;

    fn view(&self) -> &Session {
        &self.view
    }
}
