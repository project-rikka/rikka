use crate::animation::Value;
use crate::camera::{Camera, CameraField};
use crate::footage::{Mix, MixField};
use crate::graphic::{Art, ArtField};
use crate::placement::{Placement, PlacementField};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Property {
    Placement(PlacementField),
    Art(ArtField),
    Camera(CameraField),
    Mix(MixField),
    Effect { slot: usize, knob: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Field {
    Placement(PlacementField),
    Art(ArtField),
    Camera(CameraField),
    Mix(MixField),
}

impl Field {
    pub(crate) fn from_property(property: &Property) -> Option<Field> {
        match property {
            Property::Placement(field) => Some(Field::Placement(*field)),
            Property::Art(field) => Some(Field::Art(*field)),
            Property::Camera(field) => Some(Field::Camera(*field)),
            Property::Mix(field) => Some(Field::Mix(*field)),
            Property::Effect { .. } => None,
        }
    }

    pub(crate) fn is_spatial(self) -> bool {
        matches!(
            self,
            Field::Placement(PlacementField::Position) | Field::Camera(CameraField::Interest)
        )
    }

    pub(crate) fn is_pivot(self) -> bool {
        self == Field::Placement(PlacementField::Anchor)
    }

    pub(crate) fn is_audible(self) -> bool {
        matches!(self, Field::Mix(_))
    }

    pub(crate) fn is_angle(self) -> bool {
        match self {
            Field::Placement(field) => field.is_angle(),
            Field::Camera(field) => field.is_angle(),
            Field::Art(_) | Field::Mix(_) => false,
        }
    }

    pub(crate) fn clamp_value(self, value: Value) -> Value {
        match self {
            Field::Placement(field) => field.clamp_value(value),
            Field::Art(field) => field.clamp_value(value),
            Field::Camera(field) => field.clamp_value(value),
            Field::Mix(field) => field.clamp_value(value),
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Field::Placement(field) => match field {
                PlacementField::Position => "position",
                PlacementField::Depth => "depth",
                PlacementField::Anchor => "anchor",
                PlacementField::Zoom => "zoom",
                PlacementField::Aspect => "aspect",
                PlacementField::Rotation => "rotation",
                PlacementField::TiltX => "tilt_x",
                PlacementField::TiltY => "tilt_y",
                PlacementField::Opacity => "opacity",
            },
            Field::Art(field) => match field {
                ArtField::Width => "width",
                ArtField::Height => "height",
                ArtField::Corner => "corner",
                ArtField::Fill => "fill",
                ArtField::Stroke => "stroke",
                ArtField::StrokeWidth => "stroke_width",
                ArtField::End => "end",
                ArtField::Size => "size",
                ArtField::Color => "color",
                ArtField::Spacing => "spacing",
                ArtField::Reveal => "reveal",
            },
            Field::Camera(field) => match field {
                CameraField::Interest => "interest",
                CameraField::Dolly => "dolly",
                CameraField::Zoom => "zoom",
                CameraField::Roll => "roll",
                CameraField::Pan => "pan",
                CameraField::Tilt => "tilt",
                CameraField::Angle => "angle",
            },
            Field::Mix(field) => match field {
                MixField::Volume => "volume",
                MixField::Pan => "pan",
            },
        }
    }

    pub(crate) fn named(name: &str, camera: bool) -> Option<Field> {
        let fields: Vec<Field> = match camera {
            true => CameraField::ALL.map(Field::Camera).to_vec(),
            false => PlacementField::ALL
                .map(Field::Placement)
                .into_iter()
                .chain(ArtField::ALL.map(Field::Art))
                .chain(MixField::ALL.map(Field::Mix))
                .collect(),
        };
        fields.into_iter().find(|field| field.name() == name)
    }
}

pub(crate) trait Animated: Clone {
    fn animated_fields(&self) -> Vec<Field>;

    fn field_value(&self, field: Field) -> Option<Value>;

    fn with_field(&self, field: Field, value: Value) -> Self;
}

impl Animated for Placement {
    fn animated_fields(&self) -> Vec<Field> {
        PlacementField::ALL.map(Field::Placement).to_vec()
    }

    fn field_value(&self, field: Field) -> Option<Value> {
        match field {
            Field::Placement(field) => Some(Placement::value(self, field)),
            Field::Art(_) | Field::Camera(_) | Field::Mix(_) => None,
        }
    }

    fn with_field(&self, field: Field, value: Value) -> Placement {
        match field {
            Field::Placement(field) => Placement::with(*self, field, value),
            Field::Art(_) | Field::Camera(_) | Field::Mix(_) => *self,
        }
    }
}

impl Animated for Art {
    fn animated_fields(&self) -> Vec<Field> {
        Art::fields(self).into_iter().map(Field::Art).collect()
    }

    fn field_value(&self, field: Field) -> Option<Value> {
        match field {
            Field::Art(field) => Art::value(self, field),
            Field::Placement(_) | Field::Camera(_) | Field::Mix(_) => None,
        }
    }

    fn with_field(&self, field: Field, value: Value) -> Art {
        match field {
            Field::Art(field) => Art::with(self, field, value),
            Field::Placement(_) | Field::Camera(_) | Field::Mix(_) => self.clone(),
        }
    }
}

impl Animated for Camera {
    fn animated_fields(&self) -> Vec<Field> {
        CameraField::ALL.map(Field::Camera).to_vec()
    }

    fn field_value(&self, field: Field) -> Option<Value> {
        match field {
            Field::Camera(field) => Some(Camera::value(self, field)),
            Field::Placement(_) | Field::Art(_) | Field::Mix(_) => None,
        }
    }

    fn with_field(&self, field: Field, value: Value) -> Camera {
        match field {
            Field::Camera(field) => Camera::with(*self, field, value),
            Field::Placement(_) | Field::Art(_) | Field::Mix(_) => *self,
        }
    }
}

impl Animated for Mix {
    fn animated_fields(&self) -> Vec<Field> {
        MixField::ALL.map(Field::Mix).to_vec()
    }

    fn field_value(&self, field: Field) -> Option<Value> {
        match field {
            Field::Mix(field) => Some(Value::Scalar(self.value(field))),
            Field::Placement(_) | Field::Art(_) | Field::Camera(_) => None,
        }
    }

    fn with_field(&self, field: Field, value: Value) -> Mix {
        match (field, value) {
            (Field::Mix(field), Value::Scalar(value)) => self.with(field, value),
            _ => *self,
        }
    }
}
