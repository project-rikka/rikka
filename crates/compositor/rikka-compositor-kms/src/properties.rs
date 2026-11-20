use std::io;

use drm::control::{Device as _, ResourceHandle, property};

use crate::atomic::Sink;
use crate::drm_fd::DrmFd;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Default)]
pub(crate) struct ObjectProperties {
    object: u32,
    entries: Vec<(property::Info, property::RawValue)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CrtcProps {
    pub(crate) id: u32,
    pub(crate) active: property::Handle,
    pub(crate) mode_id: property::Handle,
    pub(crate) vrr_enabled: Option<property::Handle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConnectorProps {
    pub(crate) id: u32,
    pub(crate) crtc_id: property::Handle,
    pub(crate) max_bpc: Option<property::Handle>,
    pub(crate) colorspace: Option<Colorspace>,
    pub(crate) hdr_metadata: Option<property::Handle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Colorspace {
    handle: property::Handle,
    default: property::RawValue,
    bt2020_rgb: Option<property::RawValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlaneProps {
    pub(crate) id: u32,
    pub(crate) fb_id: property::Handle,
    pub(crate) crtc_id: property::Handle,
    pub(crate) source: [property::Handle; 4],
    pub(crate) destination: [property::Handle; 4],
    pub(crate) in_fence: Option<property::Handle>,
    pub(crate) damage_clips: Option<property::Handle>,
    pub(crate) blending: Option<Blending>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Blending {
    mode: property::Handle,
    premultiplied: property::RawValue,
    alpha: Option<(property::Handle, u64)>,
}

impl ObjectProperties {
    pub(crate) fn read<H: ResourceHandle>(fd: &DrmFd, object: H) -> io::Result<Self> {
        let values = fd.get_properties(object)?;
        let (handles, raw) = values.as_props_and_values();
        let mut entries = Vec::with_capacity(handles.len());
        for (handle, value) in handles.iter().zip(raw) {
            entries.push((fd.get_property(*handle)?, *value));
        }
        Ok(Self {
            object: object.into(),
            entries,
        })
    }

    fn find(&self, name: &str) -> Option<&(property::Info, property::RawValue)> {
        self.entries
            .iter()
            .find(|(info, _)| info.name().to_bytes() == name.as_bytes())
    }

    pub(crate) fn handle(&self, name: &str) -> Option<property::Handle> {
        self.find(name).map(|(info, _)| info.handle())
    }

    pub(crate) fn raw(&self, name: &str) -> Option<property::RawValue> {
        self.find(name).map(|(_, value)| *value)
    }

    pub(crate) fn range(&self, name: &str) -> Option<(u64, u64)> {
        match self.find(name)?.0.value_type() {
            property::ValueType::UnsignedRange(low, high) => Some((low, high)),
            _ => None,
        }
    }

    fn required(&self, name: &'static str) -> Result<property::Handle> {
        self.handle(name).ok_or(Error::MissingProperty {
            object: self.object,
            name,
        })
    }
}

impl CrtcProps {
    pub(crate) fn resolve(properties: &ObjectProperties) -> Result<Self> {
        Ok(Self {
            id: properties.object,
            active: properties.required("ACTIVE")?,
            mode_id: properties.required("MODE_ID")?,
            vrr_enabled: properties.handle("VRR_ENABLED"),
        })
    }
}

impl ConnectorProps {
    pub(crate) fn resolve(properties: &ObjectProperties) -> Result<Self> {
        Ok(Self {
            id: properties.object,
            crtc_id: properties.required("CRTC_ID")?,
            max_bpc: properties.handle("max bpc"),
            colorspace: Colorspace::resolve(properties),
            hdr_metadata: properties.handle("HDR_OUTPUT_METADATA"),
        })
    }

    pub(crate) fn signals_pq(&self) -> bool {
        self.hdr_metadata.is_some()
            && self
                .colorspace
                .is_some_and(|colorspace| colorspace.bt2020_rgb.is_some())
    }

    pub(crate) fn encode_signalling(&self, signalling: Signalling, sink: &mut impl Sink) {
        if let Some(colorspace) = self.colorspace {
            let value = match signalling {
                Signalling::Sdr => colorspace.default,
                Signalling::Pq { .. } => colorspace.bt2020_rgb.unwrap_or(colorspace.default),
            };
            sink.set(self.id, colorspace.handle, value);
        }
        if let Some(property) = self.hdr_metadata {
            let blob = match signalling {
                Signalling::Sdr => 0,
                Signalling::Pq { metadata } => metadata,
            };
            sink.set(self.id, property, blob);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Signalling {
    Sdr,
    Pq { metadata: u64 },
}

impl Colorspace {
    fn resolve(properties: &ObjectProperties) -> Option<Self> {
        let (info, _) = properties.find("Colorspace")?;
        let property::ValueType::Enum(values) = info.value_type() else {
            return None;
        };
        let named = |name: &[u8]| {
            values
                .values()
                .1
                .iter()
                .find(|value| value.name().to_bytes() == name)
                .map(|value| value.value())
        };
        Some(Self {
            handle: info.handle(),
            default: named(b"Default")?,
            bt2020_rgb: named(b"BT2020_RGB"),
        })
    }
}

impl PlaneProps {
    pub(crate) fn resolve(properties: &ObjectProperties) -> Result<Self> {
        Ok(Self {
            id: properties.object,
            fb_id: properties.required("FB_ID")?,
            crtc_id: properties.required("CRTC_ID")?,
            source: [
                properties.required("SRC_X")?,
                properties.required("SRC_Y")?,
                properties.required("SRC_W")?,
                properties.required("SRC_H")?,
            ],
            destination: [
                properties.required("CRTC_X")?,
                properties.required("CRTC_Y")?,
                properties.required("CRTC_W")?,
                properties.required("CRTC_H")?,
            ],
            in_fence: properties.handle("IN_FENCE_FD"),
            damage_clips: properties.handle("FB_DAMAGE_CLIPS"),
            blending: Blending::resolve(properties),
        })
    }
}

impl Blending {
    fn resolve(properties: &ObjectProperties) -> Option<Self> {
        let (info, _) = properties.find("pixel blend mode")?;
        let property::ValueType::Enum(modes) = info.value_type() else {
            return None;
        };
        let premultiplied = modes
            .values()
            .1
            .iter()
            .find(|mode| mode.name().to_bytes() == b"Pre-multiplied")?
            .value();
        let alpha = properties
            .handle("alpha")
            .zip(properties.range("alpha"))
            .map(|(handle, (_, opaque))| (handle, opaque));
        Some(Self {
            mode: info.handle(),
            premultiplied,
            alpha,
        })
    }

    pub(crate) fn encode(&self, plane: u32, sink: &mut impl Sink) {
        sink.set(plane, self.mode, self.premultiplied);
        if let Some((handle, opaque)) = self.alpha {
            sink.set(plane, handle, opaque);
        }
    }
}
