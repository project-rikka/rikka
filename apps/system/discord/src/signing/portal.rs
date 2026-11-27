use discord_client::WebSignIn;
use rikka_web::{
    InjectAt, NewSessionRequest, Script, ScriptMessage, Session, SignInPortal, UserScript, World,
};

const HOOK: &str = "rikka-discord-token";

const DISCORD: &str = "https://discord.com/*";

#[derive(Debug)]
pub(crate) struct Portal {
    request: WebSignIn,
    portal: SignInPortal,
}

impl Portal {
    pub(crate) fn open(request: WebSignIn) -> Result<Portal, String> {
        let hook = UserScript::new(HOOK, Script::bundled(request.script()))
            .inject_at(InjectAt::Start)
            .world(World::Page)
            .matches([DISCORD]);
        let portal =
            SignInPortal::open(request.url(), &[hook]).map_err(|error| error.to_string())?;
        Ok(Portal { request, portal })
    }

    pub(crate) fn session(&self) -> &Session {
        self.portal.session()
    }

    pub(crate) fn follow(&self, opening: NewSessionRequest) {
        self.portal.follow(opening);
    }

    pub(crate) fn posted(&self, message: &ScriptMessage) -> Option<String> {
        if message.name != self.request.message() || message.world != World::Page {
            return None;
        }
        message
            .body
            .as_str()
            .map(str::trim)
            .filter(|posted| !posted.is_empty())
            .map(str::to_owned)
    }

    pub(crate) fn finish(self, posted: String) {
        self.request.answer(posted);
    }
}
