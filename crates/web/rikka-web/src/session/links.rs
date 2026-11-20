use rikka_gecko_protocol::geckoview::navigation::OpenWhere;
use rikka_gecko_protocol::rikka::links::OpenLink as WireLink;

use crate::session::load::Load;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenLink {
    pub uri: String,
    pub referrer: Option<String>,
    pub target: OpenWhere,
}

impl OpenLink {
    pub(crate) fn from_wire(link: WireLink) -> OpenLink {
        OpenLink {
            uri: link.uri,
            referrer: link.referrer,
            target: link.target,
        }
    }

    pub fn load(&self) -> Load {
        let load = Load::uri(self.uri.clone());
        match &self.referrer {
            Some(referrer) => load.referrer(referrer.clone()),
            None => load,
        }
    }
}
