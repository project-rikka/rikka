use std::sync::Arc;

use crate::camera::Camera;
use crate::clip::Source;
use crate::graphic::Graphic;
use crate::region::Region;

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Graphic(Graphic),
    Region(Region),
    Camera(Camera),
}

impl Item {
    pub(crate) fn source(&self) -> Source {
        match self {
            Item::Graphic(graphic) => Source::Graphic(Arc::new(graphic.clone().checked())),
            Item::Region(region) => Source::Region(Arc::new(region.checked())),
            Item::Camera(camera) => Source::Camera(Arc::new(camera.checked())),
        }
    }
}

impl From<Graphic> for Item {
    fn from(graphic: Graphic) -> Item {
        Item::Graphic(graphic)
    }
}

impl From<Region> for Item {
    fn from(region: Region) -> Item {
        Item::Region(region)
    }
}

impl From<Camera> for Item {
    fn from(camera: Camera) -> Item {
        Item::Camera(camera)
    }
}
