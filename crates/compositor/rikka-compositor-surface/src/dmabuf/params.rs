use std::os::fd::OwnedFd;

use rikka_compositor_geometry::Size;
use rikka_compositor_protocols::zwp_linux_buffer_params_v1::Error;
use rikka_compositor_types::{Format, Fourcc, Modifier};
use rustix::fs::{Dev, SeekFrom, seek};

use super::buffer::{Dmabuf, DmabufFlags, DmabufPlane};
use super::planes::plane_count;

const MAX_PLANES: usize = 4;
const SAME_MODIFIER_SINCE: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Invalid {
    AlreadyUsed,
    PlaneIndex,
    PlaneSet,
    Incomplete,
    Format,
    Dimensions,
    OutOfBounds,
    DeviceSize,
}

impl Invalid {
    pub(crate) fn error(self) -> Error {
        match self {
            Self::AlreadyUsed => Error::AlreadyUsed,
            Self::PlaneIndex => Error::PlaneIdx,
            Self::PlaneSet => Error::PlaneSet,
            Self::Incomplete => Error::Incomplete,
            Self::Format => Error::InvalidFormat,
            Self::Dimensions => Error::InvalidDimensions,
            Self::OutOfBounds => Error::OutOfBounds,
            Self::DeviceSize => Error::InvalidDevTSize,
        }
    }
}

#[derive(Debug)]
struct Plane {
    plane: DmabufPlane,
    modifier: u64,
}

#[derive(Debug, Default)]
pub(crate) struct Params {
    planes: [Option<Plane>; MAX_PLANES],
    used: bool,
    sampling_device: Option<Dev>,
}

impl Params {
    pub(crate) fn add(
        &mut self,
        fd: OwnedFd,
        index: u32,
        offset: u32,
        stride: u32,
        modifier: u64,
        version: u32,
    ) -> Result<(), Invalid> {
        if self.used {
            return Err(Invalid::AlreadyUsed);
        }
        let slot = usize::try_from(index)
            .ok()
            .and_then(|index| self.planes.get(index))
            .ok_or(Invalid::PlaneIndex)?;
        if slot.is_some() {
            return Err(Invalid::PlaneSet);
        }
        if version >= SAME_MODIFIER_SINCE
            && self
                .planes
                .iter()
                .flatten()
                .any(|plane| plane.modifier != modifier)
        {
            return Err(Invalid::Format);
        }
        self.planes[index as usize] = Some(Plane {
            plane: DmabufPlane { fd, offset, stride },
            modifier,
        });
        Ok(())
    }

    pub(crate) fn set_sampling_device(&mut self, device: &[u8]) -> Result<(), Invalid> {
        if self.used {
            return Err(Invalid::AlreadyUsed);
        }
        let bytes: [u8; 8] = device.try_into().map_err(|_| Invalid::DeviceSize)?;
        self.sampling_device = Some(Dev::from_ne_bytes(bytes));
        Ok(())
    }

    pub(crate) fn build(
        &mut self,
        width: i32,
        height: i32,
        format: u32,
        flags: DmabufFlags,
        supported: impl Fn(&Format) -> bool,
    ) -> Result<Dmabuf, Invalid> {
        if std::mem::replace(&mut self.used, true) {
            return Err(Invalid::AlreadyUsed);
        }
        let count = self
            .planes
            .iter()
            .take_while(|plane| plane.is_some())
            .count();
        if count == 0 || self.planes[count..].iter().any(Option::is_some) {
            return Err(Invalid::Incomplete);
        }
        if width < 1 || height < 1 {
            return Err(Invalid::Dimensions);
        }
        let modifier = self.planes[0]
            .as_ref()
            .map_or(Modifier::INVALID, |plane| Modifier(plane.modifier));
        let format = Format::new(Fourcc(format), modifier);
        if !supported(&format) {
            return Err(Invalid::Format);
        }
        let implicit = matches!(format.modifier, Modifier::INVALID | Modifier::LINEAR);
        let needed = plane_count(format.fourcc);
        if count < needed || (implicit && count != needed) {
            return Err(Invalid::Incomplete);
        }
        let planes: Vec<DmabufPlane> = self
            .planes
            .iter_mut()
            .take(count)
            .filter_map(|plane| plane.take().map(|plane| plane.plane))
            .collect();
        for (index, plane) in planes.iter().enumerate() {
            within_bounds(plane, index == 0, height.unsigned_abs())?;
        }
        let fenced_planes = distinct_planes(&planes);
        Ok(Dmabuf {
            size: Size::new(width, height),
            format,
            flags,
            planes,
            sampling_device: self.sampling_device,
            fenced_planes,
        })
    }
}

fn within_bounds(plane: &DmabufPlane, first: bool, height: u32) -> Result<(), Invalid> {
    let offset = u64::from(plane.offset);
    let stride = u64::from(plane.stride);
    let row_end = offset + stride;
    let end = if first {
        offset + stride * u64::from(height)
    } else {
        row_end
    };
    if end > u64::from(u32::MAX) {
        return Err(Invalid::OutOfBounds);
    }
    let Ok(size) = seek(&plane.fd, SeekFrom::End(0)) else {
        return Ok(());
    };
    let _ = seek(&plane.fd, SeekFrom::Start(0));
    if row_end > size || end > size {
        return Err(Invalid::OutOfBounds);
    }
    Ok(())
}

fn distinct_planes(planes: &[DmabufPlane]) -> u8 {
    let mut identities = Vec::with_capacity(planes.len());
    let mut distinct = 0;
    for (index, plane) in planes.iter().enumerate() {
        let identity = rustix::fs::fstat(&plane.fd)
            .ok()
            .map(|stat| (stat.st_dev, stat.st_ino));
        if identity.is_some_and(|identity| identities.contains(&identity)) {
            continue;
        }
        identities.extend(identity);
        distinct |= 1 << index;
    }
    distinct
}
