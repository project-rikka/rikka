#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Credentials {
    pub pid: i32,
    pub uid: u32,
    pub gid: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SecurityContext {
    pub sandbox_engine: Option<String>,
    pub app_id: Option<String>,
    pub instance_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
#[must_use]
pub struct ClientOptions {
    unprivileged: bool,
    xwayland: bool,
    agent: bool,
    security_context: Option<SecurityContext>,
}

impl ClientOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn unprivileged(mut self) -> Self {
        self.unprivileged = true;
        self
    }

    pub fn xwayland(mut self) -> Self {
        self.xwayland = true;
        self
    }

    pub fn agent(mut self) -> Self {
        self.agent = true;
        self
    }

    pub fn security_context(mut self, context: SecurityContext) -> Self {
        self.security_context = Some(context);
        self
    }

    pub(crate) fn privileged(&self) -> bool {
        !self.unprivileged && self.security_context.is_none()
    }

    pub(crate) fn is_xwayland(&self) -> bool {
        self.xwayland
    }

    pub(crate) fn is_agent(&self) -> bool {
        self.agent
    }

    pub(crate) fn context(&self) -> Option<&SecurityContext> {
        self.security_context.as_ref()
    }
}
