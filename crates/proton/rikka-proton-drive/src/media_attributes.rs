use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaAttributes {
    #[serde(rename = "Width", default, skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(rename = "Height", default, skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
    #[serde(rename = "Duration", default, skip_serializing_if = "Option::is_none")]
    duration: Option<f64>,
}

impl MediaAttributes {
    #[must_use]
    pub fn new() -> MediaAttributes {
        MediaAttributes::default()
    }

    #[must_use]
    pub fn with_size(mut self, width: u32, height: u32) -> MediaAttributes {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    #[must_use]
    pub fn with_duration_seconds(mut self, seconds: u64) -> MediaAttributes {
        self.duration = Some(seconds as f64);
        self
    }

    #[must_use]
    pub fn width(&self) -> Option<u32> {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> Option<u32> {
        self.height
    }

    #[must_use]
    pub fn duration_seconds(&self) -> Option<u64> {
        self.duration
            .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
            .map(|seconds| seconds.floor() as u64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Location {
    #[serde(rename = "Latitude")]
    latitude: f64,
    #[serde(rename = "Longitude")]
    longitude: f64,
}

impl Location {
    #[must_use]
    pub fn new(latitude: f64, longitude: f64) -> Location {
        Location {
            latitude,
            longitude,
        }
    }

    #[must_use]
    pub fn latitude(&self) -> f64 {
        self.latitude
    }

    #[must_use]
    pub fn longitude(&self) -> f64 {
        self.longitude
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CameraAttributes {
    #[serde(
        rename = "CaptureTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    capture_time: Option<String>,
    #[serde(rename = "Device", default, skip_serializing_if = "Option::is_none")]
    device: Option<String>,
    #[serde(
        rename = "Orientation",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    orientation: Option<u8>,
}

impl CameraAttributes {
    #[must_use]
    pub fn new() -> CameraAttributes {
        CameraAttributes::default()
    }

    #[must_use]
    pub fn with_captured_at(mut self, at: Timestamp) -> CameraAttributes {
        self.capture_time = Some(at.to_string());
        self
    }

    #[must_use]
    pub fn with_device(mut self, device: impl Into<String>) -> CameraAttributes {
        self.device = Some(device.into());
        self
    }

    #[must_use]
    pub fn with_orientation(mut self, orientation: u8) -> CameraAttributes {
        self.orientation = Some(orientation);
        self
    }

    #[must_use]
    pub fn captured_at(&self) -> Option<Timestamp> {
        self.capture_time
            .as_deref()
            .and_then(|value| value.parse().ok())
    }

    #[must_use]
    pub fn device(&self) -> Option<&str> {
        self.device.as_deref()
    }

    #[must_use]
    pub fn orientation(&self) -> Option<u8> {
        self.orientation
    }
}
