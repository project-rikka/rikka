use rikka_settings::session::displays;
use rikka_types::Transform;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rotation {
    #[default]
    None,
    Quarter,
    Half,
    ThreeQuarters,
}

impl Rotation {
    pub const ALL: [Self; 4] = [Self::None, Self::Quarter, Self::Half, Self::ThreeQuarters];

    pub fn is_upright(self) -> bool {
        matches!(self, Self::Quarter | Self::ThreeQuarters)
    }

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|rotation| *rotation == self)
            .expect("every rotation is in ALL")
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Orientation {
    pub rotation: Rotation,
    pub flipped: bool,
}

impl Orientation {
    pub fn parse(text: &str) -> Option<Self> {
        displays::transform_named(text).map(Self::from)
    }

    pub fn name(self) -> &'static str {
        displays::transform_name(self.into())
    }

    pub fn is_upright(self) -> bool {
        self.rotation.is_upright()
    }
}

impl From<Transform> for Orientation {
    fn from(transform: Transform) -> Self {
        Self {
            rotation: Rotation::ALL[transform.quarter_turns() as usize],
            flipped: transform.is_flipped(),
        }
    }
}

impl From<Orientation> for Transform {
    fn from(orientation: Orientation) -> Self {
        let turned = Transform::ALL[orientation.rotation.index()];
        match orientation.flipped {
            true => Transform::Flipped.then(turned),
            false => turned,
        }
    }
}
