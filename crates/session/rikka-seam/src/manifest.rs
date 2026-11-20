use crate::capability::{CapabilityId, Version, is_valid_member};
use crate::error::SeamError;
use crate::event::DeliveryMode;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    capability: CapabilityId,
    version: Version,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    summary: Option<Box<str>>,
    #[serde(default)]
    methods: Vec<MethodInfo>,
    #[serde(default)]
    events: Vec<EventInfo>,
}

impl Manifest {
    pub fn new(capability: impl Into<CapabilityId>, version: Version) -> Self {
        Self {
            capability: capability.into(),
            version,
            summary: None,
            methods: Vec::new(),
            events: Vec::new(),
        }
    }

    #[must_use]
    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into().into_boxed_str());
        self
    }

    #[must_use]
    pub fn method(mut self, method: MethodInfo) -> Self {
        self.methods.push(method);
        self
    }

    #[must_use]
    pub fn event(mut self, event: EventInfo) -> Self {
        self.events.push(event);
        self
    }

    pub fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    pub fn version(&self) -> Version {
        self.version
    }

    pub fn summary_text(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    pub fn methods(&self) -> &[MethodInfo] {
        &self.methods
    }

    pub fn events(&self) -> &[EventInfo] {
        &self.events
    }

    pub fn declared_event(&self, name: &str) -> Option<&EventInfo> {
        self.events.iter().find(|event| event.name() == name)
    }

    pub fn declares_method(&self, name: &str) -> bool {
        self.methods.iter().any(|method| method.name() == name)
    }

    pub fn declares_event(&self, name: &str) -> bool {
        self.declared_event(name).is_some()
    }

    pub(crate) fn validate(&self) -> Result<(), SeamError> {
        if !self.capability.is_valid() {
            return Err(SeamError::InvalidName {
                name: self.capability.as_str().to_owned(),
            });
        }
        check(self.methods.iter().map(MemberInfo::name))?;
        check(self.events.iter().map(EventInfo::name))
    }
}

fn check<'a>(names: impl Iterator<Item = &'a str>) -> Result<(), SeamError> {
    let mut seen: Vec<&str> = Vec::new();
    for name in names {
        if !is_valid_member(name) || seen.contains(&name) {
            return Err(SeamError::InvalidName {
                name: name.to_owned(),
            });
        }
        seen.push(name);
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemberInfo {
    name: Box<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    summary: Option<Box<str>>,
}

pub type MethodInfo = MemberInfo;

impl MemberInfo {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into().into_boxed_str(),
            summary: None,
        }
    }

    #[must_use]
    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into().into_boxed_str());
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn summary_text(&self) -> Option<&str> {
        self.summary.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EventInfo {
    #[serde(flatten)]
    member: MemberInfo,
    #[serde(default, skip_serializing_if = "is_every")]
    delivery: DeliveryMode,
}

fn is_every(delivery: &DeliveryMode) -> bool {
    *delivery == DeliveryMode::Every
}

impl EventInfo {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            member: MemberInfo::new(name),
            delivery: DeliveryMode::Every,
        }
    }

    #[must_use]
    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.member = self.member.summary(summary);
        self
    }

    #[must_use]
    pub fn latest(mut self) -> Self {
        self.delivery = DeliveryMode::Latest;
        self
    }

    pub fn name(&self) -> &str {
        self.member.name()
    }

    pub fn summary_text(&self) -> Option<&str> {
        self.member.summary_text()
    }

    pub fn delivery_mode(&self) -> DeliveryMode {
        self.delivery
    }
}
