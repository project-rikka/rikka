use animate_fluid::{EmitterId, EmitterSetting, EmitterState, Spot};
use rikka_model::glam::{Mat4, Vec3};

const PATH_POINTS: usize = 48;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub world: Vec3,
    pub frame: Mat4,
}

impl Placed {
    fn locate(spot: &Spot, locate: impl Fn(&str, &str) -> Option<Mat4>) -> Option<Self> {
        let frame = spot.anchor.frame(locate)?;
        Some(Self {
            world: frame.transform_point3(spot.offset),
            frame,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    pub figure: String,
    pub bone: String,
    pub world: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Aiming {
    pub emitter: EmitterId,
    pub view_projection: Mat4,
    pub state: EmitterState,
    pub launch: Option<Placed>,
    pub target: Option<Placed>,
    pub path: Vec<Vec3>,
    pub heading: Vec3,
    pub joints: Vec<Joint>,
}

impl Aiming {
    pub fn new(
        emitter: EmitterId,
        state: EmitterState,
        gravity: Vec3,
        view_projection: Mat4,
        locate: impl Fn(&str, &str) -> Option<Mat4>,
    ) -> Self {
        let launch = state
            .shape
            .launches()
            .then(|| Placed::locate(&state.launch, &locate))
            .flatten();
        let target = state
            .shape
            .aims()
            .then(|| Placed::locate(&state.target, &locate))
            .flatten();
        let flight = launch.and_then(|launch| {
            state.flight_between(launch.world, target.map(|target| target.world), gravity)
        });
        let falling = gravity.try_normalize().unwrap_or(Vec3::NEG_Y);
        let heading = flight
            .and_then(|flight| flight.throw.velocity.try_normalize())
            .unwrap_or(falling);
        let path = match flight {
            Some(flight) if state.shape.aims() => flight.path(gravity, PATH_POINTS),
            _ => Vec::new(),
        };
        Self {
            emitter,
            view_projection,
            state,
            launch,
            target,
            path,
            heading,
            joints: Vec::new(),
        }
    }

    pub fn placed(&self, setting: EmitterSetting) -> Option<&Placed> {
        match setting {
            EmitterSetting::Launch => self.launch.as_ref(),
            EmitterSetting::Target => self.target.as_ref(),
            _ => None,
        }
    }
}
