use animate_motion::{Take, TakeFile};
use animate_prop::{PropFile, PropMotion};
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropRecord {
    pub motion: PropMotion,
    pub take: Take,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct PropBody {
    placement: PropFile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    take: Option<TakeFile>,
}

impl PropBody {
    pub(crate) fn from_record(record: &PropRecord) -> Self {
        Self {
            placement: PropFile::from_motion(&record.motion),
            take: (!record.take.is_still()).then(|| TakeFile::from_take(&record.take)),
        }
    }

    pub(crate) fn record(self) -> Result<PropRecord> {
        Ok(PropRecord {
            motion: self.placement.motion()?,
            take: match self.take {
                Some(file) => file.take()?,
                None => Take::default(),
            },
        })
    }
}
