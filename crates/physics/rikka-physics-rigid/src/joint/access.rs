use super::axes::Axes;
use super::builder::JointDef;
use super::kinds::{JointKind, JointParams};
use super::spring::Spring;
use crate::set::DISABLED_SET;
use crate::{Error, JointId, Result, RigidWorld};

mod sealed {
    use super::JointParams;

    #[derive(Debug)]
    pub struct Params<'a>(pub(crate) &'a JointParams);

    #[derive(Debug)]
    pub struct ParamsMut<'a>(pub(crate) &'a mut JointParams);

    pub trait Sealed: Sized {
        fn from_params(params: Params<'_>) -> Option<&Self>;

        fn from_params_mut(params: ParamsMut<'_>) -> Option<&mut Self>;
    }
}

pub(crate) use sealed::{Params, ParamsMut, Sealed};

pub trait JointType: Sealed {}

fn axes_of(params: &JointParams) -> Option<&Axes> {
    match params {
        JointParams::Axes(params) => Some(params.axes()),
        _ => None,
    }
}

impl JointDef {
    pub fn kind(&self) -> JointKind {
        self.params.kind()
    }

    pub fn params<T: JointType>(&self) -> Option<&T> {
        T::from_params(Params(&self.params))
    }

    pub fn axes(&self) -> Option<&Axes> {
        axes_of(&self.params)
    }

    pub fn softness(&self) -> Option<Spring> {
        self.base.softness
    }
}

impl RigidWorld {
    pub fn joint_mut<T: JointType>(&mut self, id: JointId) -> Result<&mut T> {
        let index = self.joint_index(id)?;
        let record = self.joints.get(index);
        if T::from_params(Params(&record.params)).is_none() {
            return Err(Error::JointKind(id));
        }
        if record.set != DISABLED_SET {
            let (a, b) = (record.body_a, record.body_b);
            self.wake_body(a);
            self.wake_body(b);
        }
        let record = self.joints.get_mut(index);
        record.revision += 1;
        T::from_params_mut(ParamsMut(&mut record.params)).ok_or(Error::JointKind(id))
    }

    pub fn joint<T: JointType>(&self, id: JointId) -> Result<&T> {
        let index = self.joint_index(id)?;
        T::from_params(Params(&self.joints.get(index).params)).ok_or(Error::JointKind(id))
    }

    pub fn joint_axes(&self, id: JointId) -> Result<&Axes> {
        let index = self.joint_index(id)?;
        axes_of(&self.joints.get(index).params).ok_or(Error::JointKind(id))
    }

    pub fn joint_revision(&self, id: JointId) -> Result<u64> {
        Ok(self.joints.get(self.joint_index(id)?).revision)
    }
}
