#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Quality {
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Budget {
    pub particles: u32,
    pub piece_particles: u32,
    pub spacing: f32,
}

impl From<Quality> for Budget {
    fn from(quality: Quality) -> Budget {
        match quality {
            Quality::High => Budget {
                particles: Budget::DEFAULT_PARTICLES,
                piece_particles: 1_024,
                spacing: 0.015,
            },
            Quality::Medium => Budget {
                particles: 3_500,
                piece_particles: 768,
                spacing: 0.02,
            },
            Quality::Low => Budget {
                particles: 2_500,
                piece_particles: 512,
                spacing: 0.03,
            },
        }
    }
}

impl Budget {
    pub const DEFAULT_PARTICLES: u32 = 4_500;

    #[must_use]
    pub fn particles(self, particles: u32) -> Budget {
        Budget { particles, ..self }
    }

    #[must_use]
    pub fn piece_particles(self, piece_particles: u32) -> Budget {
        Budget {
            piece_particles,
            ..self
        }
    }

    #[must_use]
    pub fn spacing(self, spacing: f32) -> Budget {
        Budget { spacing, ..self }
    }

    pub(crate) fn piece_limit(self, counts: &[usize]) -> usize {
        let mut sorted = counts.to_vec();
        sorted.sort_unstable();
        let mut left = self.particles as usize;
        let mut fair = usize::MAX;
        for (at, count) in sorted.iter().enumerate() {
            let sharing = sorted.len() - at;
            if count * sharing > left {
                fair = left / sharing;
                break;
            }
            left -= count;
        }
        fair.min(self.piece_particles as usize)
    }

    pub(crate) fn sanitized(self) -> Budget {
        Budget {
            particles: self.particles.max(16),
            piece_particles: self.piece_particles.max(16),
            spacing: if self.spacing.is_finite() {
                self.spacing.clamp(0.004, 0.2)
            } else {
                0.015
            },
        }
    }
}

impl Default for Budget {
    fn default() -> Budget {
        Budget::from(Quality::High)
    }
}
