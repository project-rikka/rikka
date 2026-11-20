use crate::wire_enum;

wire_enum! {
    #[non_exhaustive]
    pub enum EventAction: default Delete {
        Delete = 0,
        Create = 1,
        Update = 2,
        UpdateFlags = 3,
    }
}

#[macro_export]
macro_rules! change {
    ($name:ident, $id:ty, $field:ident, $payload:ty, $wire_name:literal) => {
        #[derive(Debug, Clone, ::serde::Deserialize)]
        pub struct $name {
            #[serde(rename = "ID", default)]
            id: $id,
            #[serde(rename = "Action", default)]
            action: $crate::EventAction,
            #[serde(rename = $wire_name, default)]
            $field: Option<$payload>,
        }

        impl $name {
            #[must_use]
            pub fn id(&self) -> &$id {
                &self.id
            }

            #[must_use]
            pub fn action(&self) -> $crate::EventAction {
                self.action
            }

            #[must_use]
            pub fn $field(&self) -> Option<&$payload> {
                self.$field.as_ref()
            }

            #[must_use]
            pub fn is_deleted(&self) -> bool {
                self.action == $crate::EventAction::Delete
            }
        }
    };
}
