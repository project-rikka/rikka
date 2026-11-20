use rikka_compositor_types::AxisSource;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollAxis {
    Moved { value: f64, v120: Option<f64> },
    Stopped,
}

impl ScrollAxis {
    pub(crate) fn read(source: AxisSource, present: bool, value: f64, v120: f64) -> Option<Self> {
        if !present {
            return None;
        }
        Some(match source {
            AxisSource::Wheel | AxisSource::WheelTilt => Self::Moved {
                value,
                v120: Some(v120),
            },
            AxisSource::Finger | AxisSource::Continuous if value == 0.0 => Self::Stopped,
            AxisSource::Finger | AxisSource::Continuous => Self::Moved { value, v120: None },
        })
    }
}
