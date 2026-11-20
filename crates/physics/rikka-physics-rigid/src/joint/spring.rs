use std::f32::consts::TAU;

use crate::solver::softness::Softness;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpringStiffness {
    Hertz(f32),
    Coefficient(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpringDamping {
    Ratio(f32),
    Coefficient(f32),
}

#[must_use]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    stiffness: SpringStiffness,
    damping: SpringDamping,
}

impl Spring {
    pub fn hertz(hertz: f32) -> Self {
        Self {
            stiffness: SpringStiffness::Hertz(hertz),
            damping: SpringDamping::Ratio(1.0),
        }
    }

    pub fn stiffness(stiffness: f32) -> Self {
        Self {
            stiffness: SpringStiffness::Coefficient(stiffness),
            damping: SpringDamping::Coefficient(0.0),
        }
    }

    pub fn damping_ratio(mut self, ratio: f32) -> Self {
        self.damping = SpringDamping::Ratio(ratio);
        self
    }

    pub fn damping(mut self, coefficient: f32) -> Self {
        self.damping = SpringDamping::Coefficient(coefficient);
        self
    }

    pub fn stiffness_setting(&self) -> SpringStiffness {
        self.stiffness
    }

    pub fn damping_setting(&self) -> SpringDamping {
        self.damping
    }

    pub(crate) fn to_parts(self) -> ([f32; 2], [bool; 2]) {
        let (stiffness, stiffness_coefficient) = match self.stiffness {
            SpringStiffness::Hertz(hertz) => (hertz, false),
            SpringStiffness::Coefficient(stiffness) => (stiffness, true),
        };
        let (damping, damping_coefficient) = match self.damping {
            SpringDamping::Ratio(ratio) => (ratio, false),
            SpringDamping::Coefficient(coefficient) => (coefficient, true),
        };
        (
            [stiffness, damping],
            [stiffness_coefficient, damping_coefficient],
        )
    }

    pub(crate) fn from_parts(values: [f32; 2], coefficients: [bool; 2]) -> Self {
        let [stiffness, damping] = values;
        Self {
            stiffness: if coefficients[0] {
                SpringStiffness::Coefficient(stiffness)
            } else {
                SpringStiffness::Hertz(stiffness)
            },
            damping: if coefficients[1] {
                SpringDamping::Coefficient(damping)
            } else {
                SpringDamping::Ratio(damping)
            },
        }
    }

    pub fn is_valid(&self) -> bool {
        let stiffness = match self.stiffness {
            SpringStiffness::Hertz(hertz) => hertz,
            SpringStiffness::Coefficient(stiffness) => stiffness,
        };
        let damping = match self.damping {
            SpringDamping::Ratio(value) | SpringDamping::Coefficient(value) => value,
        };
        stiffness.is_finite() && stiffness >= 0.0 && damping.is_finite() && damping >= 0.0
    }

    pub fn frequency(&self, effective_mass: f32) -> (f32, f32) {
        let hertz = match self.stiffness {
            SpringStiffness::Hertz(hertz) => hertz,
            SpringStiffness::Coefficient(stiffness) if effective_mass > 0.0 => {
                (stiffness / effective_mass).sqrt() / TAU
            }
            SpringStiffness::Coefficient(_) => 0.0,
        };
        let ratio = match self.damping {
            SpringDamping::Ratio(ratio) => ratio,
            SpringDamping::Coefficient(coefficient) => {
                let omega = TAU * hertz;
                if omega > 0.0 && effective_mass > 0.0 {
                    coefficient / (2.0 * effective_mass * omega)
                } else {
                    0.0
                }
            }
        };
        (hertz, ratio)
    }

    pub(crate) fn softness(&self, effective_mass: f32, h: f32, max_hertz: f32) -> Softness {
        let (hertz, ratio) = self.frequency(effective_mass);
        if hertz <= 0.0 {
            return self.damper(effective_mass, h);
        }
        Softness::new(hertz.min(max_hertz), ratio, h)
    }

    fn damper(&self, effective_mass: f32, h: f32) -> Softness {
        match self.damping {
            SpringDamping::Coefficient(coefficient)
                if coefficient > 0.0 && effective_mass > 0.0 =>
            {
                let a2 = h * coefficient / effective_mass;
                let a3 = 1.0 / (1.0 + a2);
                Softness {
                    bias_rate: 0.0,
                    mass_scale: a2 * a3,
                    impulse_scale: a3,
                }
            }
            _ => Softness::default(),
        }
    }
}

impl rikka_file_format::Codec for Spring {
    type Error = crate::Error;

    fn encode(&self, out: &mut rikka_file_format::Writer<'_>) {
        match self.stiffness {
            SpringStiffness::Hertz(hertz) => {
                out.tag(0);
                out.put(&hertz);
            }
            SpringStiffness::Coefficient(stiffness) => {
                out.tag(1);
                out.put(&stiffness);
            }
        }
        match self.damping {
            SpringDamping::Ratio(ratio) => {
                out.tag(0);
                out.put(&ratio);
            }
            SpringDamping::Coefficient(coefficient) => {
                out.tag(1);
                out.put(&coefficient);
            }
        }
    }

    fn decode(input: &mut rikka_file_format::Reader<'_>) -> crate::Result<Self> {
        let what = "spring";
        let stiffness = match input.tag()? {
            0 => SpringStiffness::Hertz(input.get::<f32>()?),
            1 => SpringStiffness::Coefficient(input.get::<f32>()?),
            _ => return Err(crate::Error::Decode { what }),
        };
        let damping = match input.tag()? {
            0 => SpringDamping::Ratio(input.get::<f32>()?),
            1 => SpringDamping::Coefficient(input.get::<f32>()?),
            _ => return Err(crate::Error::Decode { what }),
        };
        Ok(Self { stiffness, damping })
    }
}
