use rikka_figure::{Affine, Frame, Grips};
use rikka_types::{Point, Rect, Size};

use crate::animation_presets::PresetTransform;
use crate::camera::Camera;
use crate::footage::Treatment;
use crate::graphic::{Art, Graphic};
use crate::placement::Placement;
use crate::project::Format;
use crate::region::Region;

#[derive(Clone, Debug, PartialEq)]
pub enum Appearance {
    Graphic(Graphic),
    Footage(Treatment),
    Region(Region),
    Camera(Camera),
}

impl Appearance {
    #[must_use]
    pub fn placement(&self) -> Option<Placement> {
        match self {
            Appearance::Graphic(graphic) => Some(graphic.placement),
            Appearance::Footage(treatment) => Some(treatment.placement),
            Appearance::Region(region) => Some(region.placement),
            Appearance::Camera(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Visual {
    Graphic(Graphic),
    Footage { treatment: Treatment, picture: Size },
    Region(Region),
}

impl Visual {
    #[must_use]
    pub fn with_appearance(self, appearance: Option<&Appearance>) -> Visual {
        match (appearance, self) {
            (Some(Appearance::Graphic(graphic)), Visual::Graphic(_)) => {
                Visual::Graphic(graphic.clone())
            }
            (Some(Appearance::Footage(treatment)), Visual::Footage { picture, .. }) => {
                Visual::Footage {
                    treatment: *treatment,
                    picture,
                }
            }
            (Some(Appearance::Region(region)), Visual::Region(_)) => Visual::Region(*region),
            (_, visual) => visual,
        }
    }

    #[must_use]
    pub fn placement(&self) -> Placement {
        match self {
            Visual::Graphic(graphic) => graphic.placement,
            Visual::Footage { treatment, .. } => treatment.placement,
            Visual::Region(region) => region.placement,
        }
    }

    #[must_use]
    pub fn placed(&self, placement: Placement) -> Visual {
        match self {
            Visual::Graphic(graphic) => Visual::Graphic(Graphic {
                placement,
                art: graphic.art.clone(),
            }),
            Visual::Footage { treatment, picture } => Visual::Footage {
                treatment: Treatment {
                    placement,
                    ..*treatment
                },
                picture: *picture,
            },
            Visual::Region(region) => Visual::Region(region.with_placement(placement)),
        }
    }

    #[must_use]
    pub fn with_preset_transform(&self, transform: &PresetTransform) -> Visual {
        let placed = self.placed(transform.placed(self.placement()).checked());
        match placed {
            Visual::Graphic(Graphic {
                placement,
                art: Art::Text { lettering, reveal },
            }) => Visual::Graphic(Graphic {
                placement,
                art: Art::Text {
                    lettering,
                    reveal: reveal * transform.reveal,
                },
            }),
            other => other,
        }
    }

    #[must_use]
    pub fn graphic(&self) -> Option<&Graphic> {
        match self {
            Visual::Graphic(graphic) => Some(graphic),
            Visual::Footage { .. } | Visual::Region(_) => None,
        }
    }

    #[must_use]
    pub fn region(&self) -> Option<&Region> {
        match self {
            Visual::Region(region) => Some(region),
            Visual::Footage { .. } | Visual::Graphic(_) => None,
        }
    }

    #[must_use]
    pub fn is_text(&self) -> bool {
        self.graphic().is_some_and(Graphic::is_text)
    }

    #[must_use]
    pub fn appearance(&self) -> Appearance {
        match self {
            Visual::Graphic(graphic) => Appearance::Graphic(graphic.clone()),
            Visual::Footage { treatment, .. } => Appearance::Footage(*treatment),
            Visual::Region(region) => Appearance::Region(*region),
        }
    }

    #[must_use]
    pub fn frame(&self, format: Format) -> Frame {
        match self {
            Visual::Graphic(graphic) => graphic.frame(format),
            Visual::Footage { treatment, picture } => treatment.placement.frame(*picture, format),
            Visual::Region(region) => region.frame(format),
        }
    }

    #[must_use]
    pub fn grips(&self, format: Format) -> Option<Grips> {
        match self {
            Visual::Graphic(graphic) => Some(graphic.grips(format)),
            Visual::Footage { .. } => Some(Grips::Frame {
                frame: self.frame(format),
                sides: false,
                turns: true,
            }),
            Visual::Region(region) => region.grips(format),
        }
    }

    #[must_use]
    pub fn bounds(&self, format: Format) -> Option<Rect> {
        match self {
            Visual::Graphic(graphic) => Some(graphic.bounds(format)),
            Visual::Footage { .. } => Some(self.frame(format).bounds()),
            Visual::Region(region) => region.bounds(format),
        }
    }

    #[must_use]
    pub fn hits(&self, format: Format, at: Point, reach: f32) -> bool {
        match self {
            Visual::Graphic(graphic) => graphic.hits(format, at, reach),
            Visual::Footage { .. } => self.frame(format).contains(at, reach),
            Visual::Region(region) => region.hits(format, at, reach),
        }
    }

    #[must_use]
    pub fn mapped(&self, map: Affine, format: Format) -> Visual {
        match self {
            Visual::Graphic(graphic) => Visual::Graphic(graphic.mapped(map, format)),
            Visual::Footage { treatment, picture } => {
                let before = self.frame(format);
                let after = before.mapped(map);
                Visual::Footage {
                    treatment: Treatment {
                        placement: treatment.placement.grown(&before, &after, format),
                        ..*treatment
                    },
                    picture: *picture,
                }
            }
            Visual::Region(region) => Visual::Region(region.mapped(map, format)),
        }
    }

    #[must_use]
    pub fn with_endpoints(&self, from: Point, to: Point, format: Format) -> Visual {
        match self {
            Visual::Graphic(graphic) => Visual::Graphic(graphic.with_endpoints(from, to, format)),
            Visual::Footage { .. } | Visual::Region(_) => self.clone(),
        }
    }

    #[must_use]
    pub fn moved(&self, by: Point) -> Visual {
        match self {
            Visual::Graphic(graphic) => Visual::Graphic(graphic.moved(by)),
            Visual::Footage { treatment, picture } => Visual::Footage {
                treatment: Treatment {
                    placement: treatment.placement.moved(by),
                    ..*treatment
                },
                picture: *picture,
            },
            Visual::Region(region) => Visual::Region(region.moved(by)),
        }
    }
}
