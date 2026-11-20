use rikka_proton_api::wire_enum;
use serde::{Deserialize, Serialize};

use crate::identifier::LabelId;
use crate::system_label::SystemLabel;

wire_enum! {
    #[non_exhaustive]
    pub enum LabelType: default Label {
        Label = 1,
        ContactGroup = 2,
        Folder = 3,
        System = 4,
    }
}

impl LabelType {
    #[must_use]
    pub fn all() -> &'static [LabelType] {
        const KINDS: [LabelType; 4] = [
            LabelType::Label,
            LabelType::ContactGroup,
            LabelType::Folder,
            LabelType::System,
        ];

        &KINDS
    }

    #[must_use]
    pub fn number(self) -> i32 {
        i32::from(self)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Label {
    #[serde(rename = "ID", default)]
    id: LabelId,
    #[serde(rename = "ParentID", default, skip_serializing_if = "Option::is_none")]
    parent_id: Option<LabelId>,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Path", default, skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(rename = "Color", default)]
    color: String,
    #[serde(rename = "Type", default)]
    kind: LabelType,
    #[serde(rename = "Order", default)]
    order: i32,
    #[serde(rename = "Notify", default, with = "rikka_proton_api::flag")]
    notify: bool,
    #[serde(rename = "Display", default, with = "rikka_proton_api::flag")]
    display: bool,
    #[serde(rename = "Sticky", default, with = "rikka_proton_api::flag")]
    sticky: bool,
    #[serde(rename = "Expanded", default, with = "rikka_proton_api::flag")]
    expanded: bool,
}

impl Label {
    #[must_use]
    pub fn id(&self) -> &LabelId {
        &self.id
    }

    #[must_use]
    pub fn parent_id(&self) -> Option<&LabelId> {
        self.parent_id.as_ref()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    #[must_use]
    pub fn path_segments(&self) -> Vec<&str> {
        self.path
            .as_deref()
            .map(|path| path.split('/').collect())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn color(&self) -> &str {
        &self.color
    }

    #[must_use]
    pub fn kind(&self) -> LabelType {
        self.kind
    }

    #[must_use]
    pub fn order(&self) -> i32 {
        self.order
    }

    #[must_use]
    pub fn notifies(&self) -> bool {
        self.notify
    }

    #[must_use]
    pub fn is_displayed(&self) -> bool {
        self.display
    }

    #[must_use]
    pub fn is_sticky(&self) -> bool {
        self.sticky
    }

    #[must_use]
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    #[must_use]
    pub fn system(&self) -> Option<SystemLabel> {
        match self.kind {
            LabelType::System => self.id.as_system(),
            _ => None,
        }
    }
}
