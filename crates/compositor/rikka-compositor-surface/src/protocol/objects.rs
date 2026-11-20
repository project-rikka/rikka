use std::rc::Rc;

use rikka_compositor_geometry::{Logical, Region};
use slotmap::SlotMap;

use super::image_description::DescriptionObject;
use crate::color::Creator;
use crate::dmabuf::Params;
use crate::ids::{CreatorId, DescriptionId, ParamsId, RegionId, TimelineId};
use crate::syncobj::Timeline;

#[derive(Default)]
pub(crate) struct ProtocolObjects {
    pub(crate) regions: SlotMap<RegionId, Rc<Region<Logical>>>,
    pub(crate) params: SlotMap<ParamsId, Params>,
    pub(crate) timelines: SlotMap<TimelineId, Rc<Timeline>>,
    pub(crate) creators: SlotMap<CreatorId, Creator>,
    pub(crate) descriptions: SlotMap<DescriptionId, DescriptionObject>,
}
