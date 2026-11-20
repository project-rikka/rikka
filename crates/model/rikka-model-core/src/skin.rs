use glam::Mat4;

use crate::error::{Error, Result};
use crate::id::model_id;
use crate::node::NodeId;

model_id!(SkinId);

#[derive(Clone, Debug, Default)]
pub struct Skin {
    pub name: String,
    pub joints: Vec<NodeId>,
    pub inverse_bind: Vec<Mat4>,
    pub skeleton: Option<NodeId>,
}

impl Skin {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn joint_count(&self) -> usize {
        self.joints.len()
    }

    pub(crate) fn validate(&self, nodes: usize) -> Result<()> {
        if self.inverse_bind.len() != self.joints.len() {
            return Err(Error::Invalid(format!(
                "skin \"{}\" binds {} joints with {} matrices",
                self.name,
                self.joints.len(),
                self.inverse_bind.len()
            )));
        }
        for joint in self.joints.iter().chain(self.skeleton.iter()) {
            if joint.index() >= nodes {
                return Err(Error::Invalid(format!(
                    "skin \"{}\" references missing joint {joint}",
                    self.name
                )));
            }
        }
        Ok(())
    }
}
