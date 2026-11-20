use std::collections::BTreeMap;

use rikka_gecko_protocol::geckoview::session::{
    InitData, Module, Settings, UserAgentMode, ViewportMode,
};
use rikka_types::Size;

const MODULES: [Module; 9] = [
    Module::GeckoViewContent,
    Module::GeckoViewNavigation,
    Module::GeckoViewProcessHangMonitor,
    Module::GeckoViewProgress,
    Module::GeckoViewSettings,
    Module::GeckoViewTab,
    Module::GeckoViewContentBlocking,
    Module::GeckoViewMediaControl,
    Module::GeckoViewPrompter,
];

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSettings {
    private: bool,
    user_agent: Option<String>,
    javascript: bool,
    tracking_protection: bool,
    context: Option<String>,
    extension_popup: bool,
    handle_load_requests: bool,
    size: Option<(u32, u32)>,
    modules: BTreeMap<Module, bool>,
}

impl Default for SessionSettings {
    fn default() -> SessionSettings {
        SessionSettings {
            private: false,
            user_agent: None,
            javascript: true,
            tracking_protection: true,
            context: None,
            extension_popup: false,
            handle_load_requests: false,
            size: None,
            modules: MODULES.into_iter().map(|module| (module, true)).collect(),
        }
    }
}

impl SessionSettings {
    pub fn new() -> SessionSettings {
        SessionSettings::default()
    }

    pub fn private(mut self, private: bool) -> SessionSettings {
        self.private = private;
        self
    }

    pub fn user_agent(mut self, agent: impl Into<String>) -> SessionSettings {
        self.user_agent = Some(agent.into());
        self
    }

    pub fn javascript(mut self, enabled: bool) -> SessionSettings {
        self.javascript = enabled;
        self
    }

    pub fn tracking_protection(mut self, enabled: bool) -> SessionSettings {
        self.tracking_protection = enabled;
        self
    }

    pub fn context(mut self, context: impl Into<String>) -> SessionSettings {
        self.context = Some(context.into());
        self
    }

    pub fn extension_popup(mut self, popup: bool) -> SessionSettings {
        self.extension_popup = popup;
        self
    }

    pub fn handle_load_requests(mut self, handle: bool) -> SessionSettings {
        self.handle_load_requests = handle;
        self
    }

    pub fn module(mut self, module: Module, enabled: bool) -> SessionSettings {
        self.modules.insert(module, enabled);
        self
    }

    pub fn size(mut self, width: u32, height: u32) -> SessionSettings {
        self.size = Some((width.max(1), height.max(1)));
        self
    }

    #[must_use]
    pub fn is_private(&self) -> bool {
        self.private
    }

    #[must_use]
    pub fn context_id(&self) -> Option<&str> {
        self.context.as_deref()
    }

    pub(crate) fn handles_load_requests(&self) -> bool {
        self.handle_load_requests
    }

    pub(crate) fn viewport(&self) -> Option<Size> {
        self.size
            .map(|(width, height)| Size::new(width as f32, height as f32))
    }

    pub(crate) fn forced_private(mut self, forced: bool) -> SessionSettings {
        self.private |= forced;
        self
    }

    pub(crate) fn wire(&self) -> Settings {
        let mut settings = Settings::default();
        settings.use_tracking_protection = self.tracking_protection;
        settings.use_private_mode = self.private;
        settings.allow_javascript = self.javascript;
        settings.is_extension_popup = self.extension_popup;
        settings.user_agent_mode = UserAgentMode::Desktop;
        settings.user_agent_override = self.user_agent.clone();
        settings.viewport_mode = ViewportMode::Desktop;
        settings.set_context_id(self.context.as_deref());
        settings
    }

    pub(crate) fn init(&self) -> InitData {
        let enabled = self
            .modules
            .iter()
            .filter(|(_, enabled)| **enabled)
            .map(|(module, _)| *module);
        InitData::new(self.wire(), enabled)
    }
}
