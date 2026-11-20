use bytemuck::{Pod, Zeroable};

pub const MAX_SUBSTANCES: usize = 32;

const WATER_VISCOSITY: f32 = 1.0e-6;
const VISCOSITY_DECADES: f32 = 6.0;
const SMOOTHING: f32 = 0.15;
const SMOOTHING_THICK: f32 = 0.2;
const VORTICITY: f32 = 0.06;
const COHESION: f32 = 30.0;
const BONDED_COHESION: f32 = 0.9;
const COHESIVE_NEIGHBOURS: f32 = 6.0;
const COHESIVE_SLOPE: f32 = 3.0;
const ADHESION: f32 = 300.0;
const STICK_RATE: f32 = 200.0;
const FRICTION: f32 = 1.5;
const CONTACT_DRAG: f32 = 200.0;
const ANCHORED_FROM: f32 = 0.3;
const ANCHOR_BREAK: f32 = 2.5;
const STIFFEST_BOND: f32 = 282.0;
const BOND_CURVE: f32 = 1.25;
const REFERENCE_SPACING: f32 = 0.004;
const SLOWEST_FLOW: f32 = 0.02;
const FLOW_RANGE: f32 = 400.0;
const DASHPOT: f32 = 0.005;
const MIN_YIELD: f32 = 0.15;
const YIELD_RANGE: f32 = 0.2;
const WETTING: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Substance {
    pub viscosity: f32,
    pub elasticity: f32,
    pub relaxation: f32,
    pub cohesion: f32,
    pub adhesion: f32,
    pub friction: f32,
    pub spray: f32,
    pub wetting: f32,
    pub tint: [f32; 3],
}

impl Default for Substance {
    fn default() -> Self {
        Self {
            viscosity: 0.0,
            elasticity: 0.0,
            relaxation: 1.0,
            cohesion: 0.4,
            adhesion: 0.05,
            friction: 0.5,
            spray: 0.5,
            wetting: 0.5,
            tint: [0.8, 0.9, 1.0],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct SubstanceBlock {
    flow: [f32; 4],
    stick: [f32; 4],
    bond: [f32; 4],
    tint: [f32; 4],
    contact: [f32; 4],
}

impl Substance {
    pub(crate) fn kinematic_viscosity(&self) -> f32 {
        if self.viscosity > 0.0 {
            WATER_VISCOSITY * 10f32.powf(VISCOSITY_DECADES * unit(self.viscosity))
        } else {
            0.0
        }
    }

    pub(crate) fn bonds(&self) -> bool {
        unit(self.elasticity) > 0.0
    }

    pub(crate) fn cohesive(&self) -> bool {
        unit(self.cohesion) > 0.0
    }

    pub(crate) fn sprays(&self) -> bool {
        unit(self.spray) > 0.0
    }

    fn vorticity(&self) -> f32 {
        VORTICITY * (1.0 - unit(self.viscosity)).powi(4)
    }

    fn compliance(&self, spacing: f32) -> f32 {
        let frequency =
            STIFFEST_BOND * unit(self.elasticity).powf(BOND_CURVE) * REFERENCE_SPACING / spacing;
        let angular = std::f32::consts::TAU * frequency;
        1.0 / (angular * angular)
    }

    fn flow_rate(&self) -> f32 {
        SLOWEST_FLOW * FLOW_RANGE.powf(unit(self.relaxation))
    }

    fn cohesion_acceleration(&self) -> f32 {
        COHESION * unit(self.cohesion) * (1.0 - BONDED_COHESION * unit(self.elasticity))
    }

    pub(crate) fn cohesive_stiffness(&self, h: f32) -> f32 {
        self.cohesion_acceleration() * COHESIVE_SLOPE * COHESIVE_NEIGHBOURS / h
    }

    pub(crate) fn block(&self, h: f32, spacing: f32) -> SubstanceBlock {
        let viscosity = unit(self.viscosity);
        let cohesion = self.cohesion_acceleration();
        let adhesion = unit(self.adhesion);
        SubstanceBlock {
            flow: [
                self.kinematic_viscosity(),
                SMOOTHING + SMOOTHING_THICK * viscosity,
                self.vorticity(),
                unit(self.spray),
            ],
            stick: [
                cohesion,
                if adhesion >= ANCHORED_FROM {
                    adhesion
                } else {
                    0.0
                },
                ADHESION * adhesion * adhesion,
                STICK_RATE * adhesion * adhesion,
            ],
            bond: [
                if self.bonds() {
                    self.compliance(spacing)
                } else {
                    0.0
                },
                self.flow_rate(),
                MIN_YIELD + YIELD_RANGE * unit(self.elasticity),
                DASHPOT * viscosity,
            ],
            tint: [
                unit(self.tint[0]),
                unit(self.tint[1]),
                unit(self.tint[2]),
                WETTING * unit(self.wetting),
            ],
            contact: [
                FRICTION * unit(self.friction),
                CONTACT_DRAG * unit(self.friction),
                self.cohesive_stiffness(h),
                ANCHOR_BREAK * adhesion * adhesion * h,
            ],
        }
    }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
