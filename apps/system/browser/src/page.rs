use browser_model::LivePage;
use futures_util::{StreamExt, stream};
use rikka_web::{Event, Runtime, Session, SessionSettings};

use crate::relay::Relay;

pub(crate) type Page = browser_model::Page<Live>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Opener {
    App,
    Page,
}

#[derive(Debug)]
pub(crate) struct Live {
    pub(crate) view: Session,
    pub(crate) origin: Opener,
    pub(crate) icon: Option<String>,
    events: Relay<Event>,
}

impl Live {
    pub(crate) fn open(runtime: &Runtime, url: &str) -> rikka_web::Result<Live> {
        let session = runtime.session(SessionSettings::new().url(url))?;
        Ok(Live::adopt(session, Opener::App))
    }

    pub(crate) fn adopt(session: Session, origin: Opener) -> Live {
        Live {
            events: Relay::new(stream::iter(session.events()).flatten()),
            view: session,
            origin,
            icon: None,
        }
    }

    pub(crate) fn events(&self) -> &Relay<Event> {
        &self.events
    }
}

impl LivePage for Live {
    type View = Session;

    fn view(&self) -> &Session {
        &self.view
    }
}
